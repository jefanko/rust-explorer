use crate::ids::{ItemToken, JobId, PlanId};
use serde::{Deserialize, Serialize};

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
    pub kind: OperationKind,
    pub source_tokens: Vec<ItemToken>,
    pub target_folder_token: Option<ItemToken>,
    pub target_name: Option<String>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: JobId,
    pub plan_id: PlanId,
    pub kind: OperationKind,
    pub state: JobState,
    pub total_items: u64,
    pub completed_items: u64,
    pub failed_items: u64,
}
