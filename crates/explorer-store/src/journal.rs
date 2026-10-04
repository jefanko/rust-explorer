//! Job journal persistence and recovery states

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{JobId, PlanId};
use explorer_domain::operations::{ItemOutcome, ItemStatus, JobState, JobSummary, OperationKind};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct JobJournal {
    conn: Mutex<Connection>,
}

impl JobJournal {
    pub fn open(db_path: &Path) -> Result<Self, ExplorerError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Cannot create journal directory: {e}"),
                    "JobJournal::open",
                )
            })?;
        }

        let conn = Connection::open(db_path).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open journal database: {e}"),
                "JobJournal::open",
            )
        })?;

        let journal = Self {
            conn: Mutex::new(conn),
        };
        journal.init_schema()?;
        Ok(journal)
    }

    pub fn open_in_memory() -> Result<Self, ExplorerError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open in-memory database: {e}"),
                "JobJournal::open_in_memory",
            )
        })?;

        let journal = Self {
            conn: Mutex::new(conn),
        };
        journal.init_schema()?;
        Ok(journal)
    }

    fn init_schema(&self) -> Result<(), ExplorerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "init_schema"))?;

        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Cannot set journal busy timeout: {e}"),
                    "JobJournal::init_schema",
                )
            })?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Cannot configure durable journal: {e}"),
                "JobJournal::init_schema",
            )
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS job_journal (
                job_id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                state TEXT NOT NULL,
                total_items INTEGER NOT NULL,
                completed_items INTEGER NOT NULL,
                failed_items INTEGER NOT NULL,
                canceled_items INTEGER NOT NULL DEFAULT 0,
                skipped_items INTEGER NOT NULL DEFAULT 0,
                item_outcomes TEXT NOT NULL DEFAULT '[]',
                error_message TEXT,
                created_epoch INTEGER NOT NULL,
                updated_epoch INTEGER NOT NULL
            );",
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to initialize job_journal table: {e}"),
                "JobJournal::init_schema",
            )
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS job_item_outcomes (
                job_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL,
                status TEXT NOT NULL,
                outcome_json TEXT NOT NULL,
                PRIMARY KEY(job_id, ordinal)
            );",
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to initialize item outcome table: {e}"),
                "JobJournal::init_schema",
            )
        })?;

        let columns = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(job_journal)")
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to inspect job journal schema: {e}"),
                        "JobJournal::init_schema",
                    )
                })?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to read job journal schema: {e}"),
                        "JobJournal::init_schema",
                    )
                })?;
            rows.flatten().collect::<std::collections::HashSet<_>>()
        };
        for (column, definition) in [
            ("canceled_items", "INTEGER NOT NULL DEFAULT 0"),
            ("skipped_items", "INTEGER NOT NULL DEFAULT 0"),
            ("item_outcomes", "TEXT NOT NULL DEFAULT '[]'"),
        ] {
            if !columns.contains(column) {
                conn.execute_batch(&format!(
                    "ALTER TABLE job_journal ADD COLUMN {column} {definition};"
                ))
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to migrate job journal column {column}: {e}"),
                        "JobJournal::init_schema",
                    )
                })?;
            }
        }

        Ok(())
    }

    pub fn record_job(&self, job: &JobSummary) -> Result<(), ExplorerError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "record_job"))?;

        let kind_str = serde_json::to_string(&job.kind)
            .unwrap_or_default()
            .replace('"', "");
        let state_str = serde_json::to_string(&job.state)
            .unwrap_or_default()
            .replace('"', "");
        let item_outcomes = serde_json::to_string(&job.item_outcomes).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to serialize item outcomes: {e}"),
                "JobJournal::record_job",
            )
        })?;
        let tx = conn.transaction().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to begin job journal transaction: {e}"),
                "JobJournal::record_job",
            )
        })?;

        tx.execute(
            "INSERT INTO job_journal (
                job_id, plan_id, kind, state, total_items, completed_items, failed_items,
                canceled_items, skipped_items, item_outcomes, error_message, created_epoch, updated_epoch
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(job_id) DO UPDATE SET
                state = ?4,
                total_items = ?5,
                completed_items = ?6,
                failed_items = ?7,
                canceled_items = ?8,
                skipped_items = ?9,
                item_outcomes = ?10,
                error_message = ?11,
                updated_epoch = ?13;",
            params![
                job.id.0,
                job.plan_id.0,
                kind_str,
                state_str,
                job.total_items as i64,
                job.completed_items as i64,
                job.failed_items as i64,
                job.canceled_items as i64,
                job.skipped_items as i64,
                item_outcomes,
                job.error_message,
                job.created_at_epoch as i64,
                job.updated_at_epoch as i64,
            ],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to record job: {e}"),
                "JobJournal::record_job",
            )
        })?;

        tx.execute(
            "DELETE FROM job_item_outcomes WHERE job_id = ?1",
            params![job.id.0],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to replace item outcomes: {e}"),
                "JobJournal::record_job",
            )
        })?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO job_item_outcomes (job_id, ordinal, status, outcome_json)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to prepare item outcome insert: {e}"),
                        "JobJournal::record_job",
                    )
                })?;
            for (ordinal, outcome) in job.item_outcomes.iter().enumerate() {
                let outcome_json = serde_json::to_string(outcome).map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to serialize an item outcome: {e}"),
                        "JobJournal::record_job",
                    )
                })?;
                let status = serde_json::to_string(&outcome.status)
                    .unwrap_or_default()
                    .replace('"', "");
                stmt.execute(params![job.id.0, ordinal as i64, status, outcome_json])
                    .map_err(|e| {
                        ExplorerError::new(
                            ErrorCode::Internal,
                            format!("Failed to persist item outcome: {e}"),
                            "JobJournal::record_job",
                        )
                    })?;
            }
        }
        tx.commit().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to commit job journal transaction: {e}"),
                "JobJournal::record_job",
            )
        })?;
        Ok(())
    }

    pub fn record_item_outcome(
        &self,
        job_id: &JobId,
        ordinal: usize,
        outcome: &ItemOutcome,
    ) -> Result<(), ExplorerError> {
        let outcome_json = serde_json::to_string(outcome).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to serialize an item outcome: {e}"),
                "JobJournal::record_item_outcome",
            )
        })?;
        let status = serde_json::to_string(&outcome.status)
            .unwrap_or_default()
            .replace('"', "");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "record_item_outcome")
        })?;
        let tx = conn.transaction().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to begin item outcome transaction: {e}"),
                "JobJournal::record_item_outcome",
            )
        })?;
        tx.execute(
            "INSERT INTO job_item_outcomes (job_id, ordinal, status, outcome_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(job_id, ordinal) DO UPDATE SET
                status = excluded.status,
                outcome_json = excluded.outcome_json",
            params![job_id.0, ordinal as i64, status, outcome_json],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to persist item outcome: {e}"),
                "JobJournal::record_item_outcome",
            )
        })?;

        let counts = tx
            .query_row(
                "SELECT
                    COALESCE(SUM(status = 'succeeded'), 0),
                    COALESCE(SUM(status = 'failed'), 0),
                    COALESCE(SUM(status = 'canceled'), 0),
                    COALESCE(SUM(status = 'skipped'), 0)
                 FROM job_item_outcomes WHERE job_id = ?1",
                params![job_id.0],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to aggregate item outcomes: {e}"),
                    "JobJournal::record_item_outcome",
                )
            })?;
        tx.execute(
            "UPDATE job_journal SET
                completed_items = ?1,
                failed_items = ?2,
                canceled_items = ?3,
                skipped_items = ?4,
                updated_epoch = ?5
             WHERE job_id = ?6",
            params![counts.0, counts.1, counts.2, counts.3, now, job_id.0],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to update job outcome counts: {e}"),
                "JobJournal::record_item_outcome",
            )
        })?;
        tx.commit().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to commit item outcome: {e}"),
                "JobJournal::record_item_outcome",
            )
        })?;
        Ok(())
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobSummary>, ExplorerError> {
        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "list_recent_jobs")
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT job_id, plan_id, kind, state, total_items, completed_items, failed_items,
                        canceled_items, skipped_items, item_outcomes, error_message, created_epoch, updated_epoch
                 FROM job_journal ORDER BY created_epoch DESC LIMIT ?1;",
            )
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to prepare query: {e}"),
                    "JobJournal::list_recent_jobs",
                )
            })?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let job_id: String = row.get(0)?;
                let plan_id: String = row.get(1)?;
                let kind_str: String = row.get(2)?;
                let state_str: String = row.get(3)?;
                let total_items: i64 = row.get(4)?;
                let completed_items: i64 = row.get(5)?;
                let failed_items: i64 = row.get(6)?;
                let canceled_items: i64 = row.get(7)?;
                let skipped_items: i64 = row.get(8)?;
                let item_outcomes_json: String = row.get(9)?;
                let error_message: Option<String> = row.get(10)?;
                let created_epoch: i64 = row.get(11)?;
                let updated_epoch: i64 = row.get(12)?;

                let kind: OperationKind = serde_json::from_str(&format!("\"{kind_str}\""))
                    .unwrap_or(OperationKind::CreateFolder);
                let state: JobState = serde_json::from_str(&format!("\"{state_str}\""))
                    .unwrap_or(JobState::Interrupted);

                Ok(JobSummary {
                    id: JobId(job_id),
                    plan_id: PlanId(plan_id),
                    kind,
                    state,
                    total_items: total_items as usize,
                    completed_items: completed_items as usize,
                    failed_items: failed_items as usize,
                    canceled_items: canceled_items as usize,
                    skipped_items: skipped_items as usize,
                    item_outcomes: serde_json::from_str::<Vec<ItemOutcome>>(&item_outcomes_json)
                        .unwrap_or_default(),
                    error_message,
                    created_at_epoch: created_epoch as u64,
                    updated_at_epoch: updated_epoch as u64,
                })
            })
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Query error: {e}"),
                    "JobJournal::list_recent_jobs",
                )
            })?;

        let mut jobs = Vec::new();
        for j in rows.flatten() {
            jobs.push(j);
        }

        // The JSON snapshot is updated at terminal state; the item table also
        // contains outcomes persisted as Shell callbacks arrive. Merge those
        // rows so interrupted jobs retain the last durable per-item results.
        for job in &mut jobs {
            let item_rows = conn
                .prepare(
                    "SELECT outcome_json FROM job_item_outcomes
                     WHERE job_id = ?1 ORDER BY ordinal",
                )
                .and_then(|mut item_stmt| {
                    item_stmt
                        .query_map(params![job.id.0], |row| row.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to load item outcomes: {e}"),
                        "JobJournal::list_recent_jobs",
                    )
                })?;

            if !item_rows.is_empty() {
                job.item_outcomes = item_rows
                    .iter()
                    .map(|json| {
                        serde_json::from_str::<ItemOutcome>(json).map_err(|e| {
                            ExplorerError::new(
                                ErrorCode::Internal,
                                format!("Failed to decode a journaled item outcome: {e}"),
                                "JobJournal::list_recent_jobs",
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                job.completed_items = count_item_status(&job.item_outcomes, ItemStatus::Succeeded);
                job.failed_items = count_item_status(&job.item_outcomes, ItemStatus::Failed);
                job.canceled_items = count_item_status(&job.item_outcomes, ItemStatus::Canceled);
                job.skipped_items = count_item_status(&job.item_outcomes, ItemStatus::Skipped);
            }
        }

        Ok(jobs)
    }

    /// On startup: marks any unfinished jobs (`running`, `validating`, `queued`, `planned`) as `interrupted`.
    pub fn recover_interrupted_jobs(&self) -> Result<usize, ExplorerError> {
        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(
                ErrorCode::Internal,
                "Lock poisoned",
                "recover_interrupted_jobs",
            )
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let affected = conn
            .execute(
                "UPDATE job_journal
                 SET state = 'interrupted', updated_epoch = ?1
                 WHERE state IN ('running', 'validating', 'queued', 'planned');",
                params![now],
            )
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to update interrupted jobs: {e}"),
                    "JobJournal::recover_interrupted_jobs",
                )
            })?;

        Ok(affected)
    }
}

fn count_item_status(outcomes: &[ItemOutcome], status: ItemStatus) -> usize {
    outcomes
        .iter()
        .filter(|outcome| outcome.status == status)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_journal_survives_reopen_and_unwritable_location_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".rust-explorer-fixture-root"),
            "journal fixture",
        )
        .unwrap();
        let path = root.path().join("state.sqlite3");
        let job = JobSummary {
            id: JobId::new(),
            plan_id: PlanId::new(),
            kind: OperationKind::Copy,
            state: JobState::Running,
            total_items: 1,
            completed_items: 0,
            failed_items: 0,
            canceled_items: 0,
            skipped_items: 0,
            item_outcomes: vec![],
            error_message: None,
            created_at_epoch: 1,
            updated_at_epoch: 1,
        };
        {
            let journal = JobJournal::open(&path).unwrap();
            journal.record_job(&job).unwrap();
            journal
                .record_item_outcome(
                    &job.id,
                    0,
                    &ItemOutcome {
                        item_display: "fixture source".into(),
                        requested_destination_display: Some("requested".into()),
                        actual_destination_display: Some("actual collision output".into()),
                        status: ItemStatus::Succeeded,
                        native_code: Some(0),
                        error_message: None,
                    },
                )
                .unwrap();
        }
        let journal = JobJournal::open(&path).unwrap();
        assert_eq!(journal.recover_interrupted_jobs().unwrap(), 1);
        let recovered = journal.list_recent_jobs(1).unwrap().remove(0);
        assert_eq!(recovered.state, JobState::Interrupted);
        assert_eq!(recovered.completed_items, 1);
        assert_eq!(
            recovered.item_outcomes[0]
                .actual_destination_display
                .as_deref(),
            Some("actual collision output")
        );
        let file = root.path().join("not-a-directory");
        std::fs::write(&file, "intact").unwrap();
        assert!(JobJournal::open(&file.join("state.sqlite3")).is_err());
        assert_eq!(std::fs::read_to_string(file).unwrap(), "intact");
    }

    #[test]
    fn test_job_journal_lifecycle_and_interrupted_recovery() {
        let journal = JobJournal::open_in_memory().expect("open journal");

        let job = JobSummary {
            id: JobId("job_1".to_string()),
            plan_id: PlanId("plan_1".to_string()),
            kind: OperationKind::CreateFolder,
            state: JobState::Running,
            total_items: 1,
            completed_items: 0,
            failed_items: 0,
            canceled_items: 0,
            skipped_items: 0,
            item_outcomes: Vec::new(),
            error_message: None,
            created_at_epoch: 1000,
            updated_at_epoch: 1000,
        };

        journal.record_job(&job).expect("record job");

        let list = journal.list_recent_jobs(10).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, JobState::Running);

        // Crash recovery
        let affected = journal.recover_interrupted_jobs().expect("recover");
        assert_eq!(affected, 1);

        let recovered = journal.list_recent_jobs(10).expect("list recovered");
        assert_eq!(recovered[0].state, JobState::Interrupted);
    }
}
