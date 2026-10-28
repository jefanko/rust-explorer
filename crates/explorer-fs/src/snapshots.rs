use explorer_domain::ids::{FolderToken, ItemToken};
use explorer_domain::models::{FileEntry, SortColumn, SortDirection};
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Instant;

pub struct FolderSnapshot {
    pub folder_token: FolderToken,
    pub path: PathBuf,
    pub path_display: String,
    pub generation: u64,
    pub entries: Vec<FileEntry>,
    pub token_map: HashMap<ItemToken, u32>,
    pub created_at: Instant,
    sorted_cache: RwLock<Option<(SortColumn, SortDirection, Vec<u32>)>>,
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
        for (idx, entry) in entries.iter().enumerate() {
            token_map.insert(entry.token.clone(), idx as u32);
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

    pub fn resolve_item(&self, token: &ItemToken) -> Option<PathBuf> {
        let &idx = self.token_map.get(token)?;
        let entry = self.entries.get(idx as usize)?;
        let native_name = OsString::from_wide(&entry.native_name_utf16);
        Some(self.path.join(native_name))
    }

    /// Retrieves a paged slice of sorted entries, caching sorted indices without duplicating entries.
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
            && let Some((col, dir, ref sorted_indices)) = *cache
            && col == sort_column
            && dir == sort_direction
        {
            let end = (offset + limit).min(total);
            let paged = sorted_indices[offset..end]
                .iter()
                .map(|&idx| self.entries[idx as usize].clone())
                .collect();
            return (paged, total);
        }

        // Slow path: sort indices and populate cache
        let sorted_indices =
            crate::listing::sort_indices(&self.entries, sort_column, sort_direction);
        let end = (offset + limit).min(total);
        let result = sorted_indices[offset..end]
            .iter()
            .map(|&idx| self.entries[idx as usize].clone())
            .collect();

        if let Ok(mut cache) = self.sorted_cache.write() {
            *cache = Some((sort_column, sort_direction, sorted_indices));
        }

        (result, total)
    }

    /// Estimates the heap bytes retained by this snapshot.
    pub fn estimate_retained_bytes(&self) -> usize {
        let entry_base = std::mem::size_of::<FileEntry>();
        let mut entry_bytes = self.entries.len() * entry_base;
        for e in &self.entries {
            entry_bytes += e.display_name.len() + e.native_name_utf16.len() * 2 + e.extension.len();
        }
        let token_map_bytes = self.token_map.capacity() * (std::mem::size_of::<ItemToken>() + 4);
        let sorted_bytes = if let Ok(cache) = self.sorted_cache.read()
            && let Some((_, _, ref indices)) = *cache
        {
            indices.capacity() * 4
        } else {
            0
        };
        entry_bytes + token_map_bytes + sorted_bytes
    }
}
