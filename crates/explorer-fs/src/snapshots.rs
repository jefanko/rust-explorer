use explorer_domain::ids::{FolderToken, ItemToken};
use explorer_domain::models::{FileEntry, SortColumn, SortDirection};
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Instant;

pub struct FolderSnapshot {
    pub folder_token: FolderToken,
    pub path: PathBuf,
    pub path_display: String,
    pub generation: u64,
    pub entries: Vec<FileEntry>,
    pub token_map: HashMap<ItemToken, PathBuf>,
    pub created_at: Instant,
    sorted_cache: RwLock<Option<(SortColumn, SortDirection, Vec<FileEntry>)>>,
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
            let native_name = OsString::from_wide(&entry.native_name_utf16);
            let item_path = path.join(native_name);
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
            sorted_cache: RwLock::new(None),
        }
    }

    pub fn resolve_item(&self, token: &ItemToken) -> Option<&Path> {
        self.token_map.get(token).map(|p| p.as_path())
    }

    /// Retrieves a paged slice of sorted entries, caching the sorted result for subsequent page requests.
    pub fn get_page(
        &self,
        offset: usize,
        limit: usize,
        sort_column: SortColumn,
        sort_direction: SortDirection,
    ) -> (Vec<FileEntry>, usize) {
        let total = self.entries.len();
        if total == 0 || offset >= total {
            return (Vec::new(), total);
        }

        // Fast path: check if sorted_cache already matches the requested sort
        if let Ok(cache) = self.sorted_cache.read()
            && let Some((col, dir, ref sorted)) = *cache
            && col == sort_column
            && dir == sort_direction
        {
            let end = (offset + limit).min(total);
            return (sorted[offset..end].to_vec(), total);
        }

        // Slow path: sort and populate cache
        let mut sorted = self.entries.clone();
        crate::listing::sort_entries(&mut sorted, sort_column, sort_direction);

        let end = (offset + limit).min(total);
        let result = sorted[offset..end].to_vec();

        if let Ok(mut cache) = self.sorted_cache.write() {
            *cache = Some((sort_column, sort_direction, sorted));
        }

        (result, total)
    }
}
