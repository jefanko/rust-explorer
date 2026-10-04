//! Filesystem Service Crate
//! Manages directory listing snapshots, sorting, filtering, and path policies.

pub mod listing;
pub mod metadata;
pub mod policy;
pub mod snapshots;

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken};
use explorer_domain::models::{DirectoryPage, NavigationResponse, SortColumn, SortDirection};
use explorer_win::enumerate::enumerate_directory;
use explorer_win::path::{to_display_string, validate_safe_path};
use snapshots::FolderSnapshot;
use std::collections::HashMap;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct FolderService {
    generation_counter: AtomicU64,
    snapshots: RwLock<HashMap<FolderToken, FolderSnapshot>>,
    // Quick token-to-path mapping
    folder_paths: RwLock<HashMap<FolderToken, PathBuf>>,
}

const MAX_DIRECTORY_PAGE_SIZE: usize = 256;

impl FolderService {
    pub fn new() -> Self {
        Self {
            generation_counter: AtomicU64::new(1),
            snapshots: RwLock::new(HashMap::new()),
            folder_paths: RwLock::new(HashMap::new()),
        }
    }

    /// Navigates to a folder path, enumerates its contents, and caches the snapshot.
    pub fn navigate(&self, target_path: &Path) -> Result<NavigationResponse, ExplorerError> {
        let normalized_target = explorer_win::path::normalize_drive_root(target_path);
        let target_path = normalized_target.as_path();
        validate_safe_path(target_path)?;

        let canonical = target_path
            .canonicalize()
            .unwrap_or_else(|_| target_path.to_path_buf());
        let canonical = explorer_win::path::normalize_drive_root(&canonical);
        let folder_token = FolderToken::new();
        let generation = self.generation_counter.fetch_add(1, Ordering::SeqCst);
        let path_display = to_display_string(&canonical);

        let entries = enumerate_directory(&canonical, Some(&folder_token))?;
        let total_entries = entries.len();

        let snapshot = FolderSnapshot::new(
            folder_token.clone(),
            canonical.clone(),
            path_display.clone(),
            generation,
            entries,
        );

        {
            let mut snaps = self.snapshots.write().map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "navigate")
            })?;

            // LRU pruning: keep at most 16 snapshots
            const MAX_CACHED_SNAPSHOTS: usize = 16;
            if snaps.len() >= MAX_CACHED_SNAPSHOTS {
                let oldest = snaps
                    .iter()
                    .min_by_key(|(_, s)| s.created_at)
                    .map(|(t, _)| t.clone());
                if let Some(oldest_token) = oldest {
                    snaps.remove(&oldest_token);
                    if let Ok(mut paths) = self.folder_paths.write() {
                        paths.remove(&oldest_token);
                    }
                }
            }

            snaps.insert(folder_token.clone(), snapshot);
        }

        {
            let mut paths = self.folder_paths.write().map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "navigate")
            })?;
            paths.insert(folder_token.clone(), canonical.clone());
        }

        Ok(NavigationResponse {
            folder_token,
            path_display,
            path_utf16: canonical.as_os_str().encode_wide().collect(),
            generation,
            total_entries,
        })
    }

    /// Returns a sorted and paginated chunk of entries from a cached folder snapshot.
    pub fn list_page(
        &self,
        folder_token: &FolderToken,
        generation: u64,
        offset: usize,
        limit: usize,
        sort_column: SortColumn,
        sort_direction: SortDirection,
    ) -> Result<DirectoryPage, ExplorerError> {
        let limit = limit.clamp(1, MAX_DIRECTORY_PAGE_SIZE);
        let snaps = self
            .snapshots
            .read()
            .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Lock poisoned", "list_page"))?;

        let snapshot = snaps.get(folder_token).ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::StaleItem,
                "Folder snapshot expired or not found",
                "list_page",
            )
        })?;

        // Validate generation
        if snapshot.generation != generation {
            return Err(ExplorerError::new(
                ErrorCode::StaleItem,
                format!(
                    "Snapshot generation mismatch (current: {}, requested: {})",
                    snapshot.generation, generation
                ),
                "list_page",
            ));
        }

        let (paged_entries, total) = snapshot.get_page(offset, limit, sort_column, sort_direction);
        let end = offset.saturating_add(limit).min(total);

        Ok(DirectoryPage {
            folder_token: folder_token.clone(),
            path_display: snapshot.path_display.clone(),
            generation,
            offset,
            total_entries: total,
            entries: paged_entries,
            is_last_page: end >= total,
        })
    }

    /// Resolves an ItemToken to its absolute filesystem PathBuf within a folder.
    pub fn resolve_item(
        &self,
        folder_token: &FolderToken,
        item_token: &ItemToken,
    ) -> Option<PathBuf> {
        let snaps = self.snapshots.read().ok()?;
        snaps
            .get(folder_token)?
            .resolve_item(item_token)
            .map(|p| p.to_path_buf())
    }

    /// Retrieves the path of a cached folder token.
    pub fn get_folder_path(&self, folder_token: &FolderToken) -> Option<PathBuf> {
        let paths = self.folder_paths.read().ok()?;
        paths.get(folder_token).cloned()
    }
}

impl Default for FolderService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_navigate_and_paginate() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp_dir.path().join(".rust-explorer-fixture-root"),
            "listing fixture",
        )
        .unwrap();
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir(&data_dir).unwrap();
        let path = data_dir.as_path();

        File::create(path.join("file_a.txt")).unwrap();
        File::create(path.join("file_b.txt")).unwrap();
        fs::create_dir(path.join("dir_z")).unwrap();

        let service = FolderService::new();
        let nav = service.navigate(path).expect("navigate");
        assert_eq!(nav.total_entries, 3);

        let page = service
            .list_page(
                &nav.folder_token,
                nav.generation,
                0,
                2,
                SortColumn::Name,
                SortDirection::Ascending,
            )
            .expect("list_page");

        assert_eq!(page.entries.len(), 2);
        // dir_z must be first
        assert_eq!(page.entries[0].display_name, "dir_z");
        assert!(!page.is_last_page);

        let page2 = service
            .list_page(
                &nav.folder_token,
                nav.generation,
                2,
                2,
                SortColumn::Name,
                SortDirection::Ascending,
            )
            .expect("list_page 2");

        assert_eq!(page2.entries.len(), 1);
        assert!(page2.is_last_page);
    }
}
