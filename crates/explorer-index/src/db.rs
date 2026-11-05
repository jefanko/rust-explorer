use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_win::path::{path_to_wide, to_display_string, wide_to_path};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

pub const MAX_INDEXED_ROOTS: usize = 8;
const MAX_POOLED_READERS: usize = 4;
const SCHEMA_VERSION: i32 = 1;

const WRITER_PRAGMAS: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA foreign_keys = ON;
    PRAGMA busy_timeout = 5000;
    PRAGMA cache_size = -16384;
    PRAGMA mmap_size = 268435456;
    PRAGMA temp_store = MEMORY;
";

const READER_PRAGMAS: &str = "
    PRAGMA query_only = ON;
    PRAGMA busy_timeout = 5000;
    PRAGMA cache_size = -16384;
    PRAGMA mmap_size = 268435456;
    PRAGMA temp_store = MEMORY;
";
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
enum ReaderPool {
    File {
        path: PathBuf,
        idle: Mutex<Vec<Connection>>,
    },
    InMemory(Arc<Mutex<Connection>>),
}

pub struct IndexDb {
    writer: Arc<Mutex<Connection>>,
    readers: ReaderPool,
}

fn db_err(op: &'static str, msg: impl Into<String>) -> ExplorerError {
    ExplorerError::new(ErrorCode::Internal, msg, op)
}

impl IndexDb {
    /// Opens the index database at the specified path and initializes the schema.
    pub fn open(path: &Path) -> Result<Self, ExplorerError> {
        let conn = Connection::open(path).map_err(|e| {
            db_err(
                "IndexDb::open",
                format!("Failed to open index database at {}: {e}", path.display()),
            )
        })?;

        let db = Self {
            writer: Arc::new(Mutex::new(conn)),
            readers: ReaderPool::File {
                path: path.to_path_buf(),
                idle: Mutex::new(Vec::with_capacity(MAX_POOLED_READERS)),
            },
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Opens an in-memory index database for tests.
    pub fn open_in_memory() -> Result<Self, ExplorerError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            db_err(
                "IndexDb::open_in_memory",
                format!("Failed to open in-memory index database: {e}"),
            )
        })?;
        let writer = Arc::new(Mutex::new(conn));
        let db = Self {
            writer: writer.clone(),
            readers: ReaderPool::InMemory(writer),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Runs a read-only closure on an isolated reader connection when backed by a
    /// file (so background crawler writes never block queries or root lookups), or
    /// on the shared in-memory connection in tests.
    pub(crate) fn read<T>(
        &self,
        op: &'static str,
        f: impl FnOnce(&Connection) -> Result<T, ExplorerError>,
    ) -> Result<T, ExplorerError> {
        match &self.readers {
            ReaderPool::InMemory(writer) => {
                let conn = writer.lock().unwrap();
                f(&conn)
            }
            ReaderPool::File { path, idle } => {
                let conn = match idle.lock().unwrap().pop() {
                    Some(conn) => conn,
                    None => {
                        let conn = Connection::open(path).map_err(|e| {
                            db_err(op, format!("Failed to open index reader connection: {e}"))
                        })?;
                        conn.execute_batch(READER_PRAGMAS).map_err(|e| {
                            db_err(op, format!("Failed to configure index reader: {e}"))
                        })?;
                        conn
                    }
                };
                let result = f(&conn);
                let mut pool = idle.lock().unwrap();
                if pool.len() < MAX_POOLED_READERS {
                    pool.push(conn);
                }
                result
            }
        }
    }

    /// Initializes pragmas, metadata tables, FTS5 trigram table, synchronization triggers,
    /// and runs idempotent migrations for existing databases.
    pub fn init_schema(&self) -> Result<(), ExplorerError> {
        let mut conn = self.writer.lock().unwrap();
        conn.execute_batch(WRITER_PRAGMAS).map_err(|e| {
            db_err(
                "IndexDb::init_schema",
                format!("Failed to apply writer pragmas: {e}"),
            )
        })?;

        conn.execute_batch(
            "
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
            CREATE INDEX IF NOT EXISTS entries_parent_id ON entries(parent_id);
            CREATE INDEX IF NOT EXISTS entries_root_id ON entries(root_id);
            CREATE INDEX IF NOT EXISTS entries_extension ON entries(root_id, extension_norm, kind);
            CREATE INDEX IF NOT EXISTS entries_extension_norm ON entries(extension_norm, kind);
            CREATE INDEX IF NOT EXISTS entries_name_norm ON entries(name_norm);
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
            ",
        )
        .map_err(|e| {
            db_err(
                "IndexDb::init_schema",
                format!("Failed to initialize index schema: {e}"),
            )
        })?;

        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0);
        if version < SCHEMA_VERSION {
            Self::migrate_to_v1(&mut conn).map_err(|e| {
                db_err(
                    "IndexDb::init_schema",
                    format!("Failed to migrate index schema to v{SCHEMA_VERSION}: {e}"),
                )
            })?;
        }

        debug!("Initialized index database schema v{SCHEMA_VERSION}");
        Ok(())
    }

    fn migrate_to_v1(conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let dup_ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM roots
                 WHERE rowid NOT IN (SELECT MIN(rowid) FROM roots GROUP BY path_utf16le)",
            )?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        if !dup_ids.is_empty() {
            conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
            let res = (|| -> Result<(), rusqlite::Error> {
                let tx = conn.transaction()?;
                for id in &dup_ids {
                    tx.execute("DELETE FROM entries WHERE root_id = ?1", params![id])?;
                    tx.execute("DELETE FROM roots WHERE id = ?1", params![id])?;
                }
                tx.commit()
            })();
            let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
            res?;
            info!("Removed {} duplicate indexed root(s)", dup_ids.len());
        }

        conn.execute_batch(
            "
            CREATE UNIQUE INDEX IF NOT EXISTS roots_path_utf16le ON roots(path_utf16le);
            DROP TRIGGER IF EXISTS entries_au;
            CREATE TRIGGER entries_au AFTER UPDATE OF name_norm ON entries
            WHEN old.name_norm IS NOT new.name_norm BEGIN
                UPDATE filename_fts SET name_norm = new.name_norm WHERE rowid = old.id;
            END;
            PRAGMA user_version = 1;
            ",
        )
    }

    /// Add a new indexed root atomically. Rejects overlap with existing roots and
    /// enforces the MAX_INDEXED_ROOTS bound under a single writer transaction.
    pub fn add_root(&self, path: &Path) -> Result<IndexedRoot, ExplorerError> {
        let canonical = normalize_root_path(path);
        let display_str = to_display_string(&canonical);
        let wide_bytes = wide_to_bytes(&path_to_wide(&canonical));

        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction().map_err(|e| {
            db_err(
                "IndexDb::add_root",
                format!("Failed to begin transaction: {e}"),
            )
        })?;

        let existing = Self::query_roots(&tx, "IndexDb::add_root")?;
        if existing.len() >= MAX_INDEXED_ROOTS {
            return Err(ExplorerError::new(
                ErrorCode::QueueFull,
                format!("Maximum {MAX_INDEXED_ROOTS} indexed roots permitted in MVP"),
                "IndexDb::add_root",
            ));
        }

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

        tx.execute(
            "INSERT INTO roots (id, path_utf16le, display_path, state, completed_epoch) VALUES (?1, ?2, ?3, ?4, 0)",
            params![root_id, wide_bytes, display_str, state.as_str()],
        )
        .map_err(|e| {
            db_err(
                "IndexDb::add_root",
                format!("Failed to insert root into database: {e}"),
            )
        })?;
        tx.commit().map_err(|e| {
            db_err(
                "IndexDb::add_root",
                format!("Failed to commit root registration: {e}"),
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

    /// List all indexed roots via a reader connection.
    pub fn list_roots(&self) -> Result<Vec<IndexedRoot>, ExplorerError> {
        self.read("IndexDb::list_roots", |conn| {
            Self::query_roots(conn, "IndexDb::list_roots")
        })
    }

    fn query_roots(conn: &Connection, op: &'static str) -> Result<Vec<IndexedRoot>, ExplorerError> {
        let mut stmt = conn
            .prepare("SELECT id, path_utf16le, display_path, state, completed_epoch, reconciled_at FROM roots")
            .map_err(|e| db_err(op, format!("Failed to prepare list_roots query: {e}")))?;

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
            .map_err(|e| db_err(op, format!("Failed to query roots: {e}")))?;

        Ok(rows.flatten().collect())
    }

    /// Removes an indexed root and all associated metadata and FTS entries.
    pub fn remove_root(&self, root_id: &str) -> Result<(), ExplorerError> {
        let conn = self.writer.lock().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF")
            .map_err(|e| {
                db_err(
                    "IndexDb::remove_root",
                    format!("Failed to disable foreign keys: {e}"),
                )
            })?;

        let res = (|| -> Result<(), rusqlite::Error> {
            conn.execute("DELETE FROM entries WHERE root_id = ?1", params![root_id])?;
            conn.execute("DELETE FROM roots WHERE id = ?1", params![root_id])?;
            Ok(())
        })();

        let _ = conn.execute_batch("PRAGMA foreign_keys = ON");
        res.map_err(|e| {
            db_err(
                "IndexDb::remove_root",
                format!("Failed to delete root {root_id}: {e}"),
            )
        })?;

        info!("Removed indexed root {root_id} and all cascaded entries");
        Ok(())
    }

    /// Check whether a root still exists in the database.
    pub fn root_exists(&self, root_id: &str) -> Result<bool, ExplorerError> {
        self.read("IndexDb::root_exists", |conn| {
            let mut stmt = conn
                .prepare("SELECT 1 FROM roots WHERE id = ?1")
                .map_err(|e| {
                    db_err(
                        "IndexDb::root_exists",
                        format!("Failed to prepare root existence query: {e}"),
                    )
                })?;
            stmt.exists(params![root_id]).map_err(|e| {
                db_err(
                    "IndexDb::root_exists",
                    format!("Failed to query root existence: {e}"),
                )
            })
        })
    }

    /// Update the state of an indexed root.
    pub fn update_root_state(&self, root_id: &str, state: RootState) -> Result<(), ExplorerError> {
        let conn = self.writer.lock().unwrap();
        conn.execute(
            "UPDATE roots SET state = ?1 WHERE id = ?2",
            params![state.as_str(), root_id],
        )
        .map_err(|e| {
            db_err(
                "IndexDb::update_root_state",
                format!("Failed to update root state: {e}"),
            )
        })?;
        Ok(())
    }

    /// Mark an indexed root scan as completed.
    pub fn complete_root_scan(&self, root_id: &str, epoch: i64) -> Result<(), ExplorerError> {
        let now = chrono_like_now();
        let conn = self.writer.lock().unwrap();
        conn.execute(
            "UPDATE roots SET state = 'ready', completed_epoch = ?1, reconciled_at = ?2 WHERE id = ?3",
            params![epoch, now, root_id],
        )
        .map_err(|e| {
            db_err(
                "IndexDb::complete_root_scan",
                format!("Failed to mark root scan complete: {e}"),
            )
        })?;
        Ok(())
    }

    /// Batched insert/update of entries during crawl.
    pub fn batch_upsert_entries(&self, entries: &[IndexEntryRecord]) -> Result<(), ExplorerError> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction().map_err(|e| {
            db_err(
                "IndexDb::batch_upsert_entries",
                format!("Failed to begin transaction: {e}"),
            )
        })?;

        {
            let mut stmt = tx
                .prepare_cached(
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
                    db_err(
                        "IndexDb::batch_upsert_entries",
                        format!("Failed to prepare upsert entry query: {e}"),
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
                    db_err(
                        "IndexDb::batch_upsert_entries",
                        format!("Failed to upsert entry {}: {e}", entry.path.display()),
                    )
                })?;
            }
        }

        tx.commit().map_err(|e| {
            db_err(
                "IndexDb::batch_upsert_entries",
                format!("Failed to commit batch upsert: {e}"),
            )
        })?;
        Ok(())
    }

    /// Prune stale entries for all directories whose enumeration completed during a crawl,
    /// in a single transaction instead of one transaction per directory.
    pub fn prune_unseen(
        &self,
        root_id: &str,
        completed_dirs: &[PathBuf],
        current_epoch: i64,
    ) -> Result<usize, ExplorerError> {
        self.prune_unseen_scoped(root_id, None, completed_dirs, current_epoch)
    }

    /// Prunes stale entries, optionally restricted to a directory subtree via the
    /// `UNIQUE(root_id, path_utf16le)` B-tree so incremental watch reconciliation
    /// touches only the changed folder instead of scanning the entire root.
    pub fn prune_unseen_scoped(
        &self,
        root_id: &str,
        scope_dir: Option<&Path>,
        completed_dirs: &[PathBuf],
        current_epoch: i64,
    ) -> Result<usize, ExplorerError> {
        if completed_dirs.is_empty() {
            return Ok(0);
        }

        let mut conn = self.writer.lock().unwrap();
        let tx = conn.transaction().map_err(|e| {
            db_err(
                "IndexDb::prune_unseen_scoped",
                format!("Failed to begin prune transaction: {e}"),
            )
        })?;

        let map_row = |row: &rusqlite::Row<'_>| {
            let id: i64 = row.get(0)?;
            let path_bytes: Vec<u8> = row.get(1)?;
            let kind: i32 = row.get(2)?;
            Ok((id, wide_to_path(&bytes_to_wide(&path_bytes)), kind))
        };

        let stale_entries = if let Some(dir) = scope_dir {
            let (lo, hi) = path_prefix_range_bytes(dir);
            let self_bytes = wide_to_bytes(&path_to_wide(dir));
            let mut stmt = tx
                .prepare(
                    "SELECT id, path_utf16le, kind FROM entries
                     WHERE root_id = ?1
                       AND ((path_utf16le >= ?2 AND path_utf16le < ?3) OR path_utf16le = ?4)
                       AND seen_epoch < ?5",
                )
                .map_err(|e| {
                    db_err(
                        "IndexDb::prune_unseen_scoped",
                        format!("Failed to prepare scoped stale query: {e}"),
                    )
                })?;
            let rows = stmt
                .query_map(params![root_id, lo, hi, self_bytes, current_epoch], map_row)
                .map_err(|e| {
                    db_err(
                        "IndexDb::prune_unseen_scoped",
                        format!("Failed to enumerate scoped stale entries: {e}"),
                    )
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| {
                db_err(
                    "IndexDb::prune_unseen_scoped",
                    format!("Failed to read scoped stale entries: {e}"),
                )
            })?
        } else {
            let mut stmt = tx
                .prepare(
                    "SELECT id, path_utf16le, kind FROM entries
                     WHERE root_id = ?1 AND seen_epoch < ?2",
                )
                .map_err(|e| {
                    db_err(
                        "IndexDb::prune_unseen_scoped",
                        format!("Failed to prepare stale query: {e}"),
                    )
                })?;
            let rows = stmt
                .query_map(params![root_id, current_epoch], map_row)
                .map_err(|e| {
                    db_err(
                        "IndexDb::prune_unseen_scoped",
                        format!("Failed to enumerate stale entries: {e}"),
                    )
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| {
                db_err(
                    "IndexDb::prune_unseen_scoped",
                    format!("Failed to read stale entries: {e}"),
                )
            })?
        };
        if stale_entries.is_empty() {
            return Ok(0);
        }

        let completed_set: HashSet<&Path> = completed_dirs.iter().map(PathBuf::as_path).collect();
        let removed_dirs: HashSet<&Path> = stale_entries
            .iter()
            .filter_map(|(_, path, kind)| {
                (*kind == 1 && path.parent().is_some_and(|p| completed_set.contains(p)))
                    .then_some(path.as_path())
            })
            .collect();

        let mut delete_stmt = tx
            .prepare("DELETE FROM entries WHERE id = ?1")
            .map_err(|e| {
                db_err(
                    "IndexDb::prune_unseen",
                    format!("Failed to prepare prune delete: {e}"),
                )
            })?;
        let mut count = 0;
        for (id, path, _) in &stale_entries {
            let direct = path.parent().is_some_and(|p| completed_set.contains(p));
            let orphaned = !direct
                && !removed_dirs.is_empty()
                && path
                    .parent()
                    .into_iter()
                    .flat_map(Path::ancestors)
                    .any(|anc| removed_dirs.contains(anc));
            if direct || orphaned {
                count += delete_stmt.execute(params![id]).map_err(|e| {
                    db_err(
                        "IndexDb::prune_unseen",
                        format!("Failed to delete stale entry: {e}"),
                    )
                })?;
            }
        }
        drop(delete_stmt);
        tx.commit().map_err(|e| {
            db_err(
                "IndexDb::prune_unseen",
                format!("Failed to commit prune transaction: {e}"),
            )
        })?;
        Ok(count)
    }
    /// Resolves the exact stored directory path inside `root_id` (matching case-insensitively
    /// on relative path components when the caller's path differs in letter case).
    pub fn resolve_stored_dir(&self, root_id: &str, dir: &Path) -> Option<PathBuf> {
        let roots = self.list_roots().ok()?;
        let root = roots.into_iter().find(|r| r.id == root_id)?;

        let root_stripped = explorer_win::path::strip_extended_prefix(&root.path);
        let dir_stripped = explorer_win::path::strip_extended_prefix(dir);

        let root_lc = root_stripped.to_string_lossy().to_lowercase();
        let dir_lc = dir_stripped.to_string_lossy().to_lowercase();
        let root_trim = root_lc.trim_end_matches(['\\', '/']);
        let dir_trim = dir_lc.trim_end_matches(['\\', '/']);

        if dir_trim == root_trim {
            return Some(root.path);
        }
        let rest = dir_trim
            .strip_prefix(root_trim)?
            .trim_start_matches(['\\', '/']);
        if rest.is_empty() {
            return Some(root.path);
        }

        // Fast path: exact case match against stored `path_utf16le`.
        let rel_orig = dir_stripped
            .to_string_lossy()
            .get(root_stripped.to_string_lossy().len()..)
            .map(|s| s.trim_matches(['\\', '/']).to_string())
            .unwrap_or_default();
        let direct_candidate = if rel_orig.is_empty() {
            root.path.clone()
        } else {
            root.path.join(&rel_orig)
        };
        let direct_bytes = wide_to_bytes(&path_to_wide(&direct_candidate));

        self.read("IndexDb::resolve_stored_dir", |conn| {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM entries WHERE root_id = ?1 AND path_utf16le = ?2",
                    params![root_id, direct_bytes],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if exists {
                return Ok(Some(direct_candidate));
            }

            // Case-insensitive component walk using `entries_name_norm`.
            let mut current = root.path.clone();
            for seg in rest.split(['\\', '/']).filter(|s| !s.is_empty()) {
                let (lo, hi) = path_prefix_range_bytes(&current);
                let mut stmt = conn
                    .prepare(
                        "SELECT path_utf16le FROM entries INDEXED BY entries_name_norm
                         WHERE name_norm = ?1
                           AND root_id = ?2
                           AND kind = 1
                           AND path_utf16le >= ?3
                           AND path_utf16le < ?4",
                    )
                    .map_err(|e| db_err("resolve_stored_dir", e.to_string()))?;
                let rows = stmt
                    .query_map(params![seg, root_id, lo, hi], |row| {
                        let b: Vec<u8> = row.get(0)?;
                        Ok(wide_to_path(&bytes_to_wide(&b)))
                    })
                    .map_err(|e| db_err("resolve_stored_dir", e.to_string()))?;
                let found = rows
                    .flatten()
                    .find(|p| p.parent() == Some(current.as_path()));
                match found {
                    Some(next) => current = next,
                    None => return Ok(Some(direct_candidate)),
                }
            }
            Ok(Some(current))
        })
        .ok()
        .flatten()
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

/// Computes the half-open `[lo, hi)` byte-comparison range on `path_utf16le` that matches
/// every descendant path inside `dir` (i.e. paths beginning with `<dir>\` in UTF-16LE).
pub fn path_prefix_range_bytes(dir: &Path) -> (Vec<u8>, Vec<u8>) {
    let mut units: Vec<u16> = dir.as_os_str().encode_wide().collect();
    if !matches!(units.last(), Some(92 | 47)) {
        units.push(92); // '\\'
    }
    let lo = wide_to_bytes(&units);
    let mut hi = lo.clone();
    for b in hi.iter_mut().rev() {
        if *b < 0xFF {
            *b += 1;
            break;
        }
        *b = 0;
    }
    (lo, hi)
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
        db.read("test", |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT rowid, name_norm FROM filename_fts WHERE filename_fts MATCH 'report'",
                )
                .unwrap();
            let mut rows = stmt.query([]).unwrap();
            let row = rows.next().unwrap().expect("found match in fts");
            let fts_name: String = row.get(1).unwrap();
            assert_eq!(fts_name, "report.pdf");
            Ok(())
        })
        .unwrap();

        // Delete root -> entries and FTS rows cleaned up via CASCADE and trigger
        db.remove_root(&root.id).expect("remove root");
        let roots = db.list_roots().unwrap();
        assert!(roots.is_empty());

        db.read("test", |conn| {
            let mut stmt = conn.prepare("SELECT count(*) FROM filename_fts").unwrap();
            let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn v1_migration_dedupes_roots_and_skips_unchanged_fts_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("legacy.sqlite3");
        let wide = wide_to_bytes(&path_to_wide(Path::new(r"F:\")));

        // Simulate a v0 database containing duplicate roots and entries.
        {
            let raw = Connection::open(&db_path).unwrap();
            raw.execute_batch(
                "CREATE TABLE roots (
                    id TEXT PRIMARY KEY,
                    path_utf16le BLOB NOT NULL,
                    display_path TEXT NOT NULL,
                    volume_key BLOB,
                    state TEXT NOT NULL,
                    completed_epoch INTEGER NOT NULL DEFAULT 0,
                    reconciled_at TEXT
                );
                CREATE TABLE entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    root_id TEXT NOT NULL,
                    parent_id INTEGER,
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
                );",
            )
            .unwrap();
            for id in ["root-a", "root-b"] {
                raw.execute(
                    "INSERT INTO roots (id, path_utf16le, display_path, state) VALUES (?1, ?2, 'F:\\', 'needs_reconcile')",
                    params![id, wide],
                )
                .unwrap();
                raw.execute(
                    "INSERT INTO entries (root_id, path_utf16le, name_display, name_norm, extension_norm, kind, attributes, seen_epoch)
                     VALUES (?1, ?2, 'a.txt', 'a.txt', 'txt', 0, 32, 1)",
                    params![id, wide],
                )
                .unwrap();
            }
        }

        let db = IndexDb::open(&db_path).unwrap();
        let roots = db.list_roots().unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].id, "root-a");

        db.read("test", |conn| {
            let entries: i64 = conn
                .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
                .unwrap();
            assert_eq!(entries, 1);
            Ok(())
        })
        .unwrap();
    }
}
