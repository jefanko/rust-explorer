use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Kind of filesystem change event observed by the watcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchEventKind {
    Create,
    Modify,
    Delete,
    Rename,
    Rescan,
    Overflow,
}

/// A single file or subdirectory change record within a watched directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchChange {
    pub path: PathBuf,
    pub kind: WatchEventKind,
}

/// Coalesced notification emitted to consumers (UI, indexing engine, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchNotification {
    /// Watched directory root that was affected.
    pub dir_path: PathBuf,
    /// Whether the watcher encountered a buffer overflow or queue drop,
    /// requiring the consumer to perform a complete directory re-enumeration.
    pub is_overflow: bool,
    /// Accumulated item-level changes within this coalescing window.
    pub changes: Vec<WatchChange>,
}

/// Watching recursion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchMode {
    /// Non-recursive watching (used for visible directory tabs).
    NonRecursive,
    /// Recursive watching (used for indexed roots).
    Recursive,
}

/// Current status of a watched directory path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Active,
    Degraded,
    Stopped,
}
