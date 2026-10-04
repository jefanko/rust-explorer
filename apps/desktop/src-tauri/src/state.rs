use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_fs::FolderService;
use explorer_index::IndexService;
use explorer_jobs::OperationService;
use explorer_store::{JobJournal, SettingsStore};
use explorer_watch::WatchService;
use explorer_win::com::StaWorker;
use std::path::{Path, PathBuf};
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
    pub fn new(
        state_dir: &Path,
        instance_lease: Arc<dyn Send + Sync>,
    ) -> Result<Self, ExplorerError> {
        let db_path = state_dir.join("state.sqlite3");

        let settings_store = Arc::new(SettingsStore::open(&db_path)?);

        let job_journal = Arc::new(JobJournal::open(&db_path)?);

        // Recover any jobs left unfinished across previous runs to Interrupted
        job_journal.recover_interrupted_jobs()?;

        let sta_worker = Arc::new(StaWorker::new("file-op-sta")?);

        let operation_service = Arc::new(OperationService::new_guarded(
            sta_worker,
            job_journal.clone(),
            instance_lease,
        ));

        let watch_service = Arc::new(WatchService::new()?);

        let index_db_path = state_dir.join("index.sqlite3");
        let index_service = Arc::new(IndexService::new(&index_db_path)?);

        Ok(Self {
            folder_service: Arc::new(FolderService::new()),
            settings_store,
            job_journal,
            operation_service,
            watch_service,
            index_service,
        })
    }
}

pub fn application_state_dir() -> Result<PathBuf, ExplorerError> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("RustExplorer"))
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::Internal,
                "An absolute LOCALAPPDATA directory is required for durable application state",
                "application_state_dir",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_durable_state_never_creates_an_available_executor() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".rust-explorer-fixture-root"),
            "startup fixture",
        )
        .unwrap();
        let lease = Arc::new(
            explorer_win::instance::InstanceLease::acquire(root.path())
                .unwrap()
                .unwrap(),
        );
        let db = root.path().join("state.sqlite3");
        std::fs::write(&db, "corrupt SQLite fixture").unwrap();
        assert!(AppState::new(root.path(), lease).is_err());
        assert_eq!(
            std::fs::read_to_string(db).unwrap(),
            "corrupt SQLite fixture"
        );
    }
}
