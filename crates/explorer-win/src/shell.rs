use crate::path::path_to_wide;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::path::Path;
use std::process::Command;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW, ShellExecuteExW,
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
