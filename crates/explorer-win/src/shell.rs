use crate::path::path_to_wide;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IShellItem, SEE_MASK_FLAG_NO_UI, SEE_MASK_INVOKEIDLIST,
    SHCreateItemFromParsingName, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

/// Launches a file with its registered Windows file association using ShellExecuteExW.
pub fn open_file_with_association(path: &Path) -> Result<(), ExplorerError> {
    if !path.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Target file does not exist: {}", path.display()),
            "open_file_with_association",
        ));
    }

    let file_wide = path_to_wide(path);
    let verb_wide: Vec<u16> = "open\0".encode_utf16().collect();

    let mut exec_info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI,
        hwnd: HWND::default(),
        lpVerb: PCWSTR(verb_wide.as_ptr()),
        lpFile: PCWSTR(file_wide.as_ptr()),
        lpParameters: PCWSTR::null(),
        lpDirectory: PCWSTR::null(),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut exec_info).map_err(|e| {
            let mut err = ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to open file {}: {e}", path.display()),
                "open_file_with_association",
            );
            err.native_code = Some(e.code().0 as u32);
            err
        })?;
    }

    Ok(())
}

/// Displays the native Windows Shell Properties modal dialog for a given file or directory.
pub fn show_file_properties(path: &Path) -> Result<(), ExplorerError> {
    if !path.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Target item does not exist: {}", path.display()),
            "show_file_properties",
        ));
    }

    let file_wide = path_to_wide(path);
    let verb_wide: Vec<u16> = "properties\0".encode_utf16().collect();

    let mut exec_info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST,
        hwnd: HWND::default(),
        lpVerb: PCWSTR(verb_wide.as_ptr()),
        lpFile: PCWSTR(file_wide.as_ptr()),
        lpParameters: PCWSTR::null(),
        lpDirectory: PCWSTR::null(),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut exec_info).map_err(|e| {
            let mut err = ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to display properties for {}: {e}", path.display()),
                "show_file_properties",
            );
            err.native_code = Some(e.code().0 as u32);
            err
        })?;
    }

    Ok(())
}

/// Opens the parent directory in Windows File Explorer with the item selected.
/// Uses argument vectors rather than shell interpolation.
pub fn open_in_windows_explorer(path: &Path) -> Result<(), ExplorerError> {
    if !path.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Target item does not exist: {}", path.display()),
            "open_in_windows_explorer",
        ));
    }

    let arg = format!("/select,{}", path.display());
    Command::new("explorer.exe")
        .arg(&arg)
        .spawn()
        .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to launch Windows Explorer: {e}"),
                "open_in_windows_explorer",
            )
        })?;

    Ok(())
}

/// Creates a new folder using Windows IFileOperation.
pub fn shell_create_folder(parent: &Path, folder_name: &str) -> Result<PathBuf, ExplorerError> {
    if !parent.is_dir() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Parent folder does not exist: {}", parent.display()),
            "shell_create_folder",
        ));
    }

    let parent_wide = path_to_wide(parent);
    let name_wide = path_to_wide(Path::new(folder_name));

    unsafe {
        let parent_item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(parent_wide.as_ptr()), None).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to bind parent ShellItem: {e}"),
                    "shell_create_folder",
                )
            })?;

        let file_op: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to create IFileOperation: {e}"),
                    "shell_create_folder",
                )
            })?;

        // Queue folder creation
        file_op
            .NewItem(
                &parent_item,
                FILE_ATTRIBUTE_DIRECTORY.0,
                PCWSTR(name_wide.as_ptr()),
                PCWSTR::null(),
                None,
            )
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to queue NewItem: {e}"),
                    "shell_create_folder",
                )
            })?;

        file_op.PerformOperations().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to perform folder creation: {e}"),
                "shell_create_folder",
            )
        })?;

        if file_op.GetAnyOperationsAborted().is_ok_and(|a| a.as_bool()) {
            return Err(ExplorerError::new(
                ErrorCode::Canceled,
                "Folder creation was canceled",
                "shell_create_folder",
            ));
        }
    }

    Ok(parent.join(folder_name))
}

/// Renames a file or folder using Windows IFileOperation.
pub fn shell_rename_item(source: &Path, new_name: &str) -> Result<PathBuf, ExplorerError> {
    if !source.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Source item does not exist: {}", source.display()),
            "shell_rename_item",
        ));
    }

    let src_wide = path_to_wide(source);
    let name_wide = path_to_wide(Path::new(new_name));

    unsafe {
        let src_item: IShellItem = SHCreateItemFromParsingName(PCWSTR(src_wide.as_ptr()), None)
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to bind source ShellItem: {e}"),
                    "shell_rename_item",
                )
            })?;

        let file_op: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to create IFileOperation: {e}"),
                    "shell_rename_item",
                )
            })?;

        // Queue rename operation
        file_op
            .RenameItem(&src_item, PCWSTR(name_wide.as_ptr()), None)
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to queue RenameItem: {e}"),
                    "shell_rename_item",
                )
            })?;

        file_op.PerformOperations().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to perform rename operation: {e}"),
                "shell_rename_item",
            )
        })?;

        if file_op.GetAnyOperationsAborted().is_ok_and(|a| a.as_bool()) {
            return Err(ExplorerError::new(
                ErrorCode::Canceled,
                "Rename operation was canceled",
                "shell_rename_item",
            ));
        }
    }

    let parent = source.parent().unwrap_or(source);
    Ok(parent.join(new_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::StaWorker;
    use tempfile::tempdir;

    #[test]
    fn test_shell_create_folder_and_rename_in_sta() {
        let sta = StaWorker::new("test-sta").expect("create STA");
        let dir = tempdir().expect("create temp dir");
        let parent_path = dir.path().to_path_buf();

        // Test create folder
        let created = sta
            .execute({
                let parent = parent_path.clone();
                move || shell_create_folder(&parent, "NewFolderTest")
            })
            .expect("create folder");

        assert!(created.is_dir());
        assert_eq!(created.file_name().unwrap(), "NewFolderTest");

        // Test rename folder
        let renamed = sta
            .execute({
                let src = created.clone();
                move || shell_rename_item(&src, "RenamedFolderTest")
            })
            .expect("rename folder");

        assert!(!created.exists());
        assert!(renamed.is_dir());
        assert_eq!(renamed.file_name().unwrap(), "RenamedFolderTest");
    }
}
