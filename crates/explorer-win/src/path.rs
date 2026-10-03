use std::path::{Path, PathBuf};

/// Converts a Path to a null-terminated UTF-16 vector for Windows APIs.
pub fn path_to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Converts a null-terminated UTF-16 slice to a PathBuf losslessly.
pub fn wide_to_path(wide: &[u16]) -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    PathBuf::from(OsString::from_wide(&wide[..len]))
}
