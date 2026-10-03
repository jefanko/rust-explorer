use crate::path::{ensure_extended_prefix, path_to_wide, wide_to_path};
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken};
use explorer_domain::models::{EntryKind, FileEntry};
use std::path::Path;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, GetLastError, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_READONLY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_SYSTEM, FIND_FIRST_EX_LARGE_FETCH, FindClose,
    FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW, FindNextFileW, WIN32_FIND_DATAW,
};
use windows::core::PCWSTR;

/// Enumerates entries in a directory using native Win32 FindFirstFileExW/FindNextFileW.
pub fn enumerate_directory(
    dir_path: &Path,
    parent_token: Option<&FolderToken>,
) -> Result<Vec<FileEntry>, ExplorerError> {
    if !dir_path.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Directory does not exist: {}", dir_path.display()),
            "enumerate_directory",
        ));
    }

    if !dir_path.is_dir() {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            format!("Path is not a directory: {}", dir_path.display()),
            "enumerate_directory",
        ));
    }

    // Build search pattern: \\?\C:\dir\*
    let extended_dir = ensure_extended_prefix(dir_path);
    let mut search_pattern = extended_dir.clone();
    search_pattern.push("*");

    let search_wide = path_to_wide(&search_pattern);
    let mut find_data = WIN32_FIND_DATAW::default();

    let handle: HANDLE = unsafe {
        FindFirstFileExW(
            PCWSTR(search_wide.as_ptr()),
            FindExInfoBasic,
            &mut find_data as *mut _ as *mut _,
            FindExSearchNameMatch,
            None,
            FIND_FIRST_EX_LARGE_FETCH,
        )
    }
    .map_err(|e| {
        let err_code = match unsafe { GetLastError() } {
            windows::Win32::Foundation::ERROR_ACCESS_DENIED => ErrorCode::AccessDenied,
            ERROR_FILE_NOT_FOUND => ErrorCode::NotFound,
            _ => ErrorCode::Internal,
        };
        let mut err = ExplorerError::new(
            err_code,
            format!("Failed to open directory {}: {e}", dir_path.display()),
            "enumerate_directory",
        );
        err.native_code = Some(e.code().0 as u32);
        err
    })?;

    struct FindHandleGuard(HANDLE);
    impl Drop for FindHandleGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = FindClose(self.0);
            }
        }
    }
    let _guard = FindHandleGuard(handle);

    let mut entries = Vec::new();

    loop {
        let name_path = wide_to_path(&find_data.cFileName);
        let name = name_path.to_string_lossy();

        // Skip "." and ".."
        if name != "." && name != ".." {
            let attrs = find_data.dwFileAttributes;
            let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY.0) != 0;
            let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0;
            let is_hidden = (attrs & FILE_ATTRIBUTE_HIDDEN.0) != 0;
            let is_readonly = (attrs & FILE_ATTRIBUTE_READONLY.0) != 0;
            let is_system = (attrs & FILE_ATTRIBUTE_SYSTEM.0) != 0;

            let kind = if is_reparse {
                EntryKind::ReparsePoint
            } else if is_dir {
                EntryKind::Directory
            } else {
                EntryKind::File
            };

            let size_bytes = if is_dir {
                None
            } else {
                let size =
                    ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
                Some(size)
            };

            let modified_filetime = {
                let ft = find_data.ftLastWriteTime;
                let val = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
                if val > 0 { Some(val) } else { None }
            };

            let extension = if is_dir {
                String::new()
            } else {
                name_path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default()
            };

            entries.push(FileEntry {
                token: ItemToken::new(),
                parent_token: parent_token.cloned(),
                display_name: name.to_string(),
                escaped_name_hint: None,
                extension,
                kind,
                size_bytes,
                modified_filetime,
                attributes: attrs,
                is_hidden,
                is_readonly,
                is_system,
            });
        }

        let next_ok = unsafe { FindNextFileW(handle, &mut find_data) };
        if next_ok.is_err() {
            break;
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_enumerate_temp_dir() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path();

        File::create(path.join("file1.txt")).expect("create file1");
        File::create(path.join("file2.log")).expect("create file2");
        fs::create_dir(path.join("subfolder")).expect("create subfolder");

        let entries = enumerate_directory(path, None).expect("enumerate");
        assert_eq!(entries.len(), 3);

        let names: Vec<String> = entries.iter().map(|e| e.display_name.clone()).collect();
        assert!(names.contains(&"file1.txt".to_string()));
        assert!(names.contains(&"file2.log".to_string()));
        assert!(names.contains(&"subfolder".to_string()));

        let subfolder = entries
            .iter()
            .find(|e| e.display_name == "subfolder")
            .unwrap();
        assert_eq!(subfolder.kind, EntryKind::Directory);
        assert_eq!(subfolder.size_bytes, None);

        let file1 = entries
            .iter()
            .find(|e| e.display_name == "file1.txt")
            .unwrap();
        assert_eq!(file1.kind, EntryKind::File);
        assert_eq!(file1.extension, "txt");
    }
}
