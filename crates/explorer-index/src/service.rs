use crate::crawl::MetadataCrawler;
use crate::db::{IndexDb, IndexedRoot};
use crate::query::{QueryEngine, SearchResponse};
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

/// High-level service managing indexed roots, background crawling, and search queries.
pub struct IndexService {
    db: Arc<IndexDb>,
    crawler: Arc<MetadataCrawler>,
    query_engine: Arc<QueryEngine>,
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    crawl_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl IndexService {
    /// Creates a new `IndexService` with the specified database path.
    pub fn new(db_path: &Path) -> Result<Self, ExplorerError> {
        let db = Arc::new(IndexDb::open(db_path)?);
        let crawler = Arc::new(MetadataCrawler::new(db.clone()));
        let query_engine = Arc::new(QueryEngine::new(db.clone()));

        Ok(Self {
            db,
            crawler,
            query_engine,
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            crawl_locks: Mutex::new(HashMap::new()),
        })
    }

    /// Creates an in-memory `IndexService` for testing.
    pub fn new_in_memory() -> Result<Self, ExplorerError> {
        let db = Arc::new(IndexDb::open_in_memory()?);
        let crawler = Arc::new(MetadataCrawler::new(db.clone()));
        let query_engine = Arc::new(QueryEngine::new(db.clone()));

        Ok(Self {
            db,
            crawler,
            query_engine,
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            crawl_locks: Mutex::new(HashMap::new()),
        })
    }

    /// List all currently configured indexed roots.
    pub fn list_roots(&self) -> Result<Vec<IndexedRoot>, ExplorerError> {
        self.db.list_roots()
    }

    /// Update persisted freshness after watcher registration or recovery fails.
    pub fn update_root_state(
        &self,
        root_id: &str,
        state: crate::db::RootState,
    ) -> Result<(), ExplorerError> {
        self.db.update_root_state(root_id, state)
    }

    /// Register a new directory root for indexing and start the baseline metadata crawl.
    pub fn add_root(&self, path: &Path) -> Result<IndexedRoot, ExplorerError> {
        let root = self.register_root(path)?;
        self.trigger_crawl(&root.id, &root.path);
        Ok(root)
    }

    /// Register a root without starting a crawl so the caller can establish its watcher first.
    pub fn register_root(&self, path: &Path) -> Result<IndexedRoot, ExplorerError> {
        self.db.add_root(path)
    }

    /// Remove an indexed root and cancel any active crawl.
    pub fn remove_root(&self, root_id: &str) -> Result<(), ExplorerError> {
        // Cancel any active crawler
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            if let Some(flag) = flags.remove(root_id) {
                flag.store(true, Ordering::Relaxed);
            }
        }
        self.db.remove_root(root_id)
    }

    /// Rebuild/re-crawl an existing indexed root.
    pub fn recrawl_root(&self, root_id: &str) -> Result<(), ExplorerError> {
        let roots = self.db.list_roots()?;
        let target = roots.into_iter().find(|r| r.id == root_id).ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                format!("Indexed root {root_id} not found"),
                "IndexService::recrawl_root",
            )
        })?;

        self.trigger_crawl(&target.id, &target.path);
        Ok(())
    }

    /// Execute search query.
    pub fn search(
        &self,
        query: &str,
        root_id: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<SearchResponse, ExplorerError> {
        self.query_engine
            .execute_search(query, root_id, page, page_size)
    }

    /// Trigger a background metadata crawl on a dedicated OS thread.
    fn trigger_crawl(&self, root_id: &str, root_path: &Path) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            if let Some(previous) = flags.get(root_id) {
                previous.store(true, Ordering::Relaxed);
            }
            flags.insert(root_id.to_string(), cancel_flag.clone());
        }

        let crawl_lock = {
            let mut locks = self.crawl_locks.lock().unwrap();
            locks
                .entry(root_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let crawler = self.crawler.clone();
        let r_id = root_id.to_string();
        let r_path = root_path.to_path_buf();
        let cancel_flags = self.cancel_flags.clone();

        std::thread::Builder::new()
            .name(format!("index-crawl-{}", &r_id[..8.min(r_id.len())]))
            .spawn(move || {
                let _crawl_guard = crawl_lock.lock().unwrap();
                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }
                info!(
                    "Starting background metadata crawl for root {r_id} ({})",
                    r_path.display()
                );
                if let Err(e) = crawler.crawl_root(&r_id, &r_path, cancel_flag.clone()) {
                    warn!("Background crawl failed for root {r_id}: {e}");
                }
                let mut flags = cancel_flags.lock().unwrap();
                if flags
                    .get(&r_id)
                    .is_some_and(|active| Arc::ptr_eq(active, &cancel_flag))
                {
                    flags.remove(&r_id);
                }
            })
            .expect("spawn crawl thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_index_service_add_search_remove() {
        let temp = tempdir().unwrap();
        std::fs::write(
            temp.path().join(".rust-explorer-fixture-root"),
            "index fixture",
        )
        .unwrap();
        let data_dir = temp.path().join("data");
        std::fs::create_dir(&data_dir).unwrap();
        let root_dir = data_dir.as_path();

        fs::write(root_dir.join("quarterly_results.xlsx"), "data").unwrap();

        let service = IndexService::new_in_memory().unwrap();
        let root = service.add_root(root_dir).unwrap();

        // Give background crawler a moment
        std::thread::sleep(std::time::Duration::from_millis(300));

        let res = service.search("quarterly", None, 1, 10).unwrap();
        assert_eq!(res.total_matches, 1);
        assert_eq!(res.results[0].display_name, "quarterly_results.xlsx");

        service.remove_root(&root.id).unwrap();
        let roots = service.list_roots().unwrap();
        assert!(roots.is_empty());
    }
}
