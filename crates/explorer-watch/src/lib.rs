//! Directory Watching Crate
//! Filesystem notification subscription, event coalescing, and rescan triggers.

pub mod adapter;
pub mod coalesce;
pub mod events;
pub mod reconcile;
pub mod service;

pub use events::{WatchChange, WatchEventKind, WatchMode, WatchNotification, WatchStatus};
pub use service::WatchService;
