pub mod commands;
pub mod state;

use explorer_index::{IndexService, RootState};
use explorer_watch::WatchMode;
use explorer_watch::events::WatchNotification;
use state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;

fn schedule_index_reconcile(
    index_service: Arc<IndexService>,
    pending: &mut HashMap<String, tauri::async_runtime::JoinHandle<()>>,
    root_id: String,
) {
    if let Some(previous) = pending.remove(&root_id) {
        previous.abort();
    }

    let key = root_id.clone();
    let task = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        let service = index_service.clone();
        match tokio::task::spawn_blocking(move || service.recrawl_root(&root_id)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!("Unable to reconcile indexed root after watch event: {error}")
            }
            Err(error) => tracing::warn!("Indexed-root reconciliation worker failed: {error}"),
        }
    });
    pending.insert(key, task);
}

async fn indexed_roots(index_service: Arc<IndexService>) -> Vec<explorer_index::IndexedRoot> {
    match tokio::task::spawn_blocking(move || index_service.list_roots()).await {
        Ok(Ok(roots)) => roots,
        Ok(Err(error)) => {
            tracing::warn!("Unable to read indexed roots for watcher reconciliation: {error}");
            Vec::new()
        }
        Err(error) => {
            tracing::warn!("Indexed-root lookup worker failed: {error}");
            Vec::new()
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Rust Explorer application host");

    // The lease must outlive the host; never open a shared journal before acquiring it.
    let startup = (|| -> Result<_, Box<dyn std::error::Error>> {
        let state_dir = state::application_state_dir()?;
        let Some(lease) = explorer_win::instance::InstanceLease::acquire(&state_dir)? else {
            if !explorer_win::instance::focus_existing(&state_dir) {
                tracing::warn!(
                    "An existing instance owns the state directory; focus was unavailable"
                );
            }
            return Ok(None);
        };
        let lease = Arc::new(Mutex::new(lease));
        let state = AppState::new(&state_dir, lease.clone())?;
        Ok(Some((lease, state)))
    })();
    let (instance_lease, app_state) = match startup {
        Ok(Some(startup)) => startup,
        Ok(None) => return,
        Err(error) => {
            tracing::error!("Startup failed before enabling mutations: {error}");
            explorer_win::instance::show_startup_error(&format!(
                "Rust Explorer could not initialize durable state.\n\n{error}\n\nNo mutation executor was made available."
            ));
            return;
        }
    };
    let window_lease = instance_lease.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            let window = app.get_webview_window("main").ok_or("Main window is missing")?;
            window_lease.lock().map_err(|_| "Instance lease lock poisoned")?
                .publish_window(window.hwnd()?.0 as usize)?;
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            let mut notif_rx = state.watch_service.subscribe_notifications();
            let index_service = state.index_service.clone();
            let watch_service = state.watch_service.clone();

            // Restore recursive root watches before starting their recovery crawl.
            let startup_index = index_service.clone();
            let startup_watch = watch_service.clone();
            tauri::async_runtime::spawn(async move {
                for root in indexed_roots(startup_index.clone()).await {
                    let root_id = root.id.clone();
                    let root_path = root.path.clone();
                    let subscriber_id = format!("indexed-root:{root_id}");
                    let index = startup_index.clone();
                    let watch = startup_watch.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        match watch.subscribe(&root_path, WatchMode::Recursive, &subscriber_id) {
                            Ok(()) => index.recrawl_root(&root_id),
                            Err(error) => {
                                let state = match std::fs::metadata(&root_path) {
                                    Err(metadata_error)
                                        if metadata_error.kind() == std::io::ErrorKind::NotFound =>
                                    {
                                        RootState::Offline
                                    }
                                    _ => RootState::NeedsReconcile,
                                };
                                let _ = index.update_root_state(&root_id, state);
                                Err(error)
                            }
                        }
                    })
                    .await;
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            tracing::warn!("Unable to restore indexed-root watch: {error}");
                        }
                        Err(error) => {
                            tracing::warn!("Indexed-root startup recovery failed: {error}");
                        }
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                let mut pending_reconciles = HashMap::new();
                loop {
                    match notif_rx.recv().await {
                        Ok(notif) => {
                            let _ = handle.emit("watch-notification", &notif);
                            let roots = indexed_roots(index_service.clone()).await;
                            for root in roots {
                                if notif.dir_path.as_os_str().is_empty()
                                    || notif.dir_path.starts_with(&root.path)
                                {
                                    if notif.is_overflow {
                                        let index = index_service.clone();
                                        let root_id = root.id.clone();
                                        let _ = tokio::task::spawn_blocking(move || {
                                            index.update_root_state(
                                                &root_id,
                                                RootState::NeedsReconcile,
                                            )
                                        })
                                        .await;
                                    }
                                    schedule_index_reconcile(
                                        index_service.clone(),
                                        &mut pending_reconciles,
                                        root.id,
                                    );
                                }
                            }
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            tracing::warn!("Watch UI notification receiver lagged by {skipped}; refreshing visible tabs");
                            let overflow = WatchNotification {
                                dir_path: PathBuf::new(),
                                dir_path_display: String::new(),
                                dir_path_utf16: Vec::new(),
                                is_overflow: true,
                                changes: Vec::new(),
                            };
                            let _ = handle.emit("watch-notification", &overflow);
                            for root in indexed_roots(index_service.clone()).await {
                                let index = index_service.clone();
                                let root_id = root.id.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    index.update_root_state(
                                        &root_id,
                                        RootState::NeedsReconcile,
                                    )
                                })
                                .await;
                                schedule_index_reconcile(
                                    index_service.clone(),
                                    &mut pending_reconciles,
                                    root.id,
                                );
                            }
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::navigate,
            commands::navigate_native_path,
            commands::list_page,
            commands::refresh,
            commands::open_item,
            commands::show_properties,
            commands::open_in_explorer,
            commands::load_settings,
            commands::save_settings,
            commands::add_favorite,
            commands::remove_favorite,
            commands::plan_create_folder,
            commands::plan_rename,
            commands::plan_paste,
            commands::plan_recycle,
            commands::commit_plan,
            commands::create_folder,
            commands::rename_item,
            commands::clipboard_write_items,
            commands::clipboard_write_path,
            commands::list_jobs,
            commands::watch_folder,
            commands::unwatch_all,
            commands::list_indexed_roots,
            commands::add_indexed_root,
            commands::remove_indexed_root,
            commands::recrawl_indexed_root,
            commands::search_indexed,
            commands::open_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
