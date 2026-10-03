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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationPlan {
    pub id: PlanId,
    pub commit_token: CommitToken,
    pub kind: OperationKind,
    pub source_paths: Vec<PathBuf>,
    pub destination_path: Option<PathBuf>,
    pub target_name: Option<String>,
    pub items_count: usize,
    pub expires_at: u64,
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
    pub source_path: PathBuf,
    pub destination_path: Option<PathBuf>,
    pub status: ItemStatus,
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
    pub error_message: Option<String>,
    pub created_at_epoch: u64,
    pub updated_at_epoch: u64,
}
