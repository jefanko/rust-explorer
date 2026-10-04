use crate::ids::{CommitToken, JobId, PlanId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Copy,
    Move,
    Rename,
    CreateFolder,
    Recycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Planned,
    Queued,
    Validating,
    Running,
    Succeeded,
    PartialFailure,
    Failed,
    CancelRequested,
    Canceled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPlan {
    pub id: PlanId,
    #[serde(default, skip_serializing)]
    pub commit_token: CommitToken,
    pub kind: OperationKind,
    #[serde(default, skip_serializing)]
    pub source_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing)]
    pub destination_path: Option<PathBuf>,
    #[serde(default, skip_serializing)]
    pub source_identities: Vec<FileIdentity>,
    #[serde(default, skip_serializing)]
    pub source_parent_identities: Vec<FileIdentity>,
    #[serde(default, skip_serializing)]
    pub destination_identity: Option<FileIdentity>,
    pub target_name: Option<String>,
    pub items_count: usize,
    pub expires_at: u64,
}

/// Stable native identity plus basic metadata captured when a plan is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    pub volume_serial: u32,
    pub file_index: u64,
    pub size_bytes: u64,
    pub last_write_filetime: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Succeeded,
    Failed,
    Skipped,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemOutcome {
    /// Display-only item label. It is never accepted back as mutation authority.
    pub item_display: String,
    /// Requested destination folder/name, when the operation has one.
    pub requested_destination_display: Option<String>,
    /// Actual output returned by the Shell, including collision-renamed paths.
    #[serde(default)]
    pub actual_destination_display: Option<String>,
    pub status: ItemStatus,
    pub native_code: Option<u32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: JobId,
    pub plan_id: PlanId,
    pub kind: OperationKind,
    pub state: JobState,
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub canceled_items: usize,
    pub skipped_items: usize,
    pub item_outcomes: Vec<ItemOutcome>,
    pub error_message: Option<String>,
    pub created_at_epoch: u64,
    pub updated_at_epoch: u64,
}
