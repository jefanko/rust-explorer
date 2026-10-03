use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

/// Converts a Path to a null-terminated UTF-16 vector for Windows APIs.
pub fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Converts a null-terminated UTF-16 slice to a PathBuf losslessly.
pub fn wide_to_path(wide: &[u16]) -> PathBuf {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    PathBuf::from(OsString::from_wide(&wide[..len]))
}

/// Losslessly formats path for display in UI.
pub fn to_display_string(path: &Path) -> String {
    let stripped = strip_extended_prefix(path);
    stripped.to_string_lossy().to_string()
}

/// Strips `\\?\` and `\\?\UNC\` extended path prefixes for clean display.
pub fn strip_extended_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(unc) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", unc))
    } else if let Some(drive) = s.strip_prefix(r"\\?\") {
        PathBuf::from(drive)
    } else {
        path.to_path_buf()
    }
}

/// Appends `\\?\` or `\\?\UNC\` prefix if needed for paths exceeding MAX_PATH (260 characters).
pub fn ensure_extended_prefix(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with(r"\\?\") {
        return path.to_path_buf();
    }

    if let Some(unc_body) = path_str.strip_prefix(r"\\") {
        // UNC path: \\server\share -> \\?\UNC\server\share
        PathBuf::from(format!(r"\\?\UNC\{}", unc_body))
    } else {
        // Normal absolute drive path: C:\foo -> \\?\C:\foo
        PathBuf::from(format!(r"\\?\{}", path_str))
    }
}

/// Validates that a path is safe for navigation and mutation.
/// Rejects device paths (\\.\), embedded NULs, and Alternate Data Streams (:).
pub fn validate_safe_path(path: &Path) -> Result<(), ExplorerError> {
    let path_str = path.to_string_lossy();

    if path_str.contains('\0') {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Path contains embedded null characters",
            "validate_path",
        ));
    }

    if path_str.starts_with(r"\\.\") {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "Device namespace paths (\\\\.\\) are not supported",
            "validate_path",
        ));
    }

    // Check for ADS (Alternate Data Streams) e.g., "file.txt:stream"
    // We allow drive letters "C:\" at position 1.
    for (i, c) in path_str.char_indices() {
        if c == ':' && i != 1 {
            return Err(ExplorerError::new(
                ErrorCode::UnsupportedPath,
                "Alternate Data Streams syntax (:) is not supported",
                "validate_path",
            ));
        }
    }

    // Check components for reserved names or invalid traversal
    for comp in path.components() {
        if let Component::Normal(os_name) = comp {
            let name = os_name.to_string_lossy();
            if is_dos_device_name(&name) {
                return Err(ExplorerError::new(
                    ErrorCode::InvalidName,
                    format!("Path contains reserved Windows device name: {name}"),
                    "validate_path",
                ));
            }
        }
    }

    Ok(())
}

/// Checks if a component is a reserved DOS device name (CON, PRN, AUX, NUL, COM1..9, LPT1..9).
pub fn is_dos_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_roundtrip() {
        let p = PathBuf::from(r"C:\Windows\System32\drivers");
        let wide = path_to_wide(&p);
        let back = wide_to_path(&wide);
        assert_eq!(p, back);
    }

    #[test]
    fn test_extended_prefix_handling() {
        let drive_path = Path::new(r"C:\Users\test");
        let extended = ensure_extended_prefix(drive_path);
        assert_eq!(extended.to_str().unwrap(), r"\\?\C:\Users\test");
        let stripped = strip_extended_prefix(&extended);
        assert_eq!(stripped, drive_path);

        let unc = Path::new(r"\\server\share\file.txt");
        let extended_unc = ensure_extended_prefix(unc);
        assert_eq!(
            extended_unc.to_str().unwrap(),
            r"\\?\UNC\server\share\file.txt"
        );
        let stripped_unc = strip_extended_prefix(&extended_unc);
        assert_eq!(stripped_unc, unc);
    }

    #[test]
    fn test_path_safety_validation() {
        assert!(validate_safe_path(Path::new(r"C:\Users\John\Documents")).is_ok());
        assert!(validate_safe_path(Path::new(r"\\.\PhysicalDrive0")).is_err());
        assert!(validate_safe_path(Path::new(r"C:\folder\CON.txt")).is_err());
        assert!(validate_safe_path(Path::new(r"C:\file.txt:hidden")).is_err());
    }
}
