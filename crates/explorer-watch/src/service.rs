use crate::adapter::{NotifyAdapter, RawWatchEvent};
use crate::coalesce::EventCoalescer;
use crate::events::{WatchMode, WatchNotification};
use crate::reconcile::ReconciliationManager;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tokio::sync::{broadcast, mpsc};
use tracing::debug;

/// Tracks active subscribers and recursive mode for a watched directory.
#[derive(Debug, Clone)]
struct PathSubscription {
    recursive: bool,
    subscribers: HashSet<String>,
}

/// Thread-safe service coordinating filesystem watching, coalescing, and tab subscriptions.
pub struct WatchService {
    adapter: Mutex<Option<NotifyAdapter>>,
    subscriptions: Mutex<HashMap<PathBuf, PathSubscription>>,
    watched_paths: Arc<RwLock<HashSet<PathBuf>>>,
    reconciliation: Arc<ReconciliationManager>,
    notif_tx: broadcast::Sender<WatchNotification>,
    _event_tx: mpsc::Sender<RawWatchEvent>,
}

impl WatchService {
    /// Initializes the `WatchService`, launching the background event coalescer task.
    pub fn new() -> Result<Self, ExplorerError> {
        let (raw_tx, raw_rx) = mpsc::channel::<RawWatchEvent>(4096);
        let (notif_tx, _) = broadcast::channel::<WatchNotification>(1024);

        let watched_paths = Arc::new(RwLock::new(HashSet::new()));
        let reconciliation = Arc::new(ReconciliationManager::new());

        let adapter = NotifyAdapter::new(raw_tx.clone())?;

        let coalescer = EventCoalescer::new(watched_paths.clone(), notif_tx.clone());

        // Spawn async coalescer on a dedicated background thread with its own runtime
        // so WatchService can be instantiated from any thread (with or without an ambient runtime).
        std::thread::Builder::new()
            .name("watch-coalescer".to_string())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("Failed to create tokio runtime for watch-coalescer: {e}");
                        return;
                    }
                };
                rt.block_on(coalescer.run_loop(raw_rx));
            })
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to spawn watch-coalescer thread: {e}"),
                    "WatchService::new",
                )
            })?;

        Ok(Self {
            adapter: Mutex::new(Some(adapter)),
            subscriptions: Mutex::new(HashMap::new()),
            watched_paths,
            reconciliation,
            notif_tx,
            _event_tx: raw_tx,
        })
    }

    /// Subscribe to changes for a given directory path.
    ///
    /// Deduplicates subscriptions and reference-counts by `subscriber_id`.
    /// If another subscriber already watches the folder non-recursively and a recursive
    /// watch is requested, the native watch is upgraded.
    pub fn subscribe(
        &self,
        path: &Path,
        mode: WatchMode,
        subscriber_id: &str,
    ) -> Result<(), ExplorerError> {
        let canonical = normalize_watch_path(path);
        let recursive = mode == WatchMode::Recursive;

        let mut subs = self.subscriptions.lock().unwrap();
        let mut adapter_guard = self.adapter.lock().unwrap();
        let adapter = adapter_guard.as_mut().ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::Internal,
                "WatchService has been shut down",
                "WatchService::subscribe",
            )
        })?;

        match subs.get_mut(&canonical) {
            Some(existing) => {
                existing.subscribers.insert(subscriber_id.to_string());
                if recursive && !existing.recursive {
                    debug!("Upgrading watch on {} to recursive", canonical.display());
                    adapter.watch(&canonical, true)?;
                    existing.recursive = true;
                }
            }
            None => {
                debug!(
                    "Registering new watch on {} (recursive: {}) for subscriber {}",
                    canonical.display(),
                    recursive,
                    subscriber_id
                );
                adapter.watch(&canonical, recursive)?;

                let mut sub_set = HashSet::new();
                sub_set.insert(subscriber_id.to_string());
                subs.insert(
                    canonical.clone(),
                    PathSubscription {
                        recursive,
                        subscribers: sub_set,
                    },
                );

                let mut paths = self.watched_paths.write().unwrap();
                paths.insert(canonical);
            }
        }

        Ok(())
    }

    /// Unsubscribe a subscriber from a watched directory path.
    ///
    /// If no remaining subscribers watch this directory, the native OS watch is removed.
    pub fn unsubscribe(&self, path: &Path, subscriber_id: &str) -> Result<(), ExplorerError> {
        let canonical = normalize_watch_path(path);

        let mut subs = self.subscriptions.lock().unwrap();
        let mut adapter_guard = self.adapter.lock().unwrap();

        let should_unwatch = if let Some(existing) = subs.get_mut(&canonical) {
            existing.subscribers.remove(subscriber_id);
            existing.subscribers.is_empty()
        } else {
            false
        };

        if should_unwatch {
            subs.remove(&canonical);
            if let Some(adapter) = adapter_guard.as_mut() {
                let _ = adapter.unwatch(&canonical);
            }
            let mut paths = self.watched_paths.write().unwrap();
            paths.remove(&canonical);
            self.reconciliation.remove(&canonical);
            debug!(
                "Unwatched directory {} after last subscriber released",
                canonical.display()
            );
        }

        Ok(())
    }

    /// Clean up all subscriptions belonging to a specific subscriber (e.g. closed tab).
    pub fn unsubscribe_all(&self, subscriber_id: &str) -> Result<(), ExplorerError> {
        let mut subs = self.subscriptions.lock().unwrap();
        let mut adapter_guard = self.adapter.lock().unwrap();

        let mut unwatched = Vec::new();

        for (path, sub) in subs.iter_mut() {
            sub.subscribers.remove(subscriber_id);
            if sub.subscribers.is_empty() {
                unwatched.push(path.clone());
            }
        }

        for path in unwatched {
            subs.remove(&path);
            if let Some(adapter) = adapter_guard.as_mut() {
                let _ = adapter.unwatch(&path);
            }
            let mut paths = self.watched_paths.write().unwrap();
            paths.remove(&path);
            self.reconciliation.remove(&path);
            debug!(
                "Unwatched directory {} after subscriber {} detached",
                path.display(),
                subscriber_id
            );
        }

        Ok(())
    }

    /// Subscribe to the broadcast stream of coalesced change notifications.
    pub fn subscribe_notifications(&self) -> broadcast::Receiver<WatchNotification> {
        self.notif_tx.subscribe()
    }

    /// Check if a watched directory is currently marked degraded (e.g. due to buffer overflow).
    pub fn is_degraded(&self, path: &Path) -> bool {
        let canonical = normalize_watch_path(path);
        self.reconciliation.is_degraded(&canonical)
    }

    /// Mark a path as cleanly reconciled after enumeration.
    pub fn mark_reconciled(&self, path: &Path) {
        let canonical = normalize_watch_path(path);
        self.reconciliation
            .mark_reconciled(&canonical, Instant::now());
    }

    /// Get list of currently watched directories.
    pub fn get_watched_paths(&self) -> Vec<PathBuf> {
        let paths = self.watched_paths.read().unwrap();
        paths.iter().cloned().collect()
    }

    /// Get number of active subscribers for a path.
    pub fn subscriber_count(&self, path: &Path) -> usize {
        let canonical = normalize_watch_path(path);
        let subs = self.subscriptions.lock().unwrap();
        subs.get(&canonical)
            .map(|s| s.subscribers.len())
            .unwrap_or(0)
    }
}

/// Normalizes watch path, stripping trailing backslashes/slashes.
fn normalize_watch_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let trimmed = s.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        path.to_path_buf()
    } else {
        PathBuf::from(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_watch_service_subscription_lifecycle_and_refcounts() {
        let service = WatchService::new().expect("create watch service");
        let temp = tempdir().unwrap();
        let path = temp.path();

        // 1. Tab A subscribes
        service
            .subscribe(path, WatchMode::NonRecursive, "tab-1")
            .expect("tab-1 subscribe");
        assert_eq!(service.subscriber_count(path), 1);
        assert_eq!(service.get_watched_paths().len(), 1);

        // 2. Tab B subscribes to same path
        service
            .subscribe(path, WatchMode::NonRecursive, "tab-2")
            .expect("tab-2 subscribe");
        assert_eq!(service.subscriber_count(path), 2);
        // Deduplicated native watch
        assert_eq!(service.get_watched_paths().len(), 1);

        // 3. Tab A closes / unsubscribes
        service
            .unsubscribe(path, "tab-1")
            .expect("tab-1 unsubscribe");
        assert_eq!(service.subscriber_count(path), 1);
        // Still watched because tab-2 is open
        assert_eq!(service.get_watched_paths().len(), 1);

        // 4. Tab B unsubscribes
        service
            .unsubscribe(path, "tab-2")
            .expect("tab-2 unsubscribe");
        assert_eq!(service.subscriber_count(path), 0);
        // Completely unwatched
        assert_eq!(service.get_watched_paths().len(), 0);
    }

    #[tokio::test]
    async fn test_unsubscribe_all_cleans_tab_watches() {
        let service = WatchService::new().expect("create watch service");
        let t1 = tempdir().unwrap();
        let t2 = tempdir().unwrap();

        service
            .subscribe(t1.path(), WatchMode::NonRecursive, "tab-x")
            .unwrap();
        service
            .subscribe(t2.path(), WatchMode::NonRecursive, "tab-x")
            .unwrap();

        assert_eq!(service.get_watched_paths().len(), 2);

        // Tab X closes
        service.unsubscribe_all("tab-x").unwrap();
        assert_eq!(service.get_watched_paths().len(), 0);
    }

    #[tokio::test]
    async fn test_live_filesystem_modification_emits_notification() {
        let service = WatchService::new().expect("create watch service");
        let temp = tempdir().unwrap();
        let path = temp.path();

        let mut rx = service.subscribe_notifications();
        service
            .subscribe(path, WatchMode::NonRecursive, "test-tab")
            .unwrap();

        // Give notify a tiny moment to attach
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Create a file in the watched directory
        let new_file = path.join("live_test.txt");
        fs::write(&new_file, "live watch test").unwrap();

        // Wait for coalesced notification (timeout after 2s)
        let notif = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("did not timeout")
            .expect("received notification");

        let normalized_temp = normalize_watch_path(path);
        assert_eq!(notif.dir_path, normalized_temp);
        assert!(!notif.is_overflow);

        service.unsubscribe(path, "test-tab").unwrap();
    }
}
