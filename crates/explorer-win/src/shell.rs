use crate::path::path_to_wide;
use crate::sink::{ShellProgressSink, SinkReport};
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::UI::Shell::{
    FILEOPERATION_FLAGS, FOF_ALLOWUNDO, FOF_WANTNUKEWARNING, FOFX_ADDUNDORECORD,
    FOFX_RECYCLEONDELETE, FileOperation, IFileOperation, IFileOperationProgressSink, IShellItem,
    SEE_MASK_FLAG_NO_UI, SEE_MASK_INVOKEIDLIST, SHCreateItemFromParsingName, SHELLEXECUTEINFOW,
    ShellExecuteExW,
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

/// Copies one or more files/folders into a destination directory using Windows IFileOperation.
pub fn shell_copy_items(
    sources: &[PathBuf],
    destination_dir: &Path,
) -> Result<SinkReport, ExplorerError> {
    if sources.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "No items to copy",
            "shell_copy_items",
        ));
    }
    if !destination_dir.exists() || !destination_dir.is_dir() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!(
                "Destination folder does not exist: {}",
                destination_dir.display()
            ),
            "shell_copy_items",
        ));
    }

    let dest_wide = path_to_wide(destination_dir);

    unsafe {
        let dest_item: IShellItem = SHCreateItemFromParsingName(PCWSTR(dest_wide.as_ptr()), None)
            .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to bind destination folder: {e}"),
                "shell_copy_items",
            )
        })?;

        let file_op: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to create IFileOperation: {e}"),
                    "shell_copy_items",
                )
            })?;

        let flags = FILEOPERATION_FLAGS(FOF_ALLOWUNDO.0 | FOFX_ADDUNDORECORD.0);
        file_op.SetOperationFlags(flags).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to set copy flags: {e}"),
                "shell_copy_items",
            )
        })?;

        let (sink, tracker) = ShellProgressSink::new(false);
        let sink_interface: IFileOperationProgressSink = sink.into();
        let cookie = file_op.Advise(&sink_interface).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to advise progress sink: {e}"),
                "shell_copy_items",
            )
        })?;

        for src in sources {
            let src_wide = path_to_wide(src);
            let src_item: IShellItem =
                match SHCreateItemFromParsingName(PCWSTR(src_wide.as_ptr()), None) {
                    Ok(item) => item,
                    Err(e) => {
                        let _ = file_op.Unadvise(cookie);
                        return Err(ExplorerError::new(
                            ErrorCode::NotFound,
                            format!("Failed to bind source {}: {e}", src.display()),
                            "shell_copy_items",
                        ));
                    }
                };

            file_op
                .CopyItem(&src_item, &dest_item, PCWSTR::null(), None)
                .map_err(|e| {
                    let _ = file_op.Unadvise(cookie);
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to queue CopyItem for {}: {e}", src.display()),
                        "shell_copy_items",
                    )
                })?;
        }

        let op_result = file_op.PerformOperations();
        let was_aborted = file_op.GetAnyOperationsAborted().is_ok_and(|a| a.as_bool());
        let _ = file_op.Unadvise(cookie);

        let report = tracker.report();

        if report.was_aborted || was_aborted {
            let msg = report
                .error_message
                .unwrap_or_else(|| "Copy operation was canceled".to_string());
            return Err(ExplorerError::new(
                ErrorCode::Canceled,
                msg,
                "shell_copy_items",
            ));
        }

        if let Err(e) = op_result {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to perform copy operations: {e}"),
                "shell_copy_items",
            ));
        }

        Ok(report)
    }
}

/// Moves one or more files/folders into a destination directory using Windows IFileOperation.
pub fn shell_move_items(
    sources: &[PathBuf],
    destination_dir: &Path,
) -> Result<SinkReport, ExplorerError> {
    if sources.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "No items to move",
            "shell_move_items",
        ));
    }
    if !destination_dir.exists() || !destination_dir.is_dir() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!(
                "Destination folder does not exist: {}",
                destination_dir.display()
            ),
            "shell_move_items",
        ));
    }

    let dest_wide = path_to_wide(destination_dir);

    unsafe {
        let dest_item: IShellItem = SHCreateItemFromParsingName(PCWSTR(dest_wide.as_ptr()), None)
            .map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to bind destination folder: {e}"),
                "shell_move_items",
            )
        })?;

        let file_op: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to create IFileOperation: {e}"),
                    "shell_move_items",
                )
            })?;

        let flags = FILEOPERATION_FLAGS(FOF_ALLOWUNDO.0 | FOFX_ADDUNDORECORD.0);
        file_op.SetOperationFlags(flags).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to set move flags: {e}"),
                "shell_move_items",
            )
        })?;

        let (sink, tracker) = ShellProgressSink::new(false);
        let sink_interface: IFileOperationProgressSink = sink.into();
        let cookie = file_op.Advise(&sink_interface).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to advise progress sink: {e}"),
                "shell_move_items",
            )
        })?;

        for src in sources {
            let src_wide = path_to_wide(src);
            let src_item: IShellItem =
                match SHCreateItemFromParsingName(PCWSTR(src_wide.as_ptr()), None) {
                    Ok(item) => item,
                    Err(e) => {
                        let _ = file_op.Unadvise(cookie);
                        return Err(ExplorerError::new(
                            ErrorCode::NotFound,
                            format!("Failed to bind source {}: {e}", src.display()),
                            "shell_move_items",
                        ));
                    }
                };

            file_op
                .MoveItem(&src_item, &dest_item, PCWSTR::null(), None)
                .map_err(|e| {
                    let _ = file_op.Unadvise(cookie);
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("Failed to queue MoveItem for {}: {e}", src.display()),
                        "shell_move_items",
                    )
                })?;
        }

        let op_result = file_op.PerformOperations();
        let was_aborted = file_op.GetAnyOperationsAborted().is_ok_and(|a| a.as_bool());
        let _ = file_op.Unadvise(cookie);

        let report = tracker.report();

        if report.was_aborted || was_aborted {
            let msg = report
                .error_message
                .unwrap_or_else(|| "Move operation was canceled".to_string());
            return Err(ExplorerError::new(
                ErrorCode::Canceled,
                msg,
                "shell_move_items",
            ));
        }

        if let Err(e) = op_result {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to perform move operations: {e}"),
                "shell_move_items",
            ));
        }

        Ok(report)
    }
}

/// Recycles one or more files/folders using Windows IFileOperation with guarded PreDeleteItem check.
/// Rejects UNC paths before submission and never silently falls back to permanent deletion.
pub fn shell_recycle_items(sources: &[PathBuf]) -> Result<SinkReport, ExplorerError> {
    if sources.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "No items to recycle",
            "shell_recycle_items",
        ));
    }

    // Safety guard: reject UNC paths as Recycle Bin does not support UNC paths
    for src in sources {
        let s = src.to_string_lossy();
        if s.starts_with(r"\\") || s.starts_with("//") {
            return Err(ExplorerError::new(
                ErrorCode::RecycleUnsupported,
                format!(
                    "Recycling is not supported for network/UNC path: {s}. Permanent deletion fallback is strictly forbidden."
                ),
                "shell_recycle_items",
            ));
        }
        if !src.exists() {
            return Err(ExplorerError::new(
                ErrorCode::NotFound,
                format!("Source item does not exist: {}", src.display()),
                "shell_recycle_items",
            ));
        }
    }

    unsafe {
        let file_op: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to create IFileOperation: {e}"),
                    "shell_recycle_items",
                )
            })?;

        let flags = FILEOPERATION_FLAGS(
            FOFX_RECYCLEONDELETE.0 | FOF_WANTNUKEWARNING.0 | FOFX_ADDUNDORECORD.0 | FOF_ALLOWUNDO.0,
        );
        file_op.SetOperationFlags(flags).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to set recycle flags: {e}"),
                "shell_recycle_items",
            )
        })?;

        let (sink, tracker) = ShellProgressSink::new(true);
        let sink_interface: IFileOperationProgressSink = sink.into();
        let cookie = file_op.Advise(&sink_interface).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to advise progress sink: {e}"),
                "shell_recycle_items",
            )
        })?;

        for src in sources {
            let src_wide = path_to_wide(src);
            let src_item: IShellItem =
                match SHCreateItemFromParsingName(PCWSTR(src_wide.as_ptr()), None) {
                    Ok(item) => item,
                    Err(e) => {
                        let _ = file_op.Unadvise(cookie);
                        return Err(ExplorerError::new(
                            ErrorCode::NotFound,
                            format!("Failed to bind source ShellItem for {}: {e}", src.display()),
                            "shell_recycle_items",
                        ));
                    }
                };

            file_op.DeleteItem(&src_item, None).map_err(|e| {
                let _ = file_op.Unadvise(cookie);
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to queue DeleteItem: {e}"),
                    "shell_recycle_items",
                )
            })?;
        }

        let op_result = file_op.PerformOperations();
        let was_aborted = file_op.GetAnyOperationsAborted().is_ok_and(|a| a.as_bool());
        let _ = file_op.Unadvise(cookie);

        let report = tracker.report();

        if report.was_aborted || was_aborted {
            let msg = report
                .error_message
                .unwrap_or_else(|| "Recycle operation was canceled or aborted".to_string());
            return Err(ExplorerError::new(
                ErrorCode::Canceled,
                msg,
                "shell_recycle_items",
            ));
        }

        if let Err(e) = op_result {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to perform recycle operations: {e}"),
                "shell_recycle_items",
            ));
        }

        Ok(report)
    }
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

    #[test]
    fn test_shell_copy_move_and_recycle_in_sta() {
        let sta = StaWorker::new("test-sta-m4").expect("create STA");
        let dir = tempdir().expect("create temp dir");
        let parent_path = dir.path().to_path_buf();

        let sub1 = parent_path.join("sub1");
        let sub2 = parent_path.join("sub2");
        std::fs::create_dir(&sub1).unwrap();
        std::fs::create_dir(&sub2).unwrap();

        let test_file = sub1.join("item.txt");
        std::fs::write(&test_file, "hello world").unwrap();

        // Copy item.txt from sub1 to sub2
        let copy_sources = vec![test_file.clone()];
        let copy_dest = sub2.clone();
        let copy_report = sta
            .execute(move || shell_copy_items(&copy_sources, &copy_dest))
            .expect("shell copy");
        assert_eq!(copy_report.completed, 1);
        assert!(test_file.exists());
        assert!(sub2.join("item.txt").exists());

        // Move item.txt in sub2 to sub1/item_moved.txt (via rename or move)
        // Move copied item to sub1 under same name (rename source first)
        let file_in_sub2 = sub2.join("item.txt");
        let renamed_in_sub2 = sta
            .execute({
                let f = file_in_sub2.clone();
                move || shell_rename_item(&f, "item_moved.txt")
            })
            .expect("rename");
        assert!(renamed_in_sub2.exists());

        let move_sources = vec![renamed_in_sub2.clone()];
        let move_dest = sub1.clone();
        let move_report = sta
            .execute(move || shell_move_items(&move_sources, &move_dest))
            .expect("shell move");
        assert_eq!(move_report.completed, 1);
        assert!(!renamed_in_sub2.exists());
        assert!(sub1.join("item_moved.txt").exists());

        // Recycle item_moved.txt
        let recycle_target = sub1.join("item_moved.txt");
        let recycle_sources = vec![recycle_target.clone()];
        let recycle_report = sta
            .execute(move || shell_recycle_items(&recycle_sources))
            .expect("shell recycle");
        assert_eq!(recycle_report.completed, 1);
        assert!(!recycle_target.exists());
    }
}
