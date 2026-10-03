use explorer_fs::FolderService;
use std::sync::Arc;

pub struct AppState {
    pub folder_service: Arc<FolderService>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            folder_service: Arc::new(FolderService::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
