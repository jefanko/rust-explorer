//! File and volume identity primitives used to revalidate mutation plans.

use crate::path::{ensure_extended_prefix, path_to_wide};
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::operations::FileIdentity;
use std::path::Path;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, GetFileInformationByHandle, OPEN_EXISTING,
};
use windows::core::PCWSTR;

/// Reads the volume/file ID and basic metadata without opening file content.
/// `FILE_FLAG_BACKUP_SEMANTICS` allows directory handles as well as file handles.
pub fn get_file_identity(path: &Path) -> Result<FileIdentity, ExplorerError> {
    let native_path = if path.is_absolute() {
        ensure_extended_prefix(path)
    } else {
        path.to_path_buf()
    };
    let wide_path = path_to_wide(&native_path);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_path.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            HANDLE::default(),
        )
    }
    .map_err(|error| {
        ExplorerError::new(
            ErrorCode::StaleItem,
            format!(
                "Cannot read current item identity for {}: {error}",
                path.display()
            ),
            "get_file_identity",
        )
    })?;

    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = HandleGuard(handle);

    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(handle, &mut info) }.map_err(|error| {
        ExplorerError::new(
            ErrorCode::StaleItem,
            format!(
                "Cannot read current item identity for {}: {error}",
                path.display()
            ),
            "get_file_identity",
        )
    })?;

    Ok(FileIdentity {
        volume_serial: info.dwVolumeSerialNumber,
        file_index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        size_bytes: ((info.nFileSizeHigh as u64) << 32) | info.nFileSizeLow as u64,
        last_write_filetime: ((info.ftLastWriteTime.dwHighDateTime as u64) << 32)
            | info.ftLastWriteTime.dwLowDateTime as u64,
    })
}
