use crate::state::AppState;
use explorer_index::{IndexService, IndexedRoot, RootState};
use explorer_watch::{WatchMode, WatchNotification};
use explorer_win::instance::InstanceLease;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

pub type InitializedApp = (Arc<Mutex<InstanceLease>>, AppState);

/// Fallibly initializes the application runtime and durable state.
/// Ensures that single-instance lease is acquired before any DB or journal is opened.
pub fn initialize_application(
    state_dir: &Path,
) -> Result<Option<InitializedApp>, Box<dyn std::error::Error>> {
    let Some(lease) = InstanceLease::acquire(state_dir)? else {
        if !explorer_win::instance::focus_existing(state_dir) {
            tracing::warn!("An existing instance owns the state directory; focus was unavailable");
        }
        return Ok(None);
    };
    let lease = Arc::new(Mutex::new(lease));
    let state = AppState::new(state_dir, lease.clone())?;
    Ok(Some((lease, state)))
}

/// Helper to asynchronously fetch all registered indexed roots.
pub async fn list_indexed_roots(index_service: Arc<IndexService>) -> Vec<IndexedRoot> {
    match tokio::task::spawn_blocking(move || index_service.list_roots()).await {
        Ok(Ok(roots)) => roots,
        Ok(Err(error)) => {
            tracing::warn!("Unable to read indexed roots: {error}");
            Vec::new()
        }
        Err(error) => {
            tracing::warn!("Indexed-root lookup worker failed: {error}");
            Vec::new()
        }
    }
}

/// Restores recursive watches for indexed roots and repairs interrupted crawls.
///
/// A full re-crawl at every launch pegs a CPU core for minutes on multi-hundred
/// thousand entry roots. Startup therefore only resumes crawls that were actually
/// interrupted (`Scanning`) or never completed (`NotIndexed`); stale-but-complete
/// roots are refreshed lazily, one at a time, well after the window is interactive.
pub async fn restore_indexed_roots(
    index_service: Arc<IndexService>,
    watch_service: Arc<explorer_watch::WatchService>,
) {
    let roots = list_indexed_roots(index_service.clone()).await;
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for root in roots {
        let dedupe_key = root
            .display_path
            .trim_end_matches(['\\', '/'])
            .to_ascii_lowercase();
        if !seen_paths.insert(dedupe_key) {
            tracing::warn!(
                "Skipping duplicate indexed root {} ({})",
                root.display_path,
                root.id
            );
            continue;
        }

        let root_id = root.id.clone();
        let root_path = root.path.clone();
        let root_state = root.state;
        let completed_epoch = root.completed_epoch.max(0) as u64;
        let subscriber_id = format!("indexed-root:{root_id}");
        let crawl_root_id = root_id.clone();
        let state_root_id = root_id.clone();
        let index = index_service.clone();
        let watch = watch_service.clone();

        let result = tokio::task::spawn_blocking(move || {
            match watch.subscribe(&root_path, WatchMode::Recursive, &subscriber_id) {
                Ok(()) => {
                    if matches!(root_state, RootState::Scanning | RootState::NotIndexed) {
                        index.recrawl_root(&crawl_root_id)
                    } else {
                        Ok(())
                    }
                }
                Err(error) => {
                    let state = match std::fs::metadata(&root_path) {
                        Err(metadata_error)
                            if metadata_error.kind() == std::io::ErrorKind::NotFound =>
                        {
                            RootState::Offline
                        }
                        _ => RootState::NeedsReconcile,
                    };
                    let _ = index.update_root_state(&state_root_id, state);
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

        // Refresh a stale completed index or a root flagged for reconciliation
        // only after the UI has had time to settle. `completed_epoch` is stored
        // in Unix milliseconds by `MetadataCrawler::crawl_root`.
        const STALE_AFTER_MS: u64 = 24 * 60 * 60 * 1000;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let stale_ready =
            completed_epoch > 0 && now_ms.saturating_sub(completed_epoch) > STALE_AFTER_MS;
        if (stale_ready || root_state == RootState::NeedsReconcile)
            && !matches!(root_state, RootState::Scanning | RootState::NotIndexed)
        {
            let index = index_service.clone();
            let root_id = root_id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(120)).await;
                if let Ok(Err(error)) =
                    tokio::task::spawn_blocking(move || index.recrawl_root(&root_id)).await
                {
                    tracing::warn!("Stale indexed-root refresh failed: {error}");
                }
            });
        }
    }

    // Automatically register OneDrive roots if logged in and not already covered
    let od_accounts = explorer_win::onedrive::get_onedrive_accounts();
    for od in od_accounts {
        let p = std::path::PathBuf::from(&od.path);
        if p.is_dir() {
            let index = index_service.clone();
            let watch = watch_service.clone();
            tokio::task::spawn_blocking(move || {
                if let Ok(root) = index.register_root(&p) {
                    let sub_id = format!("indexed-root:{}", root.id);
                    let _ = watch.subscribe(&root.path, WatchMode::Recursive, &sub_id);
                    let _ = index.recrawl_root(&root.id);
                }
            })
            .await
            .ok();
        }
    }
}

/// Debounced incremental index reconciliation scheduler for an indexed root.
/// When `overflow` is true, falls back to a full root recrawl; otherwise only the
/// affected directories in `changed_paths` are re-enumerated and pruned.
pub fn schedule_index_reconcile(
    index_service: Arc<IndexService>,
    pending: &mut HashMap<String, JoinHandle<()>>,
    root_id: String,
    changed_paths: Vec<PathBuf>,
    overflow: bool,
) {
    if let Some(previous) = pending.remove(&root_id) {
        previous.abort();
    }

    let key = root_id.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        let service = index_service.clone();
        let res = tokio::task::spawn_blocking(move || {
            if overflow || changed_paths.is_empty() {
                service.recrawl_root(&root_id)
            } else {
                service.reconcile_paths(&root_id, &changed_paths)
            }
        })
        .await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!("Unable to reconcile indexed root after watch event: {error}")
            }
            Err(error) => tracing::warn!("Indexed-root reconciliation worker failed: {error}"),
        }
    });
    pending.insert(key, task);
}

/// Background loop consuming coalesced `WatchNotification` batches and keeping
/// matching indexed roots up to date without waking the UI thread.
pub async fn run_index_watch_reconciler(
    index_service: Arc<IndexService>,
    watch_service: Arc<explorer_watch::WatchService>,
) {
    let mut rx = watch_service.subscribe_notifications();
    let mut pending: HashMap<String, JoinHandle<()>> = HashMap::new();

    while let Ok(notif) = rx.recv().await {
        dispatch_watch_to_index(&index_service, &mut pending, &notif).await;
    }
}

async fn dispatch_watch_to_index(
    index_service: &Arc<IndexService>,
    pending: &mut HashMap<String, JoinHandle<()>>,
    notif: &WatchNotification,
) {
    let roots = list_indexed_roots(index_service.clone()).await;
    if roots.is_empty() {
        return;
    }

    let notif_norm = normalize_compare_path(&notif.dir_path);
    for root in roots {
        let root_norm = normalize_compare_path(&root.path);
        let matches_root = notif_norm == root_norm
            || notif_norm
                .strip_prefix(&root_norm)
                .is_some_and(|rest| rest.starts_with('\\') || rest.starts_with('/'));
        if !matches_root {
            continue;
        }

        let changed_paths: Vec<PathBuf> = notif.changes.iter().map(|c| c.path.clone()).collect();
        schedule_index_reconcile(
            index_service.clone(),
            pending,
            root.id,
            changed_paths,
            notif.is_overflow,
        );
    }
}

fn normalize_compare_path(path: &Path) -> String {
    explorer_win::path::strip_extended_prefix(path)
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}
