//! Operation queue and high-level service

use crate::executor::JobExecutor;
use crate::planner::{plan_copy, plan_create_folder, plan_move, plan_recycle, plan_rename};
use explorer_domain::errors::ExplorerError;
use explorer_domain::ids::PlanId;
use explorer_domain::operations::{JobSummary, OperationPlan};
use explorer_store::JobJournal;
use explorer_win::com::StaWorker;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PENDING_PLANS: usize = 256;
type CommitResultCache = HashMap<PlanId, (Result<JobSummary, ExplorerError>, u64)>;

pub struct OperationService {
    executor: JobExecutor,
    plans: Mutex<HashMap<PlanId, OperationPlan>>,
    completed_commits: Mutex<CommitResultCache>,
}

impl OperationService {
    pub fn new_guarded(
        sta_worker: Arc<StaWorker>,
        journal: Arc<JobJournal>,
        guard: Arc<dyn Send + Sync>,
    ) -> Self {
        Self {
            executor: JobExecutor::new(sta_worker, journal).with_lifetime_guard(guard),
            plans: Mutex::new(HashMap::new()),
            completed_commits: Mutex::new(HashMap::new()),
        }
    }
    pub fn new(sta_worker: Arc<StaWorker>, journal: Arc<JobJournal>) -> Self {
        Self {
            executor: JobExecutor::new(sta_worker, journal),
            plans: Mutex::new(HashMap::new()),
            completed_commits: Mutex::new(HashMap::new()),
        }
    }

    fn retain_plan(&self, plan: OperationPlan) -> Result<OperationPlan, ExplorerError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut plans = self.plans.lock().map_err(|_| {
            ExplorerError::new(
                explorer_domain::errors::ErrorCode::Internal,
                "Plan registry lock poisoned",
                "OperationService::retain_plan",
            )
        })?;
        plans.retain(|_, saved| saved.expires_at >= now);
        if plans.len() >= MAX_PENDING_PLANS {
            return Err(ExplorerError::new(
                explorer_domain::errors::ErrorCode::QueueFull,
                "Too many uncommitted plans; commit or discard an existing plan first",
                "OperationService::retain_plan",
            ));
        }
        plans.insert(plan.id.clone(), plan.clone());
        Ok(plan)
    }

    pub fn plan_create_folder(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<OperationPlan, ExplorerError> {
        self.retain_plan(plan_create_folder(parent, name)?)
    }

    pub fn plan_rename(
        &self,
        source: &Path,
        new_name: &str,
    ) -> Result<OperationPlan, ExplorerError> {
        self.retain_plan(plan_rename(source, new_name)?)
    }

    pub fn plan_copy(
        &self,
        sources: &[PathBuf],
        destination: &Path,
    ) -> Result<OperationPlan, ExplorerError> {
        self.retain_plan(plan_copy(sources, destination)?)
    }

    pub fn plan_move(
        &self,
        sources: &[PathBuf],
        destination: &Path,
    ) -> Result<OperationPlan, ExplorerError> {
        self.retain_plan(plan_move(sources, destination)?)
    }

    pub fn plan_recycle(&self, sources: &[PathBuf]) -> Result<OperationPlan, ExplorerError> {
        self.retain_plan(plan_recycle(sources)?)
    }

    /// Commits only a plan created and retained by this process. The caller never supplies
    /// filesystem paths, operation kind, expiry, or the commit token at commit time.
    pub fn commit_plan(&self, plan_id: &PlanId) -> Result<JobSummary, ExplorerError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut completed = self.completed_commits.lock().map_err(|_| {
            ExplorerError::new(
                explorer_domain::errors::ErrorCode::Internal,
                "Commit result registry lock poisoned",
                "OperationService::commit_plan",
            )
        })?;
        completed.retain(|_, (_, expires_at)| *expires_at >= now);
        if let Some((result, _)) = completed.get(plan_id) {
            return result.clone();
        }

        let plan = self
            .plans
            .lock()
            .map_err(|_| {
                ExplorerError::new(
                    explorer_domain::errors::ErrorCode::Internal,
                    "Plan registry lock poisoned",
                    "OperationService::commit_plan",
                )
            })?
            .remove(plan_id)
            .ok_or_else(|| {
                ExplorerError::new(
                    explorer_domain::errors::ErrorCode::StaleItem,
                    "Operation plan is unknown, expired, or already committed",
                    "OperationService::commit_plan",
                )
            })?;
        if plan.expires_at < now {
            let result = Err(ExplorerError::new(
                explorer_domain::errors::ErrorCode::StaleItem,
                "Operation plan has expired",
                "OperationService::commit_plan",
            ));
            completed.insert(plan.id.clone(), (result.clone(), now + 300));
            return result;
        }

        // Keep the result lock during execution so concurrent duplicate commits wait
        // for the first run and receive its exact job result instead of replaying it.
        let result = self.executor.execute_plan(&plan);
        if completed.len() >= MAX_PENDING_PLANS
            && let Some(oldest_key) = completed.keys().next().cloned()
        {
            completed.remove(&oldest_key);
        }
        let result_expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 300;
        completed.insert(plan.id.clone(), (result.clone(), result_expires_at));
        result
    }

    pub fn execute_create_folder(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_create_folder(parent, name)?;
        self.commit_plan(&plan.id)
    }

    pub fn execute_rename(
        &self,
        source: &Path,
        new_name: &str,
    ) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_rename(source, new_name)?;
        self.commit_plan(&plan.id)
    }

    pub fn execute_copy(
        &self,
        sources: &[PathBuf],
        destination: &Path,
    ) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_copy(sources, destination)?;
        self.commit_plan(&plan.id)
    }

    pub fn execute_move(
        &self,
        sources: &[PathBuf],
        destination: &Path,
    ) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_move(sources, destination)?;
        self.commit_plan(&plan.id)
    }

    pub fn execute_recycle(&self, sources: &[PathBuf]) -> Result<JobSummary, ExplorerError> {
        let plan = self.plan_recycle(sources)?;
        self.commit_plan(&plan.id)
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobSummary>, ExplorerError> {
        self.executor.list_recent_jobs(limit)
    }
}
