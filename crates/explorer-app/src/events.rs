use explorer_watch::events::WatchNotification;
use std::path::PathBuf;

/// Constructs a synthetic overflow notification used to trigger UI refresh and index reconciliation
/// when the notification receiver lags or drops messages.
pub fn create_overflow_notification() -> WatchNotification {
    WatchNotification {
        dir_path: PathBuf::new(),
        dir_path_display: String::new(),
        dir_path_utf16: Vec::new(),
        is_overflow: true,
        changes: Vec::new(),
    }
}
