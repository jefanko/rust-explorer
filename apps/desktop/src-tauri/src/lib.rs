pub mod commands;
pub mod state;

use state::AppState;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Rust Explorer application host");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            let mut notif_rx = state.watch_service.subscribe_notifications();

            tauri::async_runtime::spawn(async move {
                while let Ok(notif) = notif_rx.recv().await {
                    let _ = handle.emit("watch-notification", &notif);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::navigate,
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
            commands::plan_copy,
            commands::plan_move,
            commands::plan_recycle,
            commands::commit_plan,
            commands::create_folder,
            commands::rename_item,
            commands::execute_copy,
            commands::execute_move,
            commands::execute_recycle,
            commands::clipboard_write,
            commands::clipboard_read,
            commands::list_jobs,
            commands::watch_folder,
            commands::unwatch_folder,
            commands::unwatch_all,
            commands::get_watch_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
