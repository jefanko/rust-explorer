use explorer_domain::errors::{ErrorCode, ExplorerError};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub theme: String,
    pub show_hidden_files: bool,
    pub restore_tabs: bool,
    pub saved_tabs: Vec<String>,
    pub favorites: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            show_hidden_files: false,
            restore_tabs: true,
            saved_tabs: Vec::new(),
            favorites: Vec::new(),
        }
    }
}

pub struct SettingsStore {
    conn: Mutex<Connection>,
}

impl SettingsStore {
    pub fn open(db_path: &Path) -> Result<Self, ExplorerError> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(db_path).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open state database: {e}"),
                "SettingsStore::open",
            )
        })?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, ExplorerError> {
        let conn = Connection::open_in_memory().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open in-memory database: {e}"),
                "SettingsStore::open_in_memory",
            )
        })?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), ExplorerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "init_schema"))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS favorites (
                path TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                added_epoch INTEGER NOT NULL
            );",
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to initialize schema: {e}"),
                "init_schema",
            )
        })?;

        Ok(())
    }

    pub fn load_settings(&self) -> Result<AppSettings, ExplorerError> {
        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "load_settings")
        })?;

        let mut stmt = conn
            .prepare("SELECT value FROM kv_store WHERE key = 'app_settings';")
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to prepare query: {e}"),
                    "load_settings",
                )
            })?;

        let mut rows = stmt.query([]).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to execute query: {e}"),
                "load_settings",
            )
        })?;

        let mut settings = if let Some(row) = rows.next().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Row error: {e}"),
                "load_settings",
            )
        })? {
            let json_str: String = row.get(0).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Get column error: {e}"),
                    "load_settings",
                )
            })?;
            serde_json::from_str(&json_str).unwrap_or_default()
        } else {
            AppSettings::default()
        };

        // Load favorites from favorites table
        let mut fav_stmt = conn
            .prepare("SELECT path FROM favorites ORDER BY added_epoch ASC;")
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to prepare favorites query: {e}"),
                    "load_settings",
                )
            })?;

        let fav_rows = fav_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to query favorites: {e}"),
                    "load_settings",
                )
            })?;

        let mut favs = Vec::new();
        for f in fav_rows.flatten() {
            favs.push(f);
        }
        settings.favorites = favs;

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), ExplorerError> {
        let json_str = serde_json::to_string(settings).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to serialize settings: {e}"),
                "save_settings",
            )
        })?;

        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "save_settings")
        })?;

        conn.execute(
            "INSERT INTO kv_store (key, value) VALUES ('app_settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1;",
            params![json_str],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to save settings: {e}"),
                "save_settings",
            )
        })?;

        Ok(())
    }

    pub fn add_favorite(&self, path: &str) -> Result<(), ExplorerError> {
        let name = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "add_favorite")
        })?;

        conn.execute(
            "INSERT INTO favorites (path, display_name, added_epoch) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO NOTHING;",
            params![path, name, epoch],
        )
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to add favorite: {e}"),
                "add_favorite",
            )
        })?;

        Ok(())
    }

    pub fn remove_favorite(&self, path: &str) -> Result<(), ExplorerError> {
        let conn = self.conn.lock().map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "remove_favorite")
        })?;

        conn.execute("DELETE FROM favorites WHERE path = ?1;", params![path])
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to remove favorite: {e}"),
                    "remove_favorite",
                )
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_store_in_memory() {
        let store = SettingsStore::open_in_memory().expect("open store");

        let mut initial = store.load_settings().expect("load initial");
        assert_eq!(initial.theme, "system");
        assert!(initial.favorites.is_empty());

        initial.theme = "dark".to_string();
        initial.restore_tabs = false;
        store.save_settings(&initial).expect("save");

        let loaded = store.load_settings().expect("reload");
        assert_eq!(loaded.theme, "dark");
        assert!(!loaded.restore_tabs);

        store.add_favorite(r"C:\MyProject").expect("add favorite");
        let with_fav = store.load_settings().expect("reload with fav");
        assert_eq!(with_fav.favorites.len(), 1);
        assert_eq!(with_fav.favorites[0], r"C:\MyProject");

        store.remove_favorite(r"C:\MyProject").expect("remove fav");
        let empty_fav = store.load_settings().expect("reload empty fav");
        assert!(empty_fav.favorites.is_empty());
    }
}
