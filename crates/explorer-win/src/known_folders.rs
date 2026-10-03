use crate::path::wide_to_path;
use explorer_domain::models::{DriveItem, KnownFolderItem};
use std::path::PathBuf;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures,
    FOLDERID_Videos, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
};
use windows::core::GUID;

/// Retrieves the filesystem path for a Windows Known Folder GUID.
pub fn get_known_folder_path(folder_id: &GUID) -> Option<PathBuf> {
    unsafe {
        match SHGetKnownFolderPath(folder_id, KF_FLAG_DEFAULT, HANDLE::default()) {
            Ok(path_pwstr) if !path_pwstr.is_null() => {
                let slice = path_pwstr.as_wide();
                let path = wide_to_path(slice);
                CoTaskMemFree(Some(path_pwstr.as_ptr() as *const _));
                Some(path)
            }
            _ => None,
        }
    }
}

/// Returns the standard user known folders: Desktop, Documents, Downloads, Pictures, Music, Videos.
pub fn get_standard_known_folders() -> Vec<KnownFolderItem> {
    let mut list = Vec::new();

    let folders = [
        ("documents", "Documents", &FOLDERID_Documents),
        ("downloads", "Downloads", &FOLDERID_Downloads),
        ("desktop", "Desktop", &FOLDERID_Desktop),
        ("pictures", "Pictures", &FOLDERID_Pictures),
        ("music", "Music", &FOLDERID_Music),
        ("videos", "Videos", &FOLDERID_Videos),
    ];

    for (id, name, guid) in folders {
        if let Some(path) = get_known_folder_path(guid) {
            list.push(KnownFolderItem {
                id: id.to_string(),
                name: name.to_string(),
                path: path.to_string_lossy().to_string(),
            });
        }
    }

    list
}

/// Enumerates all logical drives and their capacity/type.
pub fn get_logical_drives() -> Vec<DriveItem> {
    let mut drives = Vec::new();
    let mut buffer = [0u16; 512];

    let len = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) };
    if len == 0 || len > buffer.len() as u32 {
        return drives;
    }

    let mut start = 0;
    for i in 0..len as usize {
        if buffer[i] == 0 {
            if i > start {
                let drive_slice = &buffer[start..i];
                let drive_path = wide_to_path(drive_slice);
                let path_str = drive_path.to_string_lossy().to_string();

                let drive_wide: Vec<u16> = drive_slice.iter().copied().chain(Some(0)).collect();
                let drive_type = unsafe {
                    // Win32 Drive Type constants:
                    // 2: DRIVE_REMOVABLE, 3: DRIVE_FIXED, 4: DRIVE_REMOTE, 5: DRIVE_CDROM, 6: DRIVE_RAMDISK
                    match GetDriveTypeW(windows::core::PCWSTR(drive_wide.as_ptr())) {
                        2 => "Removable Disk",
                        3 => "Local Disk",
                        4 => "Network Drive",
                        5 => "CD/DVD Drive",
                        6 => "RAM Disk",
                        _ => "Drive",
                    }
                };

                let mut free_bytes_avail = 0u64;
                let mut total_bytes = 0u64;
                let mut total_free_bytes = 0u64;

                let space_ok = unsafe {
                    GetDiskFreeSpaceExW(
                        windows::core::PCWSTR(drive_wide.as_ptr()),
                        Some(&mut free_bytes_avail),
                        Some(&mut total_bytes),
                        Some(&mut total_free_bytes),
                    )
                };

                let (total, free) = if space_ok.is_ok() {
                    (Some(total_bytes), Some(total_free_bytes))
                } else {
                    (None, None)
                };

                let clean_name = path_str.trim_end_matches('\\');
                let display_name = format!("{} ({})", drive_type, clean_name);

                drives.push(DriveItem {
                    name: display_name,
                    path: path_str,
                    drive_type: drive_type.to_string(),
                    total_bytes: total,
                    free_bytes: free,
                });
            }
            start = i + 1;
        }
    }

    drives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_known_folders() {
        let folders = get_standard_known_folders();
        assert!(
            !folders.is_empty(),
            "Should resolve at least one known folder"
        );
        let docs = folders.iter().find(|f| f.id == "documents");
        assert!(docs.is_some(), "Documents folder should be found");
    }

    #[test]
    fn test_get_logical_drives() {
        let drives = get_logical_drives();
        assert!(!drives.is_empty(), "Should detect at least C: drive");
        let c_drive = drives
            .iter()
            .find(|d| d.path.to_uppercase().starts_with("C:"));
        assert!(c_drive.is_some(), "C: drive should exist on Windows");
    }
}
