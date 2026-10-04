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
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    let unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
    let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    if units.starts_with(&unc) {
        let mut normal: Vec<u16> = r"\\".encode_utf16().collect();
        normal.extend_from_slice(&units[unc.len()..]);
        PathBuf::from(OsString::from_wide(&normal))
    } else if units.starts_with(&prefix) {
        PathBuf::from(OsString::from_wide(&units[prefix.len()..]))
    } else {
        path.to_path_buf()
    }
}

/// Shell parsing needs a normal filesystem path; retain every UTF-16 code unit.
/// Exceptional component endings must never be silently normalized by the Shell.
pub fn shell_path_to_wide(path: &Path) -> Result<Vec<u16>, ExplorerError> {
    let path = strip_extended_prefix(path);
    validate_safe_path(&path)?;
    for component in path.components() {
        if let Component::Normal(name) = component {
            let units: Vec<u16> = name.encode_wide().collect();
            if matches!(units.last(), Some(32 | 46)) {
                return Err(ExplorerError::new(
                    ErrorCode::UnsupportedPath,
                    "Shell mutation does not support trailing spaces/dots in existing names",
                    "shell_path_to_wide",
                ));
            }
        }
    }
    Ok(path_to_wide(&path))
}

/// Ensures that a bare drive root path (e.g. `C:` or `\\?\C:`) has a trailing backslash (`C:\` or `\\?\C:\`).
/// In Win32 API, `\\?\C:` is invalid syntax, and `C:` refers to relative current-drive directory.
/// Explicit navigation or enumeration of a drive root must always be targeted as a directory root with `\`.
pub fn normalize_drive_root(path: &Path) -> PathBuf {
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    let is_drive_letter = |u: u16| (65..=90).contains(&u) || (97..=122).contains(&u);

    // Bare drive: e.g. "C:" or "c:" (units.len() == 2)
    if units.len() == 2 && is_drive_letter(units[0]) && units[1] == 58 {
        let mut fixed = units;
        fixed.push(92);
        return PathBuf::from(OsString::from_wide(&fixed));
    }

    // Extended bare drive: e.g. r"\\?\C:" or r"\\?\c:" (units.len() == 6)
    let extended_prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    if units.len() == 6
        && units.starts_with(&extended_prefix)
        && is_drive_letter(units[4])
        && units[5] == 58
    {
        let mut fixed = units;
        fixed.push(92);
        return PathBuf::from(OsString::from_wide(&fixed));
    }

    // Device bare drive: e.g. r"\\.\C:" or r"\\.\c:" (units.len() == 6)
    let device_prefix: Vec<u16> = r"\\.\".encode_utf16().collect();
    if units.len() == 6
        && units.starts_with(&device_prefix)
        && is_drive_letter(units[4])
        && units[5] == 58
    {
        let mut fixed = units;
        fixed.push(92);
        return PathBuf::from(OsString::from_wide(&fixed));
    }

    path.to_path_buf()
}

/// Appends `\\?\` or `\\?\UNC\` prefix if needed for paths exceeding MAX_PATH (260 characters).
pub fn ensure_extended_prefix(path: &Path) -> PathBuf {
    let normalized = normalize_drive_root(path);
    let units: Vec<u16> = normalized.as_os_str().encode_wide().collect();
    let starts_with = |prefix: &str| units.starts_with(&prefix.encode_utf16().collect::<Vec<_>>());
    if starts_with(r"\\?\") {
        return normalized;
    }

    let mut prefixed: Vec<u16>;
    if starts_with(r"\\") {
        // UNC path: \\server\share -> \\?\UNC\server\share
        prefixed = r"\\?\UNC\".encode_utf16().collect();
        prefixed.extend_from_slice(&units[2..]);
    } else {
        // Normal absolute drive path: C:\foo -> \\?\C:\foo
        prefixed = r"\\?\".encode_utf16().collect();
        prefixed.extend_from_slice(&units);
    }
    PathBuf::from(OsString::from_wide(&prefixed))
}

/// Validates that a path is safe for navigation and mutation.
/// Rejects device paths (\\.\), embedded NULs, and Alternate Data Streams (:).
pub fn validate_safe_path(path: &Path) -> Result<(), ExplorerError> {
    let unextended = strip_extended_prefix(path);
    let path_str = unextended.to_string_lossy();
    let trimmed = path_str.trim();

    if trimmed.contains('\0') {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Path contains embedded null characters",
            "validate_path",
        ));
    }

    if trimmed.starts_with(r"\\.\") {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "Device namespace paths (\\\\.\\) are not supported",
            "validate_path",
        ));
    }

    // Check for ADS (Alternate Data Streams) e.g., "file.txt:stream"
    // We allow drive letters "C:\" at position 1.
    for (i, c) in trimmed.char_indices() {
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

        let mut native: Vec<u16> = r"C:\fixture\".encode_utf16().collect();
        native.push(0xD800); // Existing unpaired surrogate must retain its native identity.
        native.extend(".txt".encode_utf16());
        let exceptional = PathBuf::from(OsString::from_wide(&native));
        let prefixed = ensure_extended_prefix(&exceptional);
        assert_eq!(strip_extended_prefix(&prefixed), exceptional);
        native.push(0);
        assert_eq!(shell_path_to_wide(&prefixed).unwrap(), native);
    }

    #[test]
    fn test_path_safety_validation() {
        assert!(validate_safe_path(Path::new(r"C:\Users\John\Documents")).is_ok());
        assert!(validate_safe_path(Path::new(r"\\?\C:\Users\John\Documents")).is_ok());
        assert!(validate_safe_path(Path::new(r"\\.\PhysicalDrive0")).is_err());
        assert!(validate_safe_path(Path::new(r"C:\folder\CON.txt")).is_err());
        assert!(validate_safe_path(Path::new(r"C:\file.txt:hidden")).is_err());
        assert!(validate_safe_path(Path::new(r"\\?\C:\file.txt:hidden")).is_err());
    }

    #[test]
    fn test_normalize_drive_root() {
        assert_eq!(normalize_drive_root(Path::new("C:")), PathBuf::from(r"C:\"));
        assert_eq!(normalize_drive_root(Path::new("d:")), PathBuf::from(r"d:\"));
        assert_eq!(
            normalize_drive_root(Path::new(r"\\?\C:")),
            PathBuf::from(r"\\?\C:\")
        );
        assert_eq!(
            normalize_drive_root(Path::new(r"\\?\D:")),
            PathBuf::from(r"\\?\D:\")
        );
        assert_eq!(
            normalize_drive_root(Path::new(r"\\.\C:")),
            PathBuf::from(r"\\.\C:\")
        );
        // Already normalized should remain unchanged
        assert_eq!(
            normalize_drive_root(Path::new(r"C:\")),
            PathBuf::from(r"C:\")
        );
        assert_eq!(
            normalize_drive_root(Path::new(r"\\?\C:\")),
            PathBuf::from(r"\\?\C:\")
        );
        // Subpaths should remain unchanged
        assert_eq!(
            normalize_drive_root(Path::new(r"C:\Users")),
            PathBuf::from(r"C:\Users")
        );
        assert_eq!(
            normalize_drive_root(Path::new(r"\\?\C:\Users")),
            PathBuf::from(r"\\?\C:\Users")
        );
    }
}
