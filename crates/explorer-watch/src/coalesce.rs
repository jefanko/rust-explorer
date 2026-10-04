use crate::adapter::RawWatchEvent;
use crate::events::{WatchChange, WatchEventKind, WatchNotification};
use std::collections::{HashMap, HashSet};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, warn};

/// Maximum unique dirty directories tracked before triggering overflow reconciliation.
pub const MAX_DIRTY_DIRS: usize = 1024;

/// Coalescing debounce window (150 ms, conforming to 100-200 ms requirement).
pub const DEBOUNCE_DURATION: Duration = Duration::from_millis(150);

/// Maximum time to hold dirty changes before forcing a flush during continuous activity.
pub const MAX_DEBOUNCE_WAIT: Duration = Duration::from_millis(500);

#[derive(Debug)]
struct DirAccumulator {
    is_overflow: bool,
    changes: Vec<WatchChange>,
    seen_paths: HashSet<PathBuf>,
    first_seen: Instant,
    last_seen: Instant,
}

impl DirAccumulator {
    fn new(now: Instant) -> Self {
        Self {
            is_overflow: false,
            changes: Vec::new(),
            seen_paths: HashSet::new(),
            first_seen: now,
            last_seen: now,
        }
    }

    fn add_change(&mut self, change: WatchChange, now: Instant) {
        self.last_seen = now;
        if self.seen_paths.insert(change.path.clone()) {
            self.changes.push(change);
        }
    }

    fn mark_overflow(&mut self, now: Instant) {
        self.last_seen = now;
        self.is_overflow = true;
        self.changes.clear();
        self.seen_paths.clear();
    }
}

/// Accumulates raw watcher events, deduplicates them within a debounce window,
/// and broadcasts coalesced `WatchNotification` batches.
pub struct EventCoalescer {
    watched_paths: Arc<RwLock<HashSet<PathBuf>>>,
    dirty_dirs: HashMap<PathBuf, DirAccumulator>,
    overflow_all: Option<DirAccumulator>,
    out_tx: broadcast::Sender<WatchNotification>,
}

impl EventCoalescer {
    pub fn new(
        watched_paths: Arc<RwLock<HashSet<PathBuf>>>,
        out_tx: broadcast::Sender<WatchNotification>,
    ) -> Self {
        Self {
            watched_paths,
            dirty_dirs: HashMap::new(),
            overflow_all: None,
            out_tx,
        }
    }

    /// Process a single raw event into dirty directory state.
    pub fn ingest_event(&mut self, event: RawWatchEvent, now: Instant) {
        if let Some(acc) = self.overflow_all.as_mut() {
            acc.last_seen = now;
            return;
        }

        let watched = {
            let guard = self.watched_paths.read().unwrap();
            guard.clone()
        };

        if event.is_overflow || event.kind == WatchEventKind::Overflow {
            // If paths are specified, mark them overflow; otherwise mark all active watches
            if event.paths.is_empty() {
                self.mark_all_overflow(now);
            } else {
                for p in &event.paths {
                    if let Some(dir) = find_best_matching_root(p, &watched) {
                        self.mark_dir_overflow(dir, now);
                        if self.overflow_all.is_some() {
                            return;
                        }
                    }
                }
            }
            return;
        }

        for path in event.paths {
            if let Some(dir) = find_best_matching_root(&path, &watched) {
                if self.dirty_dirs.len() >= MAX_DIRTY_DIRS && !self.dirty_dirs.contains_key(&dir) {
                    warn!(
                        "Exceeded maximum dirty directories ({MAX_DIRTY_DIRS}); scheduling full watched-path reconciliation"
                    );
                    self.mark_all_overflow(now);
                    return;
                }

                let entry = self
                    .dirty_dirs
                    .entry(dir)
                    .or_insert_with(|| DirAccumulator::new(now));

                if !entry.is_overflow {
                    entry.add_change(
                        WatchChange {
                            path: path.clone(),
                            path_utf16: path.as_os_str().encode_wide().collect(),
                            kind: event.kind,
                        },
                        now,
                    );
                }
            }
        }
    }

    fn mark_dir_overflow(&mut self, dir: PathBuf, now: Instant) {
        if !self.dirty_dirs.contains_key(&dir) && self.dirty_dirs.len() >= MAX_DIRTY_DIRS {
            self.mark_all_overflow(now);
            return;
        }
        let entry = self
            .dirty_dirs
            .entry(dir)
            .or_insert_with(|| DirAccumulator::new(now));
        entry.mark_overflow(now);
    }

    fn mark_all_overflow(&mut self, now: Instant) {
        self.dirty_dirs.clear();
        match self.overflow_all.as_mut() {
            Some(acc) => acc.mark_overflow(now),
            None => {
                let mut acc = DirAccumulator::new(now);
                acc.mark_overflow(now);
                self.overflow_all = Some(acc);
            }
        }
    }

    /// Check if any accumulated directory is ready to flush based on debounce duration.
    pub fn should_flush(&self, now: Instant) -> bool {
        if self.dirty_dirs.is_empty() && self.overflow_all.is_none() {
            return false;
        }

        // Flush if the oldest entry has exceeded MAX_DEBOUNCE_WAIT
        // or if all dirty entries have been quiescent for DEBOUNCE_DURATION
        let mut any_quiescent = false;
        let mut any_exceeded_max = false;

        if let Some(acc) = &self.overflow_all {
            any_exceeded_max = now.duration_since(acc.first_seen) >= MAX_DEBOUNCE_WAIT;
            any_quiescent = now.duration_since(acc.last_seen) >= DEBOUNCE_DURATION;
        }

        for acc in self.dirty_dirs.values() {
            if now.duration_since(acc.first_seen) >= MAX_DEBOUNCE_WAIT {
                any_exceeded_max = true;
                break;
            }
            if now.duration_since(acc.last_seen) >= DEBOUNCE_DURATION {
                any_quiescent = true;
            }
        }

        any_exceeded_max || any_quiescent
    }

    /// Flushes quiescent or expired directories, returning generated notifications.
    pub fn flush_ready(&mut self, now: Instant) -> Vec<WatchNotification> {
        let mut flushed = Vec::new();
        let flush_global = self.overflow_all.as_ref().is_some_and(|acc| {
            now.duration_since(acc.last_seen) >= DEBOUNCE_DURATION
                || now.duration_since(acc.first_seen) >= MAX_DEBOUNCE_WAIT
        });
        if flush_global {
            self.overflow_all = None;
            let watched = self.watched_paths.read().unwrap().clone();
            for dir_path in watched {
                let notif = make_notification(dir_path, true, Vec::new());
                let _ = self.out_tx.send(notif.clone());
                flushed.push(notif);
            }
            return flushed;
        }

        let mut ready_keys = Vec::new();

        for (dir, acc) in &self.dirty_dirs {
            let quiescent = now.duration_since(acc.last_seen) >= DEBOUNCE_DURATION;
            let expired = now.duration_since(acc.first_seen) >= MAX_DEBOUNCE_WAIT;

            if quiescent || expired {
                ready_keys.push(dir.clone());
            }
        }

        for key in ready_keys {
            if let Some(acc) = self.dirty_dirs.remove(&key) {
                let notif = make_notification(key, acc.is_overflow, acc.changes);
                let _ = self.out_tx.send(notif.clone());
                flushed.push(notif);
            }
        }

        flushed
    }

    /// Flushes all dirty directories immediately regardless of timers.
    pub fn flush_all(&mut self) -> Vec<WatchNotification> {
        let mut flushed = Vec::new();
        if self.overflow_all.take().is_some() {
            let watched = self.watched_paths.read().unwrap().clone();
            for dir_path in watched {
                let notif = make_notification(dir_path, true, Vec::new());
                let _ = self.out_tx.send(notif.clone());
                flushed.push(notif);
            }
        }
        for (dir, acc) in self.dirty_dirs.drain() {
            let notif = make_notification(dir, acc.is_overflow, acc.changes);
            let _ = self.out_tx.send(notif.clone());
            flushed.push(notif);
        }
        flushed
    }

    /// Runs the async coalescer event loop consuming raw events from `rx`.
    pub async fn run_loop(
        mut self,
        mut rx: Receiver<RawWatchEvent>,
        overflow_signal: Arc<AtomicBool>,
    ) {
        debug!("Starting event coalescer loop");
        let mut ticker = tokio::time::interval(Duration::from_millis(50));

        loop {
            tokio::select! {
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            self.ingest_event(event, Instant::now());
                        }
                        None => {
                            debug!("Raw watch event channel closed; stopping coalescer loop");
                            self.flush_all();
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    let now = Instant::now();
                    if overflow_signal.swap(false, Ordering::SeqCst) {
                        self.mark_all_overflow(now);
                    }
                    if self.should_flush(now) {
                        self.flush_ready(now);
                    }
                }
            }
        }
    }
}

fn make_notification(
    dir_path: PathBuf,
    is_overflow: bool,
    changes: Vec<WatchChange>,
) -> WatchNotification {
    WatchNotification {
        dir_path_display: dir_path.to_string_lossy().into_owned(),
        dir_path_utf16: dir_path.as_os_str().encode_wide().collect(),
        dir_path,
        is_overflow,
        changes,
    }
}

/// Helper function to match an event path with its most specific registered watched directory.
fn find_best_matching_root(path: &Path, watched: &HashSet<PathBuf>) -> Option<PathBuf> {
    // If the path itself is watched (common for directory changes)
    if watched.contains(path) {
        return Some(path.to_path_buf());
    }

    // Check parent ancestry
    let mut current = path.parent();
    while let Some(parent) = current {
        if watched.contains(parent) {
            return Some(parent.to_path_buf());
        }
        current = parent.parent();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_coalescing_and_deduplication() {
        let watched = Arc::new(RwLock::new(HashSet::new()));
        let dir = PathBuf::from(r"C:\test\watched");
        watched.write().unwrap().insert(dir.clone());

        let (tx, _rx) = broadcast::channel(16);
        let mut coalescer = EventCoalescer::new(watched, tx);
        let start = Instant::now();

        // 1. Ingest multiple changes to the same file
        let file = dir.join("file.txt");
        coalescer.ingest_event(
            RawWatchEvent {
                paths: vec![file.clone()],
                kind: WatchEventKind::Create,
                is_overflow: false,
            },
            start,
        );
        coalescer.ingest_event(
            RawWatchEvent {
                paths: vec![file.clone()],
                kind: WatchEventKind::Modify,
                is_overflow: false,
            },
            start + Duration::from_millis(20),
        );

        // Should not flush yet before debounce window
        assert!(!coalescer.should_flush(start + Duration::from_millis(50)));

        // After debounce duration (150ms after last event = 170ms)
        let later = start + Duration::from_millis(180);
        assert!(coalescer.should_flush(later));

        let flushed = coalescer.flush_ready(later);
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].dir_path, dir);
        assert!(!flushed[0].is_overflow);
        // Deduplicated item changes
        assert_eq!(flushed[0].changes.len(), 1);
        assert_eq!(flushed[0].changes[0].path, file);
    }

    #[test]
    fn test_overflow_event_clears_changes() {
        let watched = Arc::new(RwLock::new(HashSet::new()));
        let dir = PathBuf::from(r"C:\test\watched");
        watched.write().unwrap().insert(dir.clone());

        let (tx, _rx) = broadcast::channel(16);
        let mut coalescer = EventCoalescer::new(watched, tx);
        let start = Instant::now();

        coalescer.ingest_event(
            RawWatchEvent {
                paths: vec![dir.join("file.txt")],
                kind: WatchEventKind::Modify,
                is_overflow: false,
            },
            start,
        );

        // Ingest overflow
        coalescer.ingest_event(
            RawWatchEvent {
                paths: vec![dir.clone()],
                kind: WatchEventKind::Overflow,
                is_overflow: true,
            },
            start + Duration::from_millis(10),
        );

        let later = start + Duration::from_millis(200);
        let flushed = coalescer.flush_ready(later);
        assert_eq!(flushed.len(), 1);
        assert!(flushed[0].is_overflow);
        assert!(flushed[0].changes.is_empty());
    }
}
