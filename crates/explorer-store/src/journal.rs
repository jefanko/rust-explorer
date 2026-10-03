//! Job journal persistence and recovery states

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{JobId, PlanId};
use explorer_domain::operations::{JobState, JobSummary, OperationKind};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct JobJournal {
    conn: Mutex<Connection>,
}

impl JobJournal {
    pub fn open(db_path: &Path) -> Result<Self, ExplorerError> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
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

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS job_journal (
                job_id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                state TEXT NOT NULL,
                total_items INTEGER NOT NULL,
                completed_items INTEGER NOT NULL,
                failed_items INTEGER NOT NULL,
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

        Ok(())
    }

    pub fn record_job(&self, job: &JobSummary) -> Result<(), ExplorerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "record_job"))?;

        let kind_str = serde_json::to_string(&job.kind)
            .unwrap_or_default()
            .replace('"', "");
        let state_str = serde_json::to_string(&job.state)
            .unwrap_or_default()
            .replace('"', "");

        conn.execute(
            "INSERT INTO job_journal (
                job_id, plan_id, kind, state, total_items, completed_items, failed_items, error_message, created_epoch, updated_epoch
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(job_id) DO UPDATE SET
                state = ?4,
                total_items = ?5,
                completed_items = ?6,
                failed_items = ?7,
                error_message = ?8,
                updated_epoch = ?10;",
            params![
                job.id.0,
                job.plan_id.0,
                kind_str,
                state_str,
                job.total_items as i64,
                job.completed_items as i64,
                job.failed_items as i64,
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

        Ok(())
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobSummary>, ExplorerError> {
        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "list_recent_jobs")
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT job_id, plan_id, kind, state, total_items, completed_items, failed_items, error_message, created_epoch, updated_epoch
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
                let error_message: Option<String> = row.get(7)?;
                let created_epoch: i64 = row.get(8)?;
                let updated_epoch: i64 = row.get(9)?;

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

#[cfg(test)]
mod tests {
    use super::*;

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
