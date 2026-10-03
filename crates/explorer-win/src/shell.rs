use crate::path::path_to_wide;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::path::Path;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW};
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
