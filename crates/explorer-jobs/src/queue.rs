//! Operation queue and high-level service

use crate::executor::JobExecutor;
use crate::planner::{plan_create_folder, plan_rename};
use explorer_domain::errors::ExplorerError;
use explorer_domain::operations::{JobSummary, OperationPlan};
use explorer_store::JobJournal;
use explorer_win::com::StaWorker;
use std::path::Path;
use std::sync::Arc;

pub struct OperationService {
    executor: JobExecutor,
}

impl OperationService {
    pub fn new(sta_worker: Arc<StaWorker>, journal: Arc<JobJournal>) -> Self {
        Self {
            executor: JobExecutor::new(sta_worker, journal),
        }
    }

    pub fn plan_create_folder(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<OperationPlan, ExplorerError> {
        plan_create_folder(parent, name)
    }

    pub fn plan_rename(
        &self,
        source: &Path,
        new_name: &str,
    ) -> Result<OperationPlan, ExplorerError> {
        plan_rename(source, new_name)
    }

    pub fn commit_plan(&self, plan: &OperationPlan) -> Result<JobSummary, ExplorerError> {
        self.executor.execute_plan(plan)
    }

    pub fn execute_create_folder(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_create_folder(parent, name)?;
        self.commit_plan(&plan)
    }

    pub fn execute_rename(
        &self,
        source: &Path,
        new_name: &str,
    ) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_rename(source, new_name)?;
        self.commit_plan(&plan)
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobSummary>, ExplorerError> {
        self.executor.list_recent_jobs(limit)
    }
}
