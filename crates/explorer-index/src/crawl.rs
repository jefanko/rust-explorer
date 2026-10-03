use crate::db::{IndexDb, IndexEntryRecord, RootState};
use explorer_domain::errors::ExplorerError;
use explorer_domain::models::EntryKind;
use explorer_win::enumerate::enumerate_directory;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info, warn};

pub const BATCH_SIZE: usize = 500;
pub const BATCH_TIMEOUT: Duration = Duration::from_millis(100);

/// Metadata crawler configuration and statistics.
#[derive(Debug, Clone, Default)]
pub struct CrawlStats {
    pub files_indexed: u64,
    pub directories_indexed: u64,
    pub reparse_points_skipped: u64,
    pub excluded_skipped: u64,
    pub elapsed: Duration,
}

/// Runs a bounded metadata crawl over a designated root directory.
pub struct MetadataCrawler {
    db: Arc<IndexDb>,
}

impl MetadataCrawler {
    pub fn new(db: Arc<IndexDb>) -> Self {
        Self { db }
    }

    /// Crawls the specified root, performing paced batch inserts into SQLite.
    pub fn crawl_root(
        &self,
        root_id: &str,
        root_path: &Path,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<CrawlStats, ExplorerError> {
        let start_time = Instant::now();
        let mut stats = CrawlStats::default();

        self.db.update_root_state(root_id, RootState::Scanning)?;

        let current_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // BFS traversal queue: (dir_path, parent_entry_id)
        let mut queue: VecDeque<(PathBuf, Option<i64>)> = VecDeque::new();
        queue.push_back((root_path.to_path_buf(), None));

        let mut batch: Vec<IndexEntryRecord> = Vec::with_capacity(BATCH_SIZE);
        let mut last_flush = Instant::now();

        // Resolve app state directory to exclude it from indexing
        let state_dir_opt = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok()
            .map(|p| p.join("RustExplorer"));

        while let Some((dir, parent_id)) = queue.pop_front() {
            if cancel_flag.load(Ordering::Relaxed) {
                warn!("Crawl canceled by user or system for root {root_id}");
                let _ = self
                    .db
                    .update_root_state(root_id, RootState::NeedsReconcile);
                stats.elapsed = start_time.elapsed();
                return Ok(stats);
            }

            // Enumerate directory entries using Win32 API
            let entries = match enumerate_directory(&dir, None) {
                Ok(entries) => entries,
                Err(err) => {
                    // Non-fatal permission/access issue on a subdirectory; log and continue
                    debug!("Skipping inaccessible folder {}: {err}", dir.display());
                    continue;
                }
            };

            for entry in entries {
                let full_path = dir.join(&entry.display_name);

                let is_reparse = entry.kind == EntryKind::ReparsePoint;
                let is_dir = entry.kind == EntryKind::Directory;

                // Check exclusions
                if is_excluded(&entry.display_name, &full_path, state_dir_opt.as_deref()) {
                    stats.excluded_skipped += 1;
                    continue;
                }

                let kind_code = match entry.kind {
                    EntryKind::File => 0,
                    EntryKind::Directory => 1,
                    EntryKind::ReparsePoint => 2,
                };

                let name_norm = entry.display_name.to_lowercase();
                let extension_norm = entry.extension.to_lowercase();

                batch.push(IndexEntryRecord {
                    root_id: root_id.to_string(),
                    parent_id,
                    path: full_path.clone(),
                    name_display: entry.display_name.clone(),
                    name_norm,
                    extension_norm,
                    kind: kind_code,
                    size_bytes: entry.size_bytes,
                    modified_filetime: entry.modified_filetime,
                    attributes: entry.attributes,
                    seen_epoch: current_epoch,
                });

                if is_reparse {
                    stats.reparse_points_skipped += 1;
                    // Spec Section 14.4: Do not follow reparse points
                } else if is_dir {
                    stats.directories_indexed += 1;
                    queue.push_back((full_path, None));
                } else {
                    stats.files_indexed += 1;
                }

                // Paced batch commit: 500 entries or 100 ms of work
                if batch.len() >= BATCH_SIZE || last_flush.elapsed() >= BATCH_TIMEOUT {
                    self.db.batch_upsert_entries(&batch)?;
                    batch.clear();
                    last_flush = Instant::now();
                }
            }
        }

        // Flush any remaining entries
        if !batch.is_empty() {
            self.db.batch_upsert_entries(&batch)?;
            batch.clear();
        }

        // Prune entries from older epochs that were removed from disk
        let pruned = self.db.prune_unseen_entries(root_id, current_epoch)?;
        debug!("Pruned {pruned} missing entries from root {root_id}");

        // Mark root ready and update completion timestamp
        self.db.complete_root_scan(root_id, current_epoch)?;

        stats.elapsed = start_time.elapsed();
        info!(
            "Completed metadata crawl for root {}: {} files, {} dirs in {:?}",
            root_id, stats.files_indexed, stats.directories_indexed, stats.elapsed
        );

        Ok(stats)
    }
}

/// Checks if a directory or file should be excluded from index crawling.
fn is_excluded(name: &str, path: &Path, state_dir: Option<&Path>) -> bool {
    let lower = name.to_lowercase();
    if lower == "$recycle.bin" || lower == "system volume information" {
        return true;
    }

    if state_dir.is_some_and(|state| path.starts_with(state)) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_crawler_indexes_files_and_skips_exclusions() {
        let temp = tempdir().unwrap();
        let root_dir = temp.path();

        // Create test file structure
        let sub = root_dir.join("subfolder");
        fs::create_dir(&sub).unwrap();
        fs::write(root_dir.join("doc1.txt"), "hello").unwrap();
        fs::write(sub.join("doc2.pdf"), "world").unwrap();

        // Excluded folder
        let rec = root_dir.join("$Recycle.Bin");
        fs::create_dir(&rec).unwrap();
        fs::write(rec.join("junk.txt"), "trash").unwrap();

        let db = Arc::new(IndexDb::open_in_memory().unwrap());
        let root = db.add_root(root_dir).unwrap();

        let crawler = MetadataCrawler::new(db.clone());
        let cancel = Arc::new(AtomicBool::new(false));

        let stats = crawler.crawl_root(&root.id, root_dir, cancel).unwrap();
        assert_eq!(stats.files_indexed, 2); // doc1.txt, doc2.pdf
        assert_eq!(stats.directories_indexed, 1); // subfolder
        assert_eq!(stats.excluded_skipped, 1); // $Recycle.Bin

        let roots = db.list_roots().unwrap();
        assert_eq!(roots[0].state, RootState::Ready);
    }
}
