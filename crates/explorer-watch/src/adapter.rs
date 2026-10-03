use crate::events::WatchEventKind;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::Sender;
use tracing::{debug, warn};

/// Raw change event extracted from the notify backend.
#[derive(Debug, Clone)]
pub struct RawWatchEvent {
    pub paths: Vec<PathBuf>,
    pub kind: WatchEventKind,
    pub is_overflow: bool,
}

/// Native file watcher adapter wrapping `notify::RecommendedWatcher`.
pub struct NotifyAdapter {
    watcher: RecommendedWatcher,
    overflow_flag: Arc<AtomicBool>,
}

impl NotifyAdapter {
    /// Creates a new `NotifyAdapter` that pushes raw events into the provided bounded channel.
    pub fn new(event_tx: Sender<RawWatchEvent>) -> Result<Self, ExplorerError> {
        let overflow_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = overflow_flag.clone();

        let watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    let kind = match event.kind {
                        EventKind::Create(_) => WatchEventKind::Create,
                        EventKind::Modify(_) => WatchEventKind::Modify,
                        EventKind::Remove(_) => WatchEventKind::Delete,
                        EventKind::Any | EventKind::Other => WatchEventKind::Rescan,
                        _ => WatchEventKind::Modify,
                    };

                    let raw = RawWatchEvent {
                        paths: event.paths,
                        kind,
                        is_overflow: false,
                    };

                    if let Err(e) = event_tx.try_send(raw) {
                        warn!(
                            "Watch event channel full (capacity 4096); setting overflow flag: {e}"
                        );
                        flag_clone.store(true, Ordering::SeqCst);
                        // Send overflow signal
                        let _ = event_tx.try_send(RawWatchEvent {
                            paths: Vec::new(),
                            kind: WatchEventKind::Overflow,
                            is_overflow: true,
                        });
                    }
                }
                Err(err) => {
                    warn!("Notify backend reported watch error (potential overflow): {err}");
                    flag_clone.store(true, Ordering::SeqCst);
                    let affected_paths = err.paths.clone();
                    let _ = event_tx.try_send(RawWatchEvent {
                        paths: affected_paths,
                        kind: WatchEventKind::Overflow,
                        is_overflow: true,
                    });
                }
            },
            Config::default(),
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to initialize Windows filesystem watcher: {e}"),
                "NotifyAdapter::new",
            )
        })?;

        Ok(Self {
            watcher,
            overflow_flag,
        })
    }

    /// Watch a directory path with the specified recursion mode.
    pub fn watch(&mut self, path: &Path, recursive: bool) -> Result<(), ExplorerError> {
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        debug!("Watching path {:?} (recursive: {})", path, recursive);
        self.watcher.watch(path, mode).map_err(|e| {
            ExplorerError::new(
                ErrorCode::WatcherDegraded,
                format!("Failed to watch directory {}: {e}", path.display()),
                "NotifyAdapter::watch",
            )
        })
    }

    /// Stop watching a directory path.
    pub fn unwatch(&mut self, path: &Path) -> Result<(), ExplorerError> {
        debug!("Unwatching path {:?}", path);
        self.watcher.unwatch(path).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to unwatch directory {}: {e}", path.display()),
                "NotifyAdapter::unwatch",
            )
        })
    }

    /// Check and clear the overflow flag.
    pub fn check_and_clear_overflow(&self) -> bool {
        self.overflow_flag.swap(false, Ordering::SeqCst)
    }
}
