//! Mutation execution and journal state transitions

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{CommitToken, JobId};
use explorer_domain::operations::{JobState, JobSummary, OperationKind, OperationPlan};
use explorer_store::JobJournal;
use explorer_win::com::StaWorker;
use explorer_win::shell::{
    shell_copy_items, shell_create_folder, shell_move_items, shell_recycle_items, shell_rename_item,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct JobExecutor {
    sta_worker: Arc<StaWorker>,
    journal: Arc<JobJournal>,
    used_commit_tokens: Mutex<HashSet<CommitToken>>,
}

impl JobExecutor {
    pub fn new(sta_worker: Arc<StaWorker>, journal: Arc<JobJournal>) -> Self {
        Self {
            sta_worker,
            journal,
            used_commit_tokens: Mutex::new(HashSet::new()),
        }
    }

    /// Commits and executes an operation plan.
    pub fn execute_plan(&self, plan: &OperationPlan) -> Result<JobSummary, ExplorerError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Verify expiry
        if now > plan.expires_at {
            return Err(ExplorerError::new(
                ErrorCode::StaleItem,
                "Operation plan has expired. Please revalidate.",
                "JobExecutor::execute_plan",
            ));
        }

        // 2. Verify single-use commit token (idempotency)
        {
            let mut used = self.used_commit_tokens.lock().map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "execute_plan")
            })?;

            if used.contains(&plan.commit_token) {
                return Err(ExplorerError::new(
                    ErrorCode::InvalidName,
                    "Commit token has already been used",
                    "JobExecutor::execute_plan",
                ));
            }
            used.insert(plan.commit_token.clone());
        }

        let job_id = JobId::new();
        let mut summary = JobSummary {
            id: job_id,
            plan_id: plan.id.clone(),
            kind: plan.kind,
            state: JobState::Running,
            total_items: plan.items_count,
            completed_items: 0,
            failed_items: 0,
            error_message: None,
            created_at_epoch: now,
            updated_at_epoch: now,
        };

        // Record initial running state in journal
        self.journal.record_job(&summary)?;

        // Execute via dedicated STA worker thread
        let result: Result<usize, ExplorerError> = match plan.kind {
            OperationKind::CreateFolder => {
                let parent = plan.destination_path.clone().ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        "Missing destination path for CreateFolder",
                        "execute_plan",
                    )
                })?;
                let name = plan.target_name.clone().ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        "Missing folder name for CreateFolder",
                        "execute_plan",
                    )
                })?;

                self.sta_worker
                    .execute(move || shell_create_folder(&parent, &name))
                    .map(|_| 1)
            }
            OperationKind::Rename => {
                let source = plan.source_paths.first().cloned().ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        "Missing source path for Rename",
                        "execute_plan",
                    )
                })?;
                let new_name = plan.target_name.clone().ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        "Missing new name for Rename",
                        "execute_plan",
                    )
                })?;

                self.sta_worker
                    .execute(move || shell_rename_item(&source, &new_name))
                    .map(|_| 1)
            }
            OperationKind::Copy => {
                let sources = plan.source_paths.clone();
                let dest = plan.destination_path.clone().ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        "Missing destination path for Copy",
                        "execute_plan",
                    )
                })?;
                self.sta_worker
                    .execute(move || shell_copy_items(&sources, &dest))
                    .map(|r| r.completed)
            }
            OperationKind::Move => {
                let sources = plan.source_paths.clone();
                let dest = plan.destination_path.clone().ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        "Missing destination path for Move",
                        "execute_plan",
                    )
                })?;
                self.sta_worker
                    .execute(move || shell_move_items(&sources, &dest))
                    .map(|r| r.completed)
            }
            OperationKind::Recycle => {
                let sources = plan.source_paths.clone();
                self.sta_worker
                    .execute(move || shell_recycle_items(&sources))
                    .map(|r| r.completed)
            }
        };

        let finish_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        summary.updated_at_epoch = finish_epoch;

        match result {
            Ok(completed) => {
                summary.state = JobState::Succeeded;
                summary.completed_items = completed;
            }
            Err(e) => {
                if e.code == ErrorCode::Canceled {
                    summary.state = JobState::Canceled;
                } else {
                    summary.state = JobState::Failed;
                }
                summary.failed_items = plan.items_count;
                summary.error_message = Some(e.user_message.clone());
            }
        }

        // Record terminal state in journal
        self.journal.record_job(&summary)?;

        if summary.state == JobState::Failed {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                summary.error_message.unwrap_or_default(),
                "execute_plan",
            ));
        }

        Ok(summary)
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobSummary>, ExplorerError> {
        self.journal.list_recent_jobs(limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{plan_create_folder, plan_rename};
    use tempfile::tempdir;

    #[test]
    fn test_executor_create_folder_and_rename_pipeline() {
        let sta = Arc::new(StaWorker::new("test-exec-sta").expect("sta"));
        let journal = Arc::new(JobJournal::open_in_memory().expect("journal"));
        let executor = JobExecutor::new(sta, journal.clone());

        let dir = tempdir().expect("tempdir");
        let parent = dir.path();

        // 1. Plan and execute folder creation
        let plan = plan_create_folder(parent, "PipelineFolder").expect("plan");
        let summary = executor.execute_plan(&plan).expect("execute");
        assert_eq!(summary.state, JobState::Succeeded);
        assert_eq!(summary.completed_items, 1);
        assert!(parent.join("PipelineFolder").is_dir());

        // 2. Commit token idempotency check
        let duplicate_attempt = executor.execute_plan(&plan);
        assert!(duplicate_attempt.is_err());

        // 3. Plan and execute rename
        let created_dir = parent.join("PipelineFolder");
        let rename_plan = plan_rename(&created_dir, "RenamedPipelineFolder").expect("plan rename");
        let rename_summary = executor.execute_plan(&rename_plan).expect("execute rename");
        assert_eq!(rename_summary.state, JobState::Succeeded);
        assert!(!created_dir.exists());
        assert!(parent.join("RenamedPipelineFolder").is_dir());

        // 4. Verify journal recorded these jobs
        let recent = executor.list_recent_jobs(5).expect("list");
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_executor_copy_move_and_recycle_pipeline() {
        use crate::planner::{plan_copy, plan_move, plan_recycle};

        let sta = Arc::new(StaWorker::new("test-exec-m4").expect("sta"));
        let journal = Arc::new(JobJournal::open_in_memory().expect("journal"));
        let executor = JobExecutor::new(sta, journal);

        let dir = tempdir().expect("tempdir");
        let parent = dir.path();
        let src_dir = parent.join("src");
        let dst_dir = parent.join("dst");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::create_dir(&dst_dir).unwrap();

        let file = src_dir.join("work.txt");
        std::fs::write(&file, "test data").unwrap();

        // 1. Copy
        let copy_plan = plan_copy(std::slice::from_ref(&file), &dst_dir).expect("plan copy");
        let copy_summary = executor.execute_plan(&copy_plan).expect("execute copy");
        assert_eq!(copy_summary.state, JobState::Succeeded);
        assert_eq!(copy_summary.completed_items, 1);
        assert!(dst_dir.join("work.txt").exists());

        // 2. Move (from dst_dir to src_dir as work2.txt)
        let copied = dst_dir.join("work.txt");
        // Rename copied first so we can move it
        let plan_ren = plan_rename(&copied, "work2.txt").unwrap();
        executor.execute_plan(&plan_ren).unwrap();
        let renamed = dst_dir.join("work2.txt");

        let move_plan = plan_move(std::slice::from_ref(&renamed), &src_dir).expect("plan move");
        let move_summary = executor.execute_plan(&move_plan).expect("execute move");
        assert_eq!(move_summary.state, JobState::Succeeded);
        assert!(!renamed.exists());
        assert!(src_dir.join("work2.txt").exists());

        // 3. Recycle
        let recycle_target = src_dir.join("work2.txt");
        let rec_plan = plan_recycle(std::slice::from_ref(&recycle_target)).expect("plan recycle");
        let rec_summary = executor.execute_plan(&rec_plan).expect("execute recycle");
        assert_eq!(rec_summary.state, JobState::Succeeded);
        assert!(!recycle_target.exists());
    }
}
