pub mod commands;
pub mod state;

use explorer_app::lifecycle::{
    initialize_application, list_indexed_roots, restore_indexed_roots, schedule_index_reconcile,
};
use explorer_index::RootState;
use explorer_watch::events::WatchNotification;
use state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Rust Explorer application host");

    let state_dir = match state::application_state_dir() {
        Ok(dir) => dir,
        Err(error) => {
            tracing::error!("Startup failed before enabling mutations: {error}");
            explorer_win::instance::show_startup_error(&format!(
                "Rust Explorer could not initialize durable state.\n\n{error}\n\nNo mutation executor was made available."
            ));
            return;
        }
    };

    let startup = match initialize_application(&state_dir) {
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
    let (instance_lease, app_state) = startup;
    let window_lease = instance_lease.clone();

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            let window = app.get_webview_window("main").ok_or("Main window is missing")?;
            window_lease
                .lock()
                .map_err(|_| "Instance lease lock poisoned")?
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
                restore_indexed_roots(startup_index, startup_watch).await;
            });

            tauri::async_runtime::spawn(async move {
                let mut pending_reconciles = HashMap::new();
                loop {
                    match notif_rx.recv().await {
                        Ok(notif) => {
                            let _ = handle.emit("watch-notification", &notif);
                            let roots = list_indexed_roots(index_service.clone()).await;
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
                            for root in list_indexed_roots(index_service.clone()).await {
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
            commands::plan_transfer,
            commands::read_preview,
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
