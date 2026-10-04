use crate::path::{path_to_wide, shell_path_to_wide};
use crate::sink::{ShellProgressSink, SinkItemCallback, SinkReport};
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

pub fn shell_create_folder(parent: &Path, name: &str) -> Result<PathBuf, ExplorerError> {
    single_output(shell_create_folder_with_callback(parent, name, None)?)
}
pub fn shell_rename_item(source: &Path, name: &str) -> Result<PathBuf, ExplorerError> {
    single_output(shell_rename_item_with_callback(source, name, None)?)
}
fn single_output(report: SinkReport) -> Result<PathBuf, ExplorerError> {
    let item = report.item_results.first();
    if let Some(item) = item
        && item.status == explorer_domain::operations::ItemStatus::Succeeded
        && let Some(path) = &item.actual_destination
    {
        return Ok(path.clone());
    }
    Err(ExplorerError::new(
        if report.was_aborted {
            ErrorCode::Canceled
        } else {
            ErrorCode::Internal
        },
        report
            .error_message
            .unwrap_or_else(|| "The Shell did not report a completed output".into()),
        "single_output",
    ))
}
pub fn shell_create_folder_with_callback(
    parent: &Path,
    name: &str,
    callback: Option<SinkItemCallback>,
) -> Result<SinkReport, ExplorerError> {
    shell_single_item(parent, name, true, callback)
}
pub fn shell_rename_item_with_callback(
    source: &Path,
    name: &str,
    callback: Option<SinkItemCallback>,
) -> Result<SinkReport, ExplorerError> {
    shell_single_item(source, name, false, callback)
}
fn shell_single_item(
    path: &Path,
    name: &str,
    create: bool,
    callback: Option<SinkItemCallback>,
) -> Result<SinkReport, ExplorerError> {
    let wide = shell_path_to_wide(path)?;
    let name = path_to_wide(Path::new(name));
    let native_error = |e: windows::core::Error| {
        let mut error = ExplorerError::new(
            ErrorCode::Internal,
            format!("Shell operation failed: {e}"),
            "shell_single_item",
        );
        error.native_code = Some(e.code().0 as u32);
        error
    };
    unsafe {
        let item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).map_err(native_error)?;
        if !create && path.is_dir() {
            // This host's IFileOperation folder RenameItem fails with ERROR_FILE_NOT_FOUND
            // before any callback. Use the native Shell folder transfer provider directly,
            // with TSF_NORMAL (no overwrite), and journal its real returned result.
            let parent_path = shell_path_to_wide(path.parent().ok_or_else(|| {
                ExplorerError::new(
                    ErrorCode::UnsupportedPath,
                    "Cannot rename a root",
                    "shell_single_item",
                )
            })?)?;
            let parent: IShellItem =
                SHCreateItemFromParsingName(PCWSTR(parent_path.as_ptr()), None)
                    .map_err(native_error)?;
            let transfer: windows::Win32::UI::Shell::ITransferSource = parent
                .BindToHandler(None, &windows::Win32::UI::Shell::BHID_Transfer)
                .map_err(native_error)?;
            let (_, tracker) = ShellProgressSink::new_with_callback(false, callback);
            let source = crate::path::ensure_extended_prefix(path);
            // Preserve positive Shell HRESULTs; the high-level binding discards them.
            use windows::core::Interface;
            let mut raw_output = std::ptr::null_mut();
            let hr = (Interface::vtable(&transfer).RenameItem)(
                Interface::as_raw(&transfer),
                Interface::as_raw(&item),
                PCWSTR(name.as_ptr()),
                windows::Win32::UI::Shell::TSF_NORMAL.0 as u32,
                &mut raw_output,
            );
            let output = (!raw_output.is_null()).then(|| IShellItem::from_raw(raw_output));
            tracker.record_provider_result(hr, source, crate::sink::shell_path(output.as_ref()));
            return Ok(tracker.report());
        }
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(native_error)?;
        operation
            .SetOperationFlags(FILEOPERATION_FLAGS(
                windows::Win32::UI::Shell::FOF_NOERRORUI.0 | FOF_ALLOWUNDO.0 | FOFX_ADDUNDORECORD.0,
            ))
            .map_err(native_error)?;
        let (sink, tracker) = ShellProgressSink::new_with_callback(false, callback);
        let sink: IFileOperationProgressSink = sink.into();
        let cookie = operation.Advise(&sink).map_err(native_error)?;
        let queued = if create {
            operation.NewItem(
                &item,
                FILE_ATTRIBUTE_DIRECTORY.0,
                PCWSTR(name.as_ptr()),
                PCWSTR::null(),
                None,
            )
        } else {
            operation.RenameItem(&item, PCWSTR(name.as_ptr()), None)
        };
        if let Err(error) = queued {
            let _ = operation.Unadvise(cookie);
            return Err(native_error(error));
        }
        let result = operation.PerformOperations();
        let aborted = operation
            .GetAnyOperationsAborted()
            .is_ok_and(|b| b.as_bool());
        let _ = operation.Unadvise(cookie);
        Ok(finalize_sink_report(
            tracker.report(),
            aborted,
            result,
            1,
            if create { "Create folder" } else { "Rename" },
        ))
    }
}

/// Copies one or more files/folders into a destination directory using Windows IFileOperation.
pub fn shell_copy_items(
    sources: &[PathBuf],
    destination_dir: &Path,
) -> Result<SinkReport, ExplorerError> {
    shell_copy_items_with_callback(sources, destination_dir, None)
}

pub fn shell_copy_items_with_callback(
    sources: &[PathBuf],
    destination_dir: &Path,
    item_callback: Option<SinkItemCallback>,
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

    let dest_wide = shell_path_to_wide(destination_dir)?;

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

        let (sink, tracker) = ShellProgressSink::new_with_callback(false, item_callback);
        let sink_interface: IFileOperationProgressSink = sink.into();
        let cookie = file_op.Advise(&sink_interface).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to advise progress sink: {e}"),
                "shell_copy_items",
            )
        })?;

        for src in sources {
            let src_wide = shell_path_to_wide(src)?;
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

        Ok(finalize_sink_report(
            tracker.report(),
            was_aborted,
            op_result,
            sources.len(),
            "Copy",
        ))
    }
}

/// Moves one or more files/folders into a destination directory using Windows IFileOperation.
pub fn shell_move_items(
    sources: &[PathBuf],
    destination_dir: &Path,
) -> Result<SinkReport, ExplorerError> {
    shell_move_items_with_callback(sources, destination_dir, None)
}

pub fn shell_move_items_with_callback(
    sources: &[PathBuf],
    destination_dir: &Path,
    item_callback: Option<SinkItemCallback>,
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

    let dest_wide = shell_path_to_wide(destination_dir)?;

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

        let (sink, tracker) = ShellProgressSink::new_with_callback(false, item_callback);
        let sink_interface: IFileOperationProgressSink = sink.into();
        let cookie = file_op.Advise(&sink_interface).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to advise progress sink: {e}"),
                "shell_move_items",
            )
        })?;

        for src in sources {
            let src_wide = shell_path_to_wide(src)?;
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

        Ok(finalize_sink_report(
            tracker.report(),
            was_aborted,
            op_result,
            sources.len(),
            "Move",
        ))
    }
}

/// Recycles one or more files/folders using Windows IFileOperation with guarded PreDeleteItem check.
/// Rejects UNC paths before submission and never silently falls back to permanent deletion.
pub fn shell_recycle_items(sources: &[PathBuf]) -> Result<SinkReport, ExplorerError> {
    shell_recycle_items_with_callback(sources, None)
}

pub fn shell_recycle_items_with_callback(
    sources: &[PathBuf],
    item_callback: Option<SinkItemCallback>,
) -> Result<SinkReport, ExplorerError> {
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
        if matches!(src.components().next(), Some(std::path::Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::UNC(_, _) | std::path::Prefix::VerbatimUNC(_, _)))
        {
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

    for source in sources {
        let normal = crate::path::strip_extended_prefix(source);
        let volume = normal.ancestors().last().ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::RecycleUnsupported,
                "Volume unavailable",
                "shell_recycle_items",
            )
        })?;
        let volume = path_to_wide(volume);
        let mut info = windows::Win32::UI::Shell::SHQUERYRBINFO {
            cbSize: std::mem::size_of::<windows::Win32::UI::Shell::SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        unsafe { windows::Win32::UI::Shell::SHQueryRecycleBinW(PCWSTR(volume.as_ptr()), &mut info) }.map_err(|e| {
            let mut error = ExplorerError::new(ErrorCode::RecycleUnsupported, format!("Recycle support could not be verified for this volume: {e}. Source left intact; permanent fallback is forbidden."), "shell_recycle_items");
            error.native_code = Some(e.code().0 as u32); error
        })?;
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

        let (sink, tracker) = ShellProgressSink::new_with_callback(true, item_callback);
        let sink_interface: IFileOperationProgressSink = sink.into();
        let cookie = file_op.Advise(&sink_interface).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to advise progress sink: {e}"),
                "shell_recycle_items",
            )
        })?;

        for src in sources {
            let src_wide = shell_path_to_wide(src)?;
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

        Ok(finalize_sink_report(
            tracker.report(),
            was_aborted,
            op_result,
            sources.len(),
            "Recycle",
        ))
    }
}

fn finalize_sink_report(
    mut report: SinkReport,
    shell_aborted: bool,
    operation_result: windows::core::Result<()>,
    expected_items: usize,
    operation_name: &str,
) -> SinkReport {
    if let Err(error) = operation_result {
        report.native_error = Some(error.code().0 as u32);
        let canceled = crate::sink::classify_hresult(error.code())
            == explorer_domain::operations::ItemStatus::Canceled;
        report.was_aborted = canceled;
        if !canceled {
            let accounted = report.completed.saturating_add(report.failed);
            report.failed = report
                .failed
                .saturating_add(expected_items.saturating_sub(accounted));
        }
        report.error_message = Some(format!(
            "{operation_name} stopped with native error: {error}"
        ));
    } else {
        report.was_aborted |= shell_aborted;
        if report.was_aborted && report.error_message.is_none() {
            report.error_message = Some(format!("{operation_name} operation was canceled"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::StaWorker;
    fn tempdir() -> std::io::Result<tempfile::TempDir> {
        let root = tempfile::tempdir()?;
        std::fs::write(
            root.path().join(".rust-explorer-fixture-root"),
            "native Shell fixture",
        )?;
        Ok(root)
    }

    #[test]
    fn native_failure_is_not_misreported_as_user_cancellation() {
        let native = windows::core::HRESULT(0x80070002u32 as i32);
        let report = finalize_sink_report(
            SinkReport::default(),
            true,
            Err(windows::core::Error::from_hresult(native)),
            1,
            "Rename",
        );
        assert!(!report.was_aborted);
        assert_eq!(report.failed, 1);
        assert_eq!(report.native_error, Some(0x80070002));
    }

    #[test]
    fn test_directory_provider_rename_preserves_both_sides_on_collision() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(source.join("source.txt"), "source intact").unwrap();
        std::fs::write(target.join("target.txt"), "target intact").unwrap();
        let sta = StaWorker::new("directory-collision-sta").unwrap();
        let result = sta.execute({
            let source = source.clone();
            move || shell_rename_item(&source, "target")
        });
        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(source.join("source.txt")).unwrap(),
            "source intact"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("target.txt")).unwrap(),
            "target intact"
        );
    }

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
        let recycle_result = sta.execute(move || shell_recycle_items(&recycle_sources));
        match recycle_result {
            Ok(report) => {
                assert_eq!(report.completed, 1, "{report:#?}");
                assert!(!recycle_target.exists());
            }
            Err(error) => {
                assert_eq!(error.code, ErrorCode::RecycleUnsupported);
                assert!(error.native_code.is_some());
                assert!(recycle_target.exists());
                eprintln!(
                    "CAPABILITY SKIP: successful recycling unavailable; unsupported volume rejected before submission: {error}"
                );
            }
        }
    }
}
