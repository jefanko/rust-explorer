use explorer_domain::errors::ExplorerError;
use explorer_domain::ids::{FolderToken, ItemToken, SessionId};
use explorer_domain::models::{SortColumn, SortDirection};

/// Unique identity for matching asynchronous completions to active view contexts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RequestIdentity {
    pub session_id: SessionId,
    pub tab_id: String,
    pub request_id: u64,
    pub view_epoch: u64,
}

impl RequestIdentity {
    pub fn new(
        session_id: SessionId,
        tab_id: impl Into<String>,
        request_id: u64,
        view_epoch: u64,
    ) -> Self {
        Self {
            session_id,
            tab_id: tab_id.into(),
            request_id,
            view_epoch,
        }
    }

    /// Verifies if a completion matches the currently active view context.
    pub fn matches(&self, current: &RequestIdentity) -> bool {
        self.session_id == current.session_id
            && self.tab_id == current.tab_id
            && self.request_id == current.request_id
            && self.view_epoch == current.view_epoch
    }
}

/// Request for a bounded page of directory entries.
#[derive(Clone, Debug)]
pub struct PageRequest {
    pub identity: RequestIdentity,
    pub folder_token: FolderToken,
    pub generation: u64,
    pub offset: usize,
    pub limit: usize,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
}

/// Request for item preview data.
#[derive(Clone, Debug)]
pub struct PreviewRequest {
    pub identity: RequestIdentity,
    pub folder_token: FolderToken,
    pub item_token: ItemToken,
}

/// Request for indexed search.
#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub identity: RequestIdentity,
    pub query: String,
    pub root_id: Option<String>,
    pub page: usize,
    pub page_size: usize,
}

/// Typed envelope carrying an asynchronous result with its originating request identity.
#[derive(Clone, Debug)]
pub struct CompletionEnvelope<T> {
    pub identity: RequestIdentity,
    pub result: Result<T, ExplorerError>,
}

impl<T> CompletionEnvelope<T> {
    pub fn ok(identity: RequestIdentity, value: T) -> Self {
        Self {
            identity,
            result: Ok(value),
        }
    }

    pub fn err(identity: RequestIdentity, error: ExplorerError) -> Self {
        Self {
            identity,
            result: Err(error),
        }
    }

    pub fn is_stale(&self, current: &RequestIdentity) -> bool {
        !self.identity.matches(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_identity_stale_detection() {
        let session = SessionId::new();
        let id1 = RequestIdentity::new(session.clone(), "tab-1", 1, 1);
        let id1_same = RequestIdentity::new(session.clone(), "tab-1", 1, 1);
        let id2_diff_req = RequestIdentity::new(session.clone(), "tab-1", 2, 1);
        let id3_diff_epoch = RequestIdentity::new(session.clone(), "tab-1", 1, 2);
        let id4_diff_tab = RequestIdentity::new(session.clone(), "tab-2", 1, 1);

        assert!(id1.matches(&id1_same));
        assert!(!id1.matches(&id2_diff_req));
        assert!(!id1.matches(&id3_diff_epoch));
        assert!(!id1.matches(&id4_diff_tab));

        let comp = CompletionEnvelope::ok(id1.clone(), 42);
        assert!(!comp.is_stale(&id1_same));
        assert!(comp.is_stale(&id3_diff_epoch));
    }
}
