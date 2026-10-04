//! Mutation execution and journal state transitions

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{CommitToken, JobId};
use explorer_domain::operations::{
    ItemOutcome, ItemStatus, JobState, JobSummary, OperationKind, OperationPlan,
};
use explorer_store::JobJournal;
use explorer_win::com::StaWorker;
use explorer_win::identity::get_file_identity;
use explorer_win::shell::{
    shell_copy_items_with_callback, shell_create_folder_with_callback,
    shell_move_items_with_callback, shell_recycle_items_with_callback,
    shell_rename_item_with_callback,
};
use explorer_win::sink::{SinkItemCallback, SinkItemResult, SinkReport};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct JobExecutor {
    sta_worker: Arc<StaWorker>,
    journal: Arc<JobJournal>,
    used_commit_tokens: Mutex<HashSet<CommitToken>>,
    _lifetime_guard: Option<Arc<dyn Send + Sync>>,
}

impl JobExecutor {
    pub fn new(sta_worker: Arc<StaWorker>, journal: Arc<JobJournal>) -> Self {
        Self {
            sta_worker,
            journal,
            used_commit_tokens: Mutex::new(HashSet::new()),
            _lifetime_guard: None,
        }
    }

    pub fn with_lifetime_guard(mut self, guard: Arc<dyn Send + Sync>) -> Self {
        self._lifetime_guard = Some(guard);
        self
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

        validate_plan_identity(plan)?;
        validate_plan_policy(plan)?;

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
            canceled_items: 0,
            skipped_items: 0,
            item_outcomes: Vec::new(),
            error_message: None,
            created_at_epoch: now,
            updated_at_epoch: now,
        };

        // Record initial running state in journal
        self.journal.record_job(&summary)?;

        let (callback, writer) =
            start_outcome_writer(self.journal.clone(), summary.id.clone(), plan.clone());
        let item_callback = Some(callback);
        let outcome_writer = Some(writer);

        // Execute via dedicated STA worker thread
        let result: Result<ExecutionReport, ExplorerError> = match plan.kind {
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

                self.execute_checked(plan, {
                    let callback = item_callback.clone();
                    move || shell_create_folder_with_callback(&parent, &name, callback)
                })
                .map(ExecutionReport::from)
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

                self.execute_checked(plan, {
                    let callback = item_callback.clone();
                    move || shell_rename_item_with_callback(&source, &new_name, callback)
                })
                .map(ExecutionReport::from)
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
                let callback = item_callback.clone();
                self.execute_checked(plan, move || {
                    shell_copy_items_with_callback(&sources, &dest, callback)
                })
                .map(ExecutionReport::from)
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
                let callback = item_callback.clone();
                self.execute_checked(plan, move || {
                    shell_move_items_with_callback(&sources, &dest, callback)
                })
                .map(ExecutionReport::from)
            }
            OperationKind::Recycle => {
                let sources = plan.source_paths.clone();
                let callback = item_callback.clone();
                self.execute_checked(plan, move || {
                    shell_recycle_items_with_callback(&sources, callback)
                })
                .map(ExecutionReport::from)
            }
        };

        // Drop the sender and drain every callback before writing terminal state.
        drop(item_callback);
        let incremental_journal_error = outcome_writer.and_then(|writer| match writer.join() {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.user_message.clone()),
            Err(_) => Some("Item outcome journal writer stopped unexpectedly".to_string()),
        });

        let finish_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        summary.updated_at_epoch = finish_epoch;

        let report = match result {
            Ok(report) => report,
            Err(error) => ExecutionReport::failure(plan.items_count, error),
        };
        summary.item_outcomes = build_item_outcomes(plan, &report);
        summary.total_items = summary.item_outcomes.len();
        summary.completed_items = summary
            .item_outcomes
            .iter()
            .filter(|item| item.status == ItemStatus::Succeeded)
            .count();
        summary.failed_items = summary
            .item_outcomes
            .iter()
            .filter(|item| item.status == ItemStatus::Failed)
            .count();
        summary.canceled_items = summary
            .item_outcomes
            .iter()
            .filter(|item| item.status == ItemStatus::Canceled)
            .count();
        summary.skipped_items = summary
            .item_outcomes
            .iter()
            .filter(|item| item.status == ItemStatus::Skipped)
            .count();
        summary.error_message = report.error_message.clone();
        if let Some(journal_error) = incremental_journal_error {
            let warning = format!(
                "Per-item results could not all be persisted as callbacks arrived; the terminal snapshot is being written now. {journal_error}"
            );
            summary.error_message = Some(match summary.error_message.take() {
                Some(operation_error) => format!("{operation_error}; {warning}"),
                None => warning,
            });
        }
        summary.state = if report.was_aborted || summary.canceled_items > 0 {
            JobState::Canceled
        } else if summary.failed_items > 0 || summary.skipped_items > 0 {
            if summary.completed_items > 0 {
                JobState::PartialFailure
            } else {
                JobState::Failed
            }
        } else {
            JobState::Succeeded
        };

        // Record terminal state in journal
        self.journal.record_job(&summary)?;

        Ok(summary)
    }

    fn execute_checked<T: Send + 'static>(
        &self,
        plan: &OperationPlan,
        operation: impl FnOnce() -> Result<T, ExplorerError> + Send + 'static,
    ) -> Result<T, ExplorerError> {
        let plan = plan.clone();
        self.sta_worker.execute(move || {
            validate_plan_identity(&plan)?;
            validate_plan_policy(&plan)?;
            operation()
        })
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobSummary>, ExplorerError> {
        self.journal.list_recent_jobs(limit)
    }
}

struct ExecutionReport {
    native_error: Option<u32>,
    failed: usize,
    was_aborted: bool,
    error_message: Option<String>,
    item_results: Vec<SinkItemResult>,
}

impl ExecutionReport {
    fn failure(item_count: usize, error: ExplorerError) -> Self {
        let was_aborted = error.code == ErrorCode::Canceled;
        Self {
            native_error: error.native_code,
            failed: if was_aborted { 0 } else { item_count },
            was_aborted,
            error_message: Some(error.user_message.clone()),
            item_results: Vec::new(),
        }
    }
}

impl From<SinkReport> for ExecutionReport {
    fn from(report: SinkReport) -> Self {
        Self {
            native_error: report.native_error,
            failed: report.failed,
            was_aborted: report.was_aborted,
            error_message: report.error_message,
            item_results: report.item_results,
        }
    }
}

fn build_item_outcomes(plan: &OperationPlan, report: &ExecutionReport) -> Vec<ItemOutcome> {
    let mut aggregate = OutcomeAccumulator::new(plan);
    for callback in &report.item_results {
        aggregate.apply(callback);
    }
    for item in &mut aggregate.items {
        if item.native_code.is_none() {
            item.native_code = report.native_error;
            item.status = if report.was_aborted {
                ItemStatus::Canceled
            } else if report.failed > 0 {
                ItemStatus::Failed
            } else {
                ItemStatus::Skipped
            };
            item.error_message = report.error_message.clone().or_else(|| Some("The Shell did not report completion for this source; inspect the output before retrying".into()));
        }
    }
    aggregate.items
}

struct OutcomeAccumulator {
    sources: Vec<std::path::PathBuf>,
    items: Vec<ItemOutcome>,
}
impl OutcomeAccumulator {
    fn new(plan: &OperationPlan) -> Self {
        let sources = if plan.kind == OperationKind::CreateFolder {
            plan.destination_path
                .as_ref()
                .zip(plan.target_name.as_ref())
                .map(|(p, n)| vec![p.join(n)])
                .unwrap_or_default()
        } else {
            plan.source_paths.clone()
        };
        Self {
            sources: sources
                .iter()
                .map(|p| explorer_win::path::ensure_extended_prefix(p))
                .collect(),
            items: plan_item_labels(plan)
                .into_iter()
                .map(
                    |(item_display, requested_destination_display)| ItemOutcome {
                        item_display,
                        requested_destination_display,
                        actual_destination_display: None,
                        status: ItemStatus::Skipped,
                        native_code: None,
                        error_message: None,
                    },
                )
                .collect(),
        }
    }
    fn apply(&mut self, result: &SinkItemResult) -> usize {
        let source = result
            .source_path
            .as_ref()
            .map(|p| explorer_win::path::ensure_extended_prefix(p));
        let index = source.as_ref().and_then(|source| {
            self.sources
                .iter()
                .position(|p| p == source)
                .or_else(|| self.sources.iter().position(|p| source.starts_with(p)))
        });
        let Some(index) = index else {
            let index = self.items.len();
            self.items.push(ItemOutcome {
                item_display: source.as_ref().map(|p| display_path(p)).unwrap_or_else(|| {
                    "Shell callback with unavailable source identity; inspect outputs".into()
                }),
                requested_destination_display: None,
                actual_destination_display: result
                    .actual_destination
                    .as_ref()
                    .map(|p| display_path(p)),
                status: if result.status == ItemStatus::Succeeded {
                    ItemStatus::Skipped
                } else {
                    result.status
                },
                native_code: result.native_code,
                error_message: result.error_message.clone().or_else(|| {
                    Some(
                        "Callback could not be bound to a selected source; no success inferred"
                            .into(),
                    )
                }),
            });
            return index;
        };
        let item = &mut self.items[index];
        let top_level = source.as_ref() == self.sources.get(index);
        let rank = |s| match s {
            ItemStatus::Succeeded => 0,
            ItemStatus::Skipped => 1,
            ItemStatus::Canceled => 2,
            ItemStatus::Failed => 3,
        };
        // Child failures/skip/cancel cannot be erased by a later successful parent callback.
        if (item.native_code.is_none() && (top_level || result.status != ItemStatus::Succeeded))
            || (item.native_code.is_some() && rank(result.status) >= rank(item.status))
        {
            item.status = result.status;
            item.native_code = result.native_code;
            item.error_message = result.error_message.clone().map(|message| {
                if top_level {
                    message
                } else {
                    format!(
                        "{}: {message}",
                        source.as_ref().map(|p| display_path(p)).unwrap_or_default()
                    )
                }
            });
        }
        if top_level {
            item.actual_destination_display =
                result.actual_destination.as_ref().map(|p| display_path(p));
        }
        index
    }
}

fn plan_item_labels(plan: &OperationPlan) -> Vec<(String, Option<String>)> {
    let requested_destination = plan
        .destination_path
        .as_ref()
        .map(|path| display_path(path));
    let mut items: Vec<(String, Option<String>)> = match plan.kind {
        OperationKind::CreateFolder => plan
            .destination_path
            .as_ref()
            .zip(plan.target_name.as_ref())
            .map(|(parent, name)| vec![(display_path(&parent.join(name)), None)])
            .unwrap_or_default(),
        OperationKind::Rename => plan
            .source_paths
            .first()
            .map(|source| {
                let destination = source
                    .parent()
                    .zip(plan.target_name.as_ref())
                    .map(|(parent, name)| display_path(&parent.join(name)));
                vec![(display_path(source), destination)]
            })
            .unwrap_or_default(),
        OperationKind::Copy | OperationKind::Move => plan
            .source_paths
            .iter()
            .map(|source| (display_path(source), requested_destination.clone()))
            .collect(),
        OperationKind::Recycle => plan
            .source_paths
            .iter()
            .map(|source| (display_path(source), None))
            .collect(),
    };

    items.truncate(plan.items_count);
    while items.len() < plan.items_count {
        items.push((
            format!("Operation item {}", items.len() + 1),
            requested_destination.clone(),
        ));
    }
    items
}

fn start_outcome_writer(
    journal: Arc<JobJournal>,
    job_id: JobId,
    plan: OperationPlan,
) -> (SinkItemCallback, JoinHandle<Result<(), ExplorerError>>) {
    let (sender, receiver) = mpsc::sync_channel::<SinkItemResult>(128);
    let callback: SinkItemCallback = Arc::new(move |result| {
        let _ = sender.send(result);
    });
    let writer = std::thread::spawn(move || {
        let mut aggregate = OutcomeAccumulator::new(&plan);
        for result in receiver {
            let index = aggregate.apply(&result);
            journal.record_item_outcome(&job_id, index, &aggregate.items[index])?;
        }
        Ok(())
    });
    (callback, writer)
}

fn display_path(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

fn validate_plan_identity(plan: &OperationPlan) -> Result<(), ExplorerError> {
    if plan.source_paths.len() != plan.source_identities.len()
        || plan.source_paths.len() != plan.source_parent_identities.len()
    {
        return Err(ExplorerError::new(
            ErrorCode::StaleItem,
            "Operation plan is missing source identity data; create a new plan",
            "JobExecutor::validate_plan_identity",
        ));
    }

    for (path, expected) in plan.source_paths.iter().zip(&plan.source_identities) {
        let current = get_file_identity(path)?;
        if &current != expected {
            return Err(ExplorerError::new(
                ErrorCode::StaleItem,
                format!("Source changed after planning: {}", path.display()),
                "JobExecutor::validate_plan_identity",
            ));
        }
    }

    for (path, expected) in plan.source_paths.iter().zip(&plan.source_parent_identities) {
        let parent = path.parent().ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::StaleItem,
                "Source parent is unavailable",
                "validate_plan_identity",
            )
        })?;
        let current = get_file_identity(parent)?;
        if current.volume_serial != expected.volume_serial
            || current.file_index != expected.file_index
        {
            return Err(ExplorerError::new(
                ErrorCode::StaleItem,
                "Source parent changed after planning",
                "validate_plan_identity",
            ));
        }
    }

    match (&plan.destination_path, &plan.destination_identity) {
        (Some(path), Some(expected)) => {
            let current = get_file_identity(path)?;
            if &current != expected {
                return Err(ExplorerError::new(
                    ErrorCode::StaleItem,
                    format!("Destination changed after planning: {}", path.display()),
                    "JobExecutor::validate_plan_identity",
                ));
            }
        }
        (None, None) => {}
        _ => {
            return Err(ExplorerError::new(
                ErrorCode::StaleItem,
                "Operation plan is missing destination identity data; create a new plan",
                "JobExecutor::validate_plan_identity",
            ));
        }
    }

    Ok(())
}

fn validate_plan_policy(plan: &OperationPlan) -> Result<(), ExplorerError> {
    explorer_fs::policy::MutationPolicy::for_application()?.prepare(
        plan.kind,
        &plan.source_paths,
        plan.destination_path.as_deref(),
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{plan_create_folder, plan_rename};
    fn tempdir() -> std::io::Result<tempfile::TempDir> {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/fixtures");
        std::fs::create_dir_all(&base)?;
        let dir = tempfile::Builder::new()
            .prefix("mutation fixture ")
            .tempdir_in(base.canonicalize()?)?;
        std::fs::write(
            dir.path().join(explorer_fs::policy::FIXTURE_MARKER),
            "rust-explorer mutation fixture",
        )?;
        Ok(dir)
    }

    #[test]
    fn test_executor_create_folder_and_rename_pipeline() {
        let dir = tempdir().expect("tempdir");
        let _instance = explorer_win::instance::InstanceLease::acquire(dir.path())
            .unwrap()
            .unwrap();
        let sta = Arc::new(StaWorker::new("test-exec-sta").expect("sta"));
        let journal =
            Arc::new(JobJournal::open(&dir.path().join("state.sqlite3")).expect("durable journal"));
        let executor = JobExecutor::new(sta, journal.clone());

        let parent = dir.path();

        // 1. Plan and execute folder creation
        let plan = plan_create_folder(parent, "PipelineFolder").expect("plan");
        let summary = executor.execute_plan(&plan).expect("execute");
        assert_eq!(summary.state, JobState::Succeeded, "{summary:#?}");
        assert_eq!(summary.completed_items, 1);
        assert!(parent.join("PipelineFolder").is_dir());

        // 2. Commit token idempotency check
        let duplicate_attempt = executor.execute_plan(&plan);
        assert!(duplicate_attempt.is_err());

        // 3. Plan and execute rename
        let created_dir = parent.join("PipelineFolder");
        let rename_plan = plan_rename(&created_dir, "RenamedPipelineFolder").expect("plan rename");
        let rename_summary = executor.execute_plan(&rename_plan).expect("execute rename");
        assert_eq!(
            rename_summary.state,
            JobState::Succeeded,
            "{rename_summary:#?}"
        );
        assert!(!created_dir.exists());
        assert!(parent.join("RenamedPipelineFolder").is_dir());

        // 4. Verify journal recorded these jobs
        let recent = executor.list_recent_jobs(5).expect("list");
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_executor_copy_move_and_recycle_pipeline() {
        use crate::planner::{plan_copy, plan_move, plan_recycle};

        let dir = tempdir().expect("tempdir");
        let _instance = explorer_win::instance::InstanceLease::acquire(dir.path())
            .unwrap()
            .unwrap();
        let sta = Arc::new(StaWorker::new("test-exec-m4").expect("sta"));
        let journal =
            Arc::new(JobJournal::open(&dir.path().join("state.sqlite3")).expect("durable journal"));
        let executor = JobExecutor::new(sta, journal);

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
        assert_eq!(copy_summary.state, JobState::Succeeded, "{copy_summary:#?}");
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
        assert_eq!(move_summary.state, JobState::Succeeded, "{move_summary:#?}");
        assert!(!renamed.exists());
        assert!(src_dir.join("work2.txt").exists());

        // 3. Recycle
        let recycle_target = src_dir.join("work2.txt");
        let rec_plan = plan_recycle(std::slice::from_ref(&recycle_target)).expect("plan recycle");
        let rec_summary = executor.execute_plan(&rec_plan).expect("execute recycle");
        if rec_summary.state == JobState::Succeeded {
            assert!(!recycle_target.exists());
        } else {
            assert_eq!(rec_summary.state, JobState::Failed, "{rec_summary:#?}");
            assert!(
                rec_summary
                    .error_message
                    .as_ref()
                    .unwrap()
                    .contains("Recycle support could not be verified"),
                "{rec_summary:#?}"
            );
            assert!(rec_summary.item_outcomes[0].native_code.is_some());
            assert_eq!(
                std::fs::read_to_string(&recycle_target).unwrap(),
                "test data"
            );
            eprintln!(
                "CAPABILITY SKIP: successful recycle is unavailable on this fixture volume; verified explicit failure and intact source: {:?}",
                rec_summary.item_outcomes[0].native_code
            );
        }
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;
    fn plan() -> OperationPlan {
        OperationPlan {
            id: explorer_domain::ids::PlanId::new(),
            commit_token: CommitToken::new(),
            kind: OperationKind::Copy,
            source_paths: vec![r"C:\fixture\a".into(), r"C:\fixture\b".into()],
            destination_path: Some(r"C:\out".into()),
            source_identities: vec![],
            source_parent_identities: vec![],
            destination_identity: None,
            target_name: None,
            items_count: 2,
            expires_at: u64::MAX,
        }
    }
    fn result(source: &str, status: ItemStatus, destination: Option<&str>) -> SinkItemResult {
        SinkItemResult {
            source_path: Some(source.into()),
            actual_destination: destination.map(Into::into),
            status,
            native_code: Some(if status == ItemStatus::Succeeded {
                0
            } else {
                0x80004005
            }),
            error_message: (status != ItemStatus::Succeeded).then(|| "native failure".into()),
        }
    }
    #[test]
    fn outcomes_follow_source_instead_of_callback_order_and_preserve_actual_output() {
        let report = ExecutionReport {
            native_error: None,
            failed: 1,
            was_aborted: false,
            error_message: None,
            item_results: vec![
                result(r"C:\fixture\b", ItemStatus::Failed, None),
                result(
                    r"C:\fixture\a",
                    ItemStatus::Succeeded,
                    Some(r"C:\out\a (2)"),
                ),
            ],
        };
        let outcomes = build_item_outcomes(&plan(), &report);
        assert_eq!(outcomes[0].status, ItemStatus::Succeeded);
        assert_eq!(
            outcomes[0].actual_destination_display.as_deref(),
            Some(r"C:\out\a (2)")
        );
        assert_eq!(outcomes[1].status, ItemStatus::Failed);
    }
    #[test]
    fn child_failure_survives_parent_success_and_unknown_callbacks_never_imply_success() {
        let report = ExecutionReport {
            native_error: None,
            failed: 1,
            was_aborted: false,
            error_message: None,
            item_results: vec![
                result(r"C:\fixture\a\child", ItemStatus::Failed, None),
                result(r"C:\fixture\a", ItemStatus::Succeeded, Some(r"C:\out\a")),
                result(r"C:\unknown", ItemStatus::Succeeded, None),
            ],
        };
        let outcomes = build_item_outcomes(&plan(), &report);
        assert_eq!(outcomes[0].status, ItemStatus::Failed);
        assert!(
            outcomes[0]
                .error_message
                .as_ref()
                .unwrap()
                .contains("child")
        );
        assert_eq!(outcomes[2].status, ItemStatus::Skipped);
    }
    #[test]
    fn incremental_journal_binds_out_of_order_sources() {
        let journal = Arc::new(JobJournal::open_in_memory().unwrap());
        let plan = plan();
        let job = JobSummary {
            id: JobId::new(),
            plan_id: plan.id.clone(),
            kind: plan.kind,
            state: JobState::Running,
            total_items: 2,
            completed_items: 0,
            failed_items: 0,
            canceled_items: 0,
            skipped_items: 0,
            item_outcomes: vec![],
            error_message: None,
            created_at_epoch: 1,
            updated_at_epoch: 1,
        };
        journal.record_job(&job).unwrap();
        let (callback, writer) = start_outcome_writer(journal.clone(), job.id.clone(), plan);
        callback(result(r"C:\fixture\b", ItemStatus::Failed, None));
        callback(result(
            r"C:\fixture\a",
            ItemStatus::Succeeded,
            Some(r"C:\out\a (2)"),
        ));
        drop(callback);
        writer.join().unwrap().unwrap();
        journal.recover_interrupted_jobs().unwrap();
        let recovered = journal.list_recent_jobs(1).unwrap().remove(0);
        assert_eq!(recovered.state, JobState::Interrupted);
        assert_eq!(recovered.item_outcomes[0].status, ItemStatus::Succeeded);
        assert!(recovered.item_outcomes[0].item_display.ends_with('a'));
        assert_eq!(recovered.item_outcomes[1].status, ItemStatus::Failed);
    }
    #[test]
    fn existing_directory_rename_native_probe() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(explorer_fs::policy::FIXTURE_MARKER),
            "directory probe",
        )
        .unwrap();
        let source = root.path().join("existing");
        std::fs::create_dir(&source).unwrap();
        let sta = Arc::new(StaWorker::new("native-probe-sta").unwrap());
        let _lease = explorer_win::instance::InstanceLease::acquire(root.path())
            .unwrap()
            .unwrap();
        let journal = Arc::new(JobJournal::open(&root.path().join("state.sqlite3")).unwrap());
        let executor = JobExecutor::new(sta, journal);
        let plan = crate::planner::plan_rename(&source, "renamed").unwrap();
        let summary = executor.execute_plan(&plan).unwrap();
        assert_eq!(summary.state, JobState::Succeeded, "{summary:#?}");
    }
    #[test]
    fn source_parent_swap_requires_replanning() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(explorer_fs::policy::FIXTURE_MARKER),
            "identity fixture",
        )
        .unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let source = parent.join("file");
        std::fs::write(&source, "identity").unwrap();
        let plan = crate::planner::plan_rename(&source, "renamed").unwrap();
        let original_parent = root.path().join("original-parent");
        std::fs::rename(&parent, &original_parent).unwrap();
        std::fs::create_dir(&parent).unwrap();
        std::fs::rename(original_parent.join("file"), &source).unwrap();
        assert_eq!(
            get_file_identity(&source).unwrap(),
            plan.source_identities[0]
        );
        assert_eq!(
            validate_plan_identity(&plan).unwrap_err().code,
            ErrorCode::StaleItem
        );
        assert!(source.exists());
        assert!(!parent.join("renamed").exists());
    }
}
