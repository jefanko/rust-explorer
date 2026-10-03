use explorer_domain::ids::{FolderToken, ItemToken};
use explorer_domain::models::FileEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct FolderSnapshot {
    pub folder_token: FolderToken,
    pub path: PathBuf,
    pub path_display: String,
    pub generation: u64,
    pub entries: Vec<FileEntry>,
    pub token_map: HashMap<ItemToken, PathBuf>,
    pub created_at: Instant,
}

impl FolderSnapshot {
    pub fn new(
        folder_token: FolderToken,
        path: PathBuf,
        path_display: String,
        generation: u64,
        entries: Vec<FileEntry>,
    ) -> Self {
        let mut token_map = HashMap::with_capacity(entries.len());
        for entry in &entries {
            let item_path = path.join(&entry.display_name);
            token_map.insert(entry.token.clone(), item_path);
        }

        Self {
            folder_token,
            path,
            path_display,
            generation,
            entries,
            token_map,
            created_at: Instant::now(),
        }
    }

    pub fn resolve_item(&self, token: &ItemToken) -> Option<&Path> {
        self.token_map.get(token).map(|p| p.as_path())
    }
}
