use crate::path::path_to_wide;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::path::PathBuf;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, POINT};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_MOVE};
use windows::Win32::UI::Shell::DROPFILES;
use windows::core::w;

const CF_HDROP_ID: u32 = 15;

/// Writes file paths to the Windows clipboard with CF_HDROP and Preferred DropEffect.
/// Interoperable with Windows Explorer copy/cut/paste.
pub fn write_clipboard_hdrop(paths: &[PathBuf], is_cut: bool) -> Result<(), ExplorerError> {
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
        let h_global: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, total_bytes).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("GlobalAlloc for DROPFILES failed: {e}"),
                "write_clipboard_hdrop",
            )
        })?;

        let ptr = GlobalLock(h_global);
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
        let _ = GlobalUnlock(h_global);

        // Prepare Preferred DropEffect (DROPEFFECT_COPY = 1, DROPEFFECT_MOVE = 2)
        let effect_value = if is_cut {
            DROPEFFECT_MOVE.0
        } else {
            DROPEFFECT_COPY.0
        };
        let h_effect: HGLOBAL =
            GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>()).map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("GlobalAlloc for Preferred DropEffect failed: {e}"),
                    "write_clipboard_hdrop",
                )
            })?;

        let effect_ptr = GlobalLock(h_effect);
        if !effect_ptr.is_null() {
            *(effect_ptr as *mut u32) = effect_value;
            let _ = GlobalUnlock(h_effect);
        }

        OpenClipboard(HWND::default()).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("OpenClipboard failed: {e}"),
                "write_clipboard_hdrop",
            )
        })?;

        let _ = EmptyClipboard();
        let _ = SetClipboardData(CF_HDROP_ID, HANDLE(h_global.0));

        let format_drop_effect = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if format_drop_effect != 0 && !h_effect.is_invalid() {
            let _ = SetClipboardData(format_drop_effect, HANDLE(h_effect.0));
        }

        let _ = CloseClipboard();
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

        let ptr = GlobalLock(HGLOBAL(handle.0));
        if ptr.is_null() {
            return Ok(None);
        }

        let dropfiles = *(ptr as *const DROPFILES);
        let p_files = dropfiles.pFiles as usize;
        let is_wide = dropfiles.fWide.as_bool();

        let mut paths = Vec::new();

        if is_wide {
            let data_ptr = (ptr as *const u8).add(p_files) as *const u16;
            let mut current = Vec::new();
            let mut idx = 0;
            const MAX_ENTRIES: usize = 10_000;
            const MAX_U16_SCAN: usize = 512 * 1024; // 1 MB bound

            while idx < MAX_U16_SCAN && paths.len() < MAX_ENTRIES {
                let ch = *data_ptr.add(idx);
                idx += 1;
                if ch == 0 {
                    if current.is_empty() {
                        // Double NUL indicates end of file list
                        break;
                    }
                    let path_str = String::from_utf16_lossy(&current);
                    paths.push(PathBuf::from(path_str));
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
        }

        let _ = GlobalUnlock(HGLOBAL(handle.0));

        let mut is_cut = false;
        let format_drop_effect = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if format_drop_effect != 0 && IsClipboardFormatAvailable(format_drop_effect).is_ok() {
            match GetClipboardData(format_drop_effect) {
                Ok(effect_handle) if !effect_handle.is_invalid() => {
                    let effect_ptr = GlobalLock(HGLOBAL(effect_handle.0));
                    if !effect_ptr.is_null() {
                        let effect = *(effect_ptr as *const u32);
                        if effect == DROPEFFECT_MOVE.0 {
                            is_cut = true;
                        }
                        let _ = GlobalUnlock(HGLOBAL(effect_handle.0));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let file1 = temp.path().join("clip1.txt");
        let file2 = temp.path().join("clip2.txt");
        std::fs::write(&file1, "one").unwrap();
        std::fs::write(&file2, "two").unwrap();

        let input_paths = vec![file1.clone(), file2.clone()];
        write_clipboard_hdrop(&input_paths, true).expect("write to clipboard");

        let read_result = read_clipboard_hdrop().expect("read clipboard");
        assert!(read_result.is_some());
        let (paths, is_cut) = read_result.unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], file1);
        assert_eq!(paths[1], file2);
        assert!(is_cut);
    }
}
