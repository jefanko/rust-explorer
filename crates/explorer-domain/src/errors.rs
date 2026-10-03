use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    AccessDenied,
    SharingViolation,
    AlreadyExists,
    InvalidName,
    PathTooLong,
    UnsupportedPath,
    UnsupportedReparsePoint,
    PlaceholderUnavailable,
    DiskFull,
    LocationOffline,
    RecycleUnsupported,
    StaleItem,
    Canceled,
    IndexUnavailable,
    WatcherDegraded,
    QueueFull,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[error("{user_message} (code: {code:?}, operation: {operation})")]
pub struct ExplorerError {
    pub code: ErrorCode,
    pub user_message: String,
    pub operation: String,
    pub retryable: bool,
    pub item_token: Option<String>,
    pub native_code: Option<u32>,
    pub correlation_id: String,
    pub safe_details: Option<String>,
}

impl ExplorerError {
    pub fn new(
        code: ErrorCode,
        user_message: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        Self {
            code,
            user_message: user_message.into(),
            operation: operation.into(),
            retryable: false,
            item_token: None,
            native_code: None,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            safe_details: None,
        }
    }
}
