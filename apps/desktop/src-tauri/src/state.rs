use explorer_fs::FolderService;
use explorer_store::SettingsStore;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AppState {
    pub folder_service: Arc<FolderService>,
    pub settings_store: Arc<SettingsStore>,
}

impl AppState {
    pub fn new() -> Self {
        let state_dir = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("RustExplorer");
        let db_path = state_dir.join("state.sqlite3");

        let settings_store = Arc::new(
            SettingsStore::open(&db_path)
                .unwrap_or_else(|_| SettingsStore::open_in_memory().unwrap()),
        );

        Self {
            folder_service: Arc::new(FolderService::new()),
            settings_store,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
