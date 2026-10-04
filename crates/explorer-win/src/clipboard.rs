use crate::path::path_to_wide;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, POINT};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_MOVE};
use windows::Win32::UI::Shell::DROPFILES;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow};
use windows::core::w;

const CF_HDROP_ID: u32 = 15;
const CF_UNICODETEXT_ID: u32 = 13;

struct GlobalHandle(Option<HGLOBAL>);

impl GlobalHandle {
    fn new(handle: HGLOBAL) -> Self {
        Self(Some(handle))
    }

    fn get(&self) -> HGLOBAL {
        self.0.expect("global handle already transferred")
    }

    fn transfer(mut self) -> HGLOBAL {
        self.0.take().expect("global handle already transferred")
    }
}

impl Drop for GlobalHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            unsafe {
                let _ = GlobalFree(handle);
            }
        }
    }
}

/// Writes file paths to the Windows clipboard with CF_HDROP and Preferred DropEffect.
/// Interoperable with Windows Explorer copy/cut/paste.
pub fn write_clipboard_hdrop(
    paths: &[PathBuf],
    is_cut: bool,
    owner: usize,
) -> Result<(), ExplorerError> {
    let owner = validate_owner(owner)?;
    if paths.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "Cannot copy empty list of paths to clipboard",
            "write_clipboard_hdrop",
        ));
    }

    if paths.len() > 10_000 {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "Cannot copy more than 10,000 entries to clipboard",
            "write_clipboard_hdrop",
        ));
    }

    // Build double-NUL terminated UTF-16 stream
    let mut wide_data: Vec<u16> = Vec::new();
    for p in paths {
        let wide = path_to_wide(p);
        wide_data.extend_from_slice(&wide); // wide already ends with 0
    }
    wide_data.push(0); // Extra trailing NUL for double-NUL termination

    let header_size = std::mem::size_of::<DROPFILES>();
    let total_bytes = header_size + (wide_data.len() * std::mem::size_of::<u16>());

    unsafe {
        let h_global = GlobalHandle::new(GlobalAlloc(GMEM_MOVEABLE, total_bytes).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("GlobalAlloc for DROPFILES failed: {e}"),
                "write_clipboard_hdrop",
            )
        })?);

        let ptr = GlobalLock(h_global.get());
        if ptr.is_null() {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                "GlobalLock for DROPFILES failed",
                "write_clipboard_hdrop",
            ));
        }

        let dropfiles = DROPFILES {
            pFiles: header_size as u32,
            pt: POINT { x: 0, y: 0 },
            fNC: false.into(),
            fWide: true.into(),
        };

        std::ptr::copy_nonoverlapping(
            &dropfiles as *const DROPFILES as *const u8,
            ptr as *mut u8,
            header_size,
        );

        let data_dest = (ptr as *mut u8).add(header_size) as *mut u16;
        std::ptr::copy_nonoverlapping(wide_data.as_ptr(), data_dest, wide_data.len());
        let _ = GlobalUnlock(h_global.get());

        // Prepare Preferred DropEffect (DROPEFFECT_COPY = 1, DROPEFFECT_MOVE = 2)
        let effect_value = if is_cut {
            DROPEFFECT_MOVE.0
        } else {
            DROPEFFECT_COPY.0
        };
        let h_effect = GlobalHandle::new(
            GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>()).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("GlobalAlloc for Preferred DropEffect failed: {e}"),
                    "write_clipboard_hdrop",
                )
            })?,
        );

        let effect_ptr = GlobalLock(h_effect.get());
        if effect_ptr.is_null() {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                "GlobalLock for Preferred DropEffect failed",
                "write_clipboard_hdrop",
            ));
        }
        *(effect_ptr as *mut u32) = effect_value;
        let _ = GlobalUnlock(h_effect.get());

        let format_drop_effect = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if format_drop_effect == 0 {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                "RegisterClipboardFormatW for Preferred DropEffect failed",
                "write_clipboard_hdrop",
            ));
        }

        OpenClipboard(owner).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("OpenClipboard failed: {e}"),
                "write_clipboard_hdrop",
            )
        })?;

        struct ClipboardGuard;
        impl Drop for ClipboardGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = CloseClipboard();
                }
            }
        }
        let _guard = ClipboardGuard;

        EmptyClipboard().map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("EmptyClipboard failed: {e}"),
                "write_clipboard_hdrop",
            )
        })?;

        SetClipboardData(format_drop_effect, HANDLE(h_effect.get().0)).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("SetClipboardData for Preferred DropEffect failed: {e}"),
                "write_clipboard_hdrop",
            )
        })?;
        let _ = h_effect.transfer();

        if let Err(e) = SetClipboardData(CF_HDROP_ID, HANDLE(h_global.get().0)) {
            // The effect handle is now clipboard-owned. Clear it so a failed cut cannot
            // be observed by other applications as an incomplete MOVE clipboard.
            let _ = EmptyClipboard();
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                format!("SetClipboardData for CF_HDROP failed: {e}"),
                "write_clipboard_hdrop",
            ));
        }
        let _ = h_global.transfer();
    }

    Ok(())
}

/// Writes one exact UTF-16 string to the clipboard as text, not as a file-drop list.
pub fn write_clipboard_text_utf16(text_utf16: &[u16], owner: usize) -> Result<(), ExplorerError> {
    let owner = validate_owner(owner)?;
    if text_utf16.is_empty() || text_utf16.contains(&0) {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Clipboard text is empty or contains an embedded NUL",
            "write_clipboard_text_utf16",
        ));
    }
    if text_utf16.len() > 32_767 {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "Clipboard path exceeds the supported Windows path length",
            "write_clipboard_text_utf16",
        ));
    }

    let mut units = Vec::with_capacity(text_utf16.len() + 1);
    units.extend_from_slice(text_utf16);
    units.push(0);

    unsafe {
        let global = GlobalHandle::new(
            GlobalAlloc(GMEM_MOVEABLE, units.len() * std::mem::size_of::<u16>()).map_err(
                |error| {
                    ExplorerError::new(
                        ErrorCode::Internal,
                        format!("GlobalAlloc for clipboard text failed: {error}"),
                        "write_clipboard_text_utf16",
                    )
                },
            )?,
        );
        let ptr = GlobalLock(global.get());
        if ptr.is_null() {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                "GlobalLock for clipboard text failed",
                "write_clipboard_text_utf16",
            ));
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr as *mut u16, units.len());
        let _ = GlobalUnlock(global.get());

        OpenClipboard(owner).map_err(|error| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("OpenClipboard failed: {error}"),
                "write_clipboard_text_utf16",
            )
        })?;

        struct ClipboardGuard;
        impl Drop for ClipboardGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = CloseClipboard();
                }
            }
        }
        let _guard = ClipboardGuard;

        EmptyClipboard().map_err(|error| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("EmptyClipboard failed: {error}"),
                "write_clipboard_text_utf16",
            )
        })?;
        SetClipboardData(CF_UNICODETEXT_ID, HANDLE(global.get().0)).map_err(|error| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("SetClipboardData for CF_UNICODETEXT failed: {error}"),
                "write_clipboard_text_utf16",
            )
        })?;
        let _ = global.transfer();
    }

    Ok(())
}

/// Reads file paths and cut intent from the Windows clipboard.
pub fn read_clipboard_hdrop() -> Result<Option<(Vec<PathBuf>, bool)>, ExplorerError> {
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP_ID).is_err() {
            return Ok(None);
        }

        OpenClipboard(HWND::default()).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("OpenClipboard failed: {e}"),
                "read_clipboard_hdrop",
            )
        })?;

        struct ClipboardGuard;
        impl Drop for ClipboardGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = CloseClipboard();
                }
            }
        }
        let _guard = ClipboardGuard;

        let handle = match GetClipboardData(CF_HDROP_ID) {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };

        if handle.is_invalid() {
            return Ok(None);
        }

        let global = HGLOBAL(handle.0);
        let allocation_size = GlobalSize(global);
        let header_size = std::mem::size_of::<DROPFILES>();
        if allocation_size < header_size {
            return Err(ExplorerError::new(
                ErrorCode::UnsupportedPath,
                "Clipboard DROPFILES header is truncated",
                "read_clipboard_hdrop",
            ));
        }

        let ptr = GlobalLock(global);
        if ptr.is_null() {
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                "GlobalLock for CF_HDROP failed",
                "read_clipboard_hdrop",
            ));
        }

        struct GlobalUnlockGuard(HGLOBAL);
        impl Drop for GlobalUnlockGuard {
            fn drop(&mut self) {
                unsafe {
                    let _ = GlobalUnlock(self.0);
                }
            }
        }
        let _unlock_guard = GlobalUnlockGuard(global);

        let dropfiles = *(ptr as *const DROPFILES);
        let p_files = dropfiles.pFiles as usize;
        if p_files < header_size || p_files > allocation_size {
            return Err(ExplorerError::new(
                ErrorCode::UnsupportedPath,
                "Clipboard DROPFILES path offset is out of bounds",
                "read_clipboard_hdrop",
            ));
        }
        if !dropfiles.fWide.as_bool() {
            return Err(ExplorerError::new(
                ErrorCode::UnsupportedPath,
                "ANSI CF_HDROP clipboard data is not supported",
                "read_clipboard_hdrop",
            ));
        }
        if !p_files.is_multiple_of(std::mem::align_of::<u16>()) {
            return Err(ExplorerError::new(
                ErrorCode::UnsupportedPath,
                "Clipboard DROPFILES path offset is not UTF-16 aligned",
                "read_clipboard_hdrop",
            ));
        }

        const MAX_ENTRIES: usize = 10_000;
        const MAX_U16_SCAN: usize = 512 * 1024; // 1 MiB bound
        const MAX_PATH_UNITS: usize = 32_767;
        let available_units = (allocation_size - p_files) / std::mem::size_of::<u16>();
        let scan_units = available_units.min(MAX_U16_SCAN);
        let data_ptr = (ptr as *const u8).add(p_files) as *const u16;
        let mut current = Vec::new();
        let mut paths = Vec::new();
        let mut terminated = false;

        for idx in 0..scan_units {
            let ch = *data_ptr.add(idx);
            if ch == 0 {
                if current.is_empty() {
                    terminated = true;
                    break;
                }
                paths.push(PathBuf::from(OsString::from_wide(&current)));
                current.clear();
                if paths.len() > MAX_ENTRIES {
                    return Err(ExplorerError::new(
                        ErrorCode::UnsupportedPath,
                        "Clipboard contains more than 10,000 paths",
                        "read_clipboard_hdrop",
                    ));
                }
            } else {
                current.push(ch);
                if current.len() > MAX_PATH_UNITS {
                    return Err(ExplorerError::new(
                        ErrorCode::UnsupportedPath,
                        "Clipboard path exceeds the supported Windows path length",
                        "read_clipboard_hdrop",
                    ));
                }
            }
        }

        if !terminated {
            let message = if available_units > MAX_U16_SCAN {
                "Clipboard path list exceeds the 1 MiB import limit"
            } else {
                "Clipboard DROPFILES path list is missing its double-NUL terminator"
            };
            return Err(ExplorerError::new(
                ErrorCode::UnsupportedPath,
                message,
                "read_clipboard_hdrop",
            ));
        }

        let mut is_cut = false;
        let format_drop_effect = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if format_drop_effect != 0 && IsClipboardFormatAvailable(format_drop_effect).is_ok() {
            match GetClipboardData(format_drop_effect) {
                Ok(effect_handle) if !effect_handle.is_invalid() => {
                    let effect_global = HGLOBAL(effect_handle.0);
                    if GlobalSize(effect_global) >= std::mem::size_of::<u32>() {
                        let effect_ptr = GlobalLock(effect_global);
                        if !effect_ptr.is_null() {
                            let effect = *(effect_ptr as *const u32);
                            if effect == DROPEFFECT_MOVE.0 {
                                is_cut = true;
                            }
                            let _ = GlobalUnlock(effect_global);
                        }
                    }
                }
                _ => {}
            }
        }

        if paths.is_empty() {
            Ok(None)
        } else {
            Ok(Some((paths, is_cut)))
        }
    }
}

fn validate_owner(owner: usize) -> Result<HWND, ExplorerError> {
    let owner = HWND(owner as *mut _);
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(owner, Some(&mut pid));
    }
    if owner.0.is_null() || pid != std::process::id() || !unsafe { IsWindow(owner) }.as_bool() {
        return Err(ExplorerError::new(
            ErrorCode::Internal,
            "Clipboard writes require a live window owned by this application",
            "clipboard::validate_owner",
        ));
    }
    Ok(owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_rejects_null_owner_before_opening() {
        assert!(write_clipboard_text_utf16(&[65], 0).is_err());
        assert!(write_clipboard_hdrop(&[PathBuf::from(r"C:\fixture.txt")], false, 0).is_err());
    }

    #[test]
    fn test_clipboard_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".rust-explorer-fixture-root"),
            "clipboard fixture",
        )
        .unwrap();
        let file1 = temp.path().join("clip1.txt");
        let file2 = temp.path().join("clip2.txt");
        std::fs::write(&file1, "one").unwrap();
        std::fs::write(&file2, "two").unwrap();

        let input_paths = vec![file1.clone(), file2.clone()];
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
        };
        let owner = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Rust Explorer clipboard test"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                HWND::default(),
                None,
                None,
                None,
            )
        }
        .expect("clipboard owner");
        struct Owner(HWND);
        impl Drop for Owner {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let _owner = Owner(owner);
        write_clipboard_hdrop(&input_paths, true, owner.0 as usize).expect("write to clipboard");

        let read_result = read_clipboard_hdrop().expect("read clipboard");
        assert!(read_result.is_some());
        let (paths, is_cut) = read_result.unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], file1);
        assert_eq!(paths[1], file2);
        assert!(is_cut);
    }
}
