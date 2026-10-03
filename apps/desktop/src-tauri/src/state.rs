use explorer_fs::FolderService;
use explorer_index::IndexService;
use explorer_jobs::OperationService;
use explorer_store::{JobJournal, SettingsStore};
use explorer_watch::WatchService;
use explorer_win::com::StaWorker;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AppState {
    pub folder_service: Arc<FolderService>,
    pub settings_store: Arc<SettingsStore>,
    pub job_journal: Arc<JobJournal>,
    pub operation_service: Arc<OperationService>,
    pub watch_service: Arc<WatchService>,
    pub index_service: Arc<IndexService>,
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

        let job_journal = Arc::new(
            JobJournal::open(&db_path).unwrap_or_else(|_| JobJournal::open_in_memory().unwrap()),
        );

        // Recover any jobs left unfinished across previous runs to Interrupted
        let _ = job_journal.recover_interrupted_jobs();

        let sta_worker = Arc::new(
            StaWorker::new("file-op-sta")
                .unwrap_or_else(|e| panic!("Failed to start dedicated File Operation STA: {e}")),
        );

        let operation_service = Arc::new(OperationService::new(sta_worker, job_journal.clone()));

        let watch_service = Arc::new(
            WatchService::new()
                .unwrap_or_else(|e| panic!("Failed to initialize WatchService: {e}")),
        );

        let index_db_path = state_dir.join("index.sqlite3");
        let index_service = Arc::new(
            IndexService::new(&index_db_path)
                .unwrap_or_else(|_| IndexService::new_in_memory().unwrap()),
        );

        Self {
            folder_service: Arc::new(FolderService::new()),
            settings_store,
            job_journal,
            operation_service,
            watch_service,
            index_service,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
