use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_win::path::{path_to_wide, to_display_string, wide_to_path};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

pub const MAX_INDEXED_ROOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootState {
    NotIndexed,
    Scanning,
    Ready,
    Degraded,
    Offline,
    NeedsReconcile,
    Error,
}

impl RootState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotIndexed => "not_indexed",
            Self::Scanning => "scanning",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Offline => "offline",
            Self::NeedsReconcile => "needs_reconcile",
            Self::Error => "error",
        }
    }
}

impl std::str::FromStr for RootState {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "scanning" => Self::Scanning,
            "ready" => Self::Ready,
            "degraded" => Self::Degraded,
            "offline" => Self::Offline,
            "needs_reconcile" => Self::NeedsReconcile,
            "error" => Self::Error,
            _ => Self::NotIndexed,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedRoot {
    pub id: String,
    #[serde(default, skip_serializing)]
    pub path: PathBuf,
    pub path_utf16: Vec<u16>,
    pub display_path: String,
    pub state: RootState,
    pub completed_epoch: i64,
    pub reconciled_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndexEntryRecord {
    pub root_id: String,
    pub parent_id: Option<i64>,
    pub path: PathBuf,
    pub name_display: String,
    pub name_norm: String,
    pub extension_norm: String,
    pub kind: i32, // 0 = file, 1 = directory, 2 = reparse_point
    pub size_bytes: Option<u64>,
    pub modified_filetime: Option<u64>,
    pub attributes: u32,
    pub seen_epoch: i64,
}

/// Thread-safe wrapper around the SQLite index database connection.
pub struct IndexDb {
    conn: Arc<Mutex<Connection>>,
}

impl IndexDb {
    /// Opens the index database at the specified path and initializes the schema.
    pub fn open(path: &Path) -> Result<Self, ExplorerError> {
        let conn = Connection::open(path).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open index database at {}: {e}", path.display()),
                "IndexDb::open",
            )
        })?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Opens an in-memory index database for tests.
    pub fn open_in_memory() -> Result<Self, ExplorerError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open in-memory index database: {e}"),
                "IndexDb::open_in_memory",
            )
        })?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Initializes pragmas, metadata tables, FTS5 trigram table, and synchronization triggers.
    pub fn init_schema(&self) -> Result<(), ExplorerError> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;

            CREATE TABLE IF NOT EXISTS roots (
                id TEXT PRIMARY KEY,
                path_utf16le BLOB NOT NULL,
                display_path TEXT NOT NULL,
                volume_key BLOB,
                state TEXT NOT NULL,
                completed_epoch INTEGER NOT NULL DEFAULT 0,
                reconciled_at TEXT
            );

            CREATE TABLE IF NOT EXISTS entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                root_id TEXT NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
                parent_id INTEGER REFERENCES entries(id) ON DELETE CASCADE,
                path_utf16le BLOB NOT NULL,
                name_display TEXT NOT NULL,
                name_norm TEXT NOT NULL,
                extension_norm TEXT NOT NULL,
                kind INTEGER NOT NULL,
                size_bytes INTEGER,
                modified_filetime INTEGER,
                attributes INTEGER NOT NULL,
                file_identity BLOB,
                seen_epoch INTEGER NOT NULL,
                stale INTEGER NOT NULL DEFAULT 0,
                UNIQUE(root_id, path_utf16le)
            );

            CREATE INDEX IF NOT EXISTS entries_parent ON entries(root_id, parent_id);
            CREATE INDEX IF NOT EXISTS entries_extension ON entries(root_id, extension_norm, kind);
            CREATE INDEX IF NOT EXISTS entries_root_seen ON entries(root_id, seen_epoch);

            CREATE VIRTUAL TABLE IF NOT EXISTS filename_fts USING fts5(
                name_norm,
                tokenize='trigram'
            );

            CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
                INSERT INTO filename_fts(rowid, name_norm) VALUES (new.id, new.name_norm);
            END;

            CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
                DELETE FROM filename_fts WHERE rowid = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE OF name_norm ON entries BEGIN
                UPDATE filename_fts SET name_norm = new.name_norm WHERE rowid = old.id;
            END;
            ",
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to initialize index schema: {e}"),
                "IndexDb::init_schema",
            )
        })?;

        debug!("Initialized index database schema with FTS5 trigram virtual table");
        Ok(())
    }

    /// Add a new indexed root. Rejects overlap with existing roots and enforces the MAX_INDEXED_ROOTS bound.
    pub fn add_root(&self, path: &Path) -> Result<IndexedRoot, ExplorerError> {
        let canonical = normalize_root_path(path);
        let display_str = to_display_string(&canonical);
        let wide_bytes = wide_to_bytes(&path_to_wide(&canonical));

        let existing = self.list_roots()?;
        if existing.len() >= MAX_INDEXED_ROOTS {
            return Err(ExplorerError::new(
                ErrorCode::QueueFull,
                format!("Maximum {MAX_INDEXED_ROOTS} indexed roots permitted in MVP"),
                "IndexDb::add_root",
            ));
        }

        // Check duplicate and overlapping roots
        for r in &existing {
            if r.path == canonical {
                return Err(ExplorerError::new(
                    ErrorCode::AlreadyExists,
                    format!("Directory is already an indexed root: {display_str}"),
                    "IndexDb::add_root",
                ));
            }
            if canonical.starts_with(&r.path) {
                return Err(ExplorerError::new(
                    ErrorCode::AlreadyExists,
                    format!(
                        "Directory is inside existing indexed root ({}): {display_str}",
                        r.display_path
                    ),
                    "IndexDb::add_root",
                ));
            }
            if r.path.starts_with(&canonical) {
                return Err(ExplorerError::new(
                    ErrorCode::AlreadyExists,
                    format!(
                        "Existing indexed root ({}) is inside selected directory: {display_str}",
                        r.display_path
                    ),
                    "IndexDb::add_root",
                ));
            }
        }

        let root_id = uuid::Uuid::new_v4().to_string();
        let state = RootState::NotIndexed;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO roots (id, path_utf16le, display_path, state, completed_epoch) VALUES (?1, ?2, ?3, ?4, 0)",
            params![root_id, wide_bytes, display_str, state.as_str()],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to insert root into database: {e}"),
                "IndexDb::add_root",
            )
        })?;

        info!(
            "Registered new indexed root {} for path {}",
            root_id, display_str
        );
        Ok(IndexedRoot {
            id: root_id,
            path_utf16: canonical.as_os_str().encode_wide().collect(),
            path: canonical,
            display_path: display_str,
            state,
            completed_epoch: 0,
            reconciled_at: None,
        })
    }

    /// List all indexed roots.
    pub fn list_roots(&self) -> Result<Vec<IndexedRoot>, ExplorerError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, path_utf16le, display_path, state, completed_epoch, reconciled_at FROM roots")
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to prepare list_roots query: {e}"),
                    "IndexDb::list_roots",
                )
            })?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let path_bytes: Vec<u8> = row.get(1)?;
                let display_path: String = row.get(2)?;
                let state_str: String = row.get(3)?;
                let completed_epoch: i64 = row.get(4)?;
                let reconciled_at: Option<String> = row.get(5)?;

                let path = wide_to_path(&bytes_to_wide(&path_bytes));
                Ok(IndexedRoot {
                    id,
                    path_utf16: path.as_os_str().encode_wide().collect(),
                    path,
                    display_path,
                    state: state_str.parse().unwrap_or(RootState::NotIndexed),
                    completed_epoch,
                    reconciled_at,
                })
            })
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to query roots: {e}"),
                    "IndexDb::list_roots",
                )
            })?;

        let list = rows.flatten().collect();
        Ok(list)
    }

    /// Removes an indexed root and all associated metadata and FTS entries via CASCADE.
    pub fn remove_root(&self, root_id: &str) -> Result<(), ExplorerError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM roots WHERE id = ?1", params![root_id])
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to delete root {root_id}: {e}"),
                    "IndexDb::remove_root",
                )
            })?;
        info!("Removed indexed root {root_id} and all cascaded entries");
        Ok(())
    }

    /// Update the state of an indexed root.
    pub fn update_root_state(&self, root_id: &str, state: RootState) -> Result<(), ExplorerError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE roots SET state = ?1 WHERE id = ?2",
            params![state.as_str(), root_id],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to update root state: {e}"),
                "IndexDb::update_root_state",
            )
        })?;
        Ok(())
    }

    /// Mark an indexed root scan as completed.
    pub fn complete_root_scan(&self, root_id: &str, epoch: i64) -> Result<(), ExplorerError> {
        let now = chrono_like_now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE roots SET state = 'ready', completed_epoch = ?1, reconciled_at = ?2 WHERE id = ?3",
            params![epoch, now, root_id],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to mark root scan complete: {e}"),
                "IndexDb::complete_root_scan",
            )
        })?;
        Ok(())
    }

    /// Batched insert/update of entries during crawl.
    pub fn batch_upsert_entries(&self, entries: &[IndexEntryRecord]) -> Result<(), ExplorerError> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to begin transaction: {e}"),
                "IndexDb::batch_upsert_entries",
            )
        })?;

        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO entries (
                        root_id, parent_id, path_utf16le, name_display, name_norm,
                        extension_norm, kind, size_bytes, modified_filetime,
                        attributes, seen_epoch, stale
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)
                    ON CONFLICT(root_id, path_utf16le) DO UPDATE SET
                        name_display = excluded.name_display,
                        name_norm = excluded.name_norm,
                        extension_norm = excluded.extension_norm,
                        kind = excluded.kind,
                        size_bytes = excluded.size_bytes,
                        modified_filetime = excluded.modified_filetime,
                        attributes = excluded.attributes,
                        seen_epoch = excluded.seen_epoch,
                        stale = 0",
                )
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to prepare upsert entry query: {e}"),
                        "IndexDb::batch_upsert_entries",
                    )
                })?;

            for entry in entries {
                let wide_bytes = wide_to_bytes(&path_to_wide(&entry.path));
                stmt.execute(params![
                    entry.root_id,
                    entry.parent_id,
                    wide_bytes,
                    entry.name_display,
                    entry.name_norm,
                    entry.extension_norm,
                    entry.kind,
                    entry.size_bytes.map(|s| s as i64),
                    entry.modified_filetime.map(|m| m as i64),
                    entry.attributes as i64,
                    entry.seen_epoch,
                ])
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to upsert entry {}: {e}", entry.path.display()),
                        "IndexDb::batch_upsert_entries",
                    )
                })?;
            }
        }

        tx.commit().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to commit batch upsert: {e}"),
                "IndexDb::batch_upsert_entries",
            )
        })?;

        Ok(())
    }

    /// Prune stale direct children only after their parent directory was fully enumerated.
    pub fn prune_unseen_children(
        &self,
        root_id: &str,
        parent_path: &Path,
        current_epoch: i64,
    ) -> Result<usize, ExplorerError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to begin scoped prune: {e}"),
                "IndexDb::prune_unseen_children",
            )
        })?;

        let stale_entries = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, path_utf16le, kind FROM entries WHERE root_id = ?1 AND seen_epoch < ?2",
                )
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to query stale entries: {e}"),
                        "IndexDb::prune_unseen_children",
                    )
                })?;
            let rows = stmt
                .query_map(params![root_id, current_epoch], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i32>(2)?,
                    ))
                })
                .map_err(|e| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to enumerate stale entries: {e}"),
                        "IndexDb::prune_unseen_children",
                    )
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to read stale entries: {e}"),
                    "IndexDb::prune_unseen_children",
                )
            })?
        };

        let stale_directories: Vec<PathBuf> = stale_entries
            .iter()
            .filter_map(|(_, path_bytes, kind)| {
                let path = wide_to_path(&bytes_to_wide(path_bytes));
                (path.parent() == Some(parent_path) && *kind == 1).then_some(path)
            })
            .collect();
        let stale_ids: Vec<i64> = stale_entries
            .iter()
            .filter_map(|(id, path_bytes, _)| {
                let path = wide_to_path(&bytes_to_wide(path_bytes));
                let is_direct_child = path.parent() == Some(parent_path);
                let is_orphaned_descendant = stale_directories
                    .iter()
                    .any(|directory| path.starts_with(directory));
                (is_direct_child || is_orphaned_descendant).then_some(*id)
            })
            .collect();

        let mut delete_stmt = tx
            .prepare("DELETE FROM entries WHERE id = ?1")
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to prepare scoped prune: {e}"),
                    "IndexDb::prune_unseen_children",
                )
            })?;
        let mut count = 0;
        for id in stale_ids {
            count += delete_stmt.execute(params![id]).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to delete stale entry: {e}"),
                    "IndexDb::prune_unseen_children",
                )
            })?;
        }
        drop(delete_stmt);
        tx.commit().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to commit scoped prune: {e}"),
                "IndexDb::prune_unseen_children",
            )
        })?;
        Ok(count)
    }

    /// Raw database connection handle for queries.
    pub fn raw_conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}

pub fn normalize_root_path(path: &Path) -> PathBuf {
    // Keep the exact native Windows path units. In particular, lossy display
    // conversion here can collapse distinct names containing unpaired surrogates.
    path.to_path_buf()
}

pub fn wide_to_bytes(wide: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(wide.len() * 2);
    for &val in wide {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

pub fn bytes_to_wide(bytes: &[u8]) -> Vec<u16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| u16::from_le_bytes(*chunk))
        .collect()
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{now}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_db_schema_roots_and_triggers() {
        let db = IndexDb::open_in_memory().expect("open in memory db");
        let r1 = PathBuf::from(r"C:\test\projects");
        let root = db.add_root(&r1).expect("add root");
        assert_eq!(root.path, r1);
        assert_eq!(root.state, RootState::NotIndexed);

        // Cannot add overlapping root
        let child = r1.join("subfolder");
        assert!(db.add_root(&child).is_err());
        let parent = PathBuf::from(r"C:\test");
        assert!(db.add_root(&parent).is_err());

        // Insert entry
        let file = r1.join("report.pdf");
        let entry = IndexEntryRecord {
            root_id: root.id.clone(),
            parent_id: None,
            path: file.clone(),
            name_display: "report.pdf".to_string(),
            name_norm: "report.pdf".to_string(),
            extension_norm: "pdf".to_string(),
            kind: 0,
            size_bytes: Some(1024),
            modified_filetime: Some(123456),
            attributes: 32,
            seen_epoch: 1,
        };
        db.batch_upsert_entries(&[entry]).expect("insert entry");

        // Verify FTS table was populated by trigger
        {
            let conn = db.raw_conn();
            let guard = conn.lock().unwrap();
            let mut stmt = guard
                .prepare(
                    "SELECT rowid, name_norm FROM filename_fts WHERE filename_fts MATCH 'report'",
                )
                .unwrap();
            let mut rows = stmt.query([]).unwrap();
            let row = rows.next().unwrap().expect("found match in fts");
            let fts_name: String = row.get(1).unwrap();
            assert_eq!(fts_name, "report.pdf");
        }

        // Delete root -> entries and FTS rows cleaned up via CASCADE and trigger
        db.remove_root(&root.id).expect("remove root");
        let roots = db.list_roots().unwrap();
        assert!(roots.is_empty());

        {
            let conn = db.raw_conn();
            let guard = conn.lock().unwrap();
            let mut stmt = guard.prepare("SELECT count(*) FROM filename_fts").unwrap();
            let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
            assert_eq!(count, 0);
        }
    }
}
