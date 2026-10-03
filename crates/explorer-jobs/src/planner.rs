//! Mutation plan creation and validation

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{CommitToken, PlanId};
use explorer_domain::operations::{OperationKind, OperationPlan};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validates that a filename complies with Windows naming restrictions.
pub fn validate_file_name(name: &str) -> Result<(), ExplorerError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "File or folder name cannot be empty",
            "validate_file_name",
        ));
    }

    if trimmed == "." || trimmed == ".." {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Name cannot be '.' or '..'",
            "validate_file_name",
        ));
    }

    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Name cannot end with a period or space",
            "validate_file_name",
        ));
    }

    if trimmed.len() > 255 {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Name exceeds maximum component limit of 255 characters",
            "validate_file_name",
        ));
    }

    for c in trimmed.chars() {
        if FORBIDDEN_CHARS.contains(&c) || (c as u32) < 32 {
            return Err(ExplorerError::new(
                ErrorCode::InvalidName,
                format!("Name contains invalid character: '{c}'"),
                "validate_file_name",
            ));
        }
    }

    // Check reserved names (e.g. "CON", "aux.txt")
    let base_stem = trimmed.split('.').next().unwrap_or(trimmed);
    for &reserved in RESERVED_NAMES {
        if base_stem.eq_ignore_ascii_case(reserved) {
            return Err(ExplorerError::new(
                ErrorCode::InvalidName,
                format!("'{trimmed}' is a reserved Windows device name"),
                "validate_file_name",
            ));
        }
    }

    Ok(())
}

/// Creates a validated, immutable plan for creating a new folder.
pub fn plan_create_folder(
    parent: &Path,
    folder_name: &str,
) -> Result<OperationPlan, ExplorerError> {
    if !parent.is_dir() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Destination directory does not exist: {}", parent.display()),
            "plan_create_folder",
        ));
    }

    validate_file_name(folder_name)?;

    let target_path = parent.join(folder_name);
    if target_path.exists() {
        return Err(ExplorerError::new(
            ErrorCode::AlreadyExists,
            format!("An item named '{folder_name}' already exists in this folder"),
            "plan_create_folder",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(OperationPlan {
        id: PlanId::new(),
        commit_token: CommitToken::new(),
        kind: OperationKind::CreateFolder,
        source_paths: Vec::new(),
        destination_path: Some(parent.to_path_buf()),
        target_name: Some(folder_name.to_string()),
        items_count: 1,
        expires_at: now + 300, // 5 minutes validity
    })
}

/// Creates a validated, immutable plan for renaming an existing file or folder.
pub fn plan_rename(source: &Path, new_name: &str) -> Result<OperationPlan, ExplorerError> {
    if !source.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Source item does not exist: {}", source.display()),
            "plan_rename",
        ));
    }

    validate_file_name(new_name)?;

    let parent = source.parent().unwrap_or(source);
    let target_path = parent.join(new_name);

    if target_path.exists() && target_path != source {
        return Err(ExplorerError::new(
            ErrorCode::AlreadyExists,
            format!("An item named '{new_name}' already exists in this folder"),
            "plan_rename",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(OperationPlan {
        id: PlanId::new(),
        commit_token: CommitToken::new(),
        kind: OperationKind::Rename,
        source_paths: vec![source.to_path_buf()],
        destination_path: Some(parent.to_path_buf()),
        target_name: Some(new_name.to_string()),
        items_count: 1,
        expires_at: now + 300,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_name_validation() {
        assert!(validate_file_name("valid_name.txt").is_ok());
        assert!(validate_file_name("My New Folder 2026").is_ok());
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name("   ").is_err());
        assert!(validate_file_name("CON").is_err());
        assert!(validate_file_name("aux.txt").is_err());
        assert!(validate_file_name("folder/sub").is_err());
        assert!(validate_file_name("bad:colon").is_err());
        assert!(validate_file_name("trailing.dot.").is_err());
    }

    #[test]
    fn test_plan_create_folder_and_rename() {
        let dir = tempdir().expect("create temp dir");
        let parent = dir.path();

        let plan = plan_create_folder(parent, "SubDir").expect("plan create");
        assert_eq!(plan.kind, OperationKind::CreateFolder);
        assert_eq!(plan.target_name.as_deref(), Some("SubDir"));

        // File creation
        let file_path = parent.join("test.txt");
        std::fs::write(&file_path, "hello").expect("write");

        let rename_plan = plan_rename(&file_path, "renamed.txt").expect("plan rename");
        assert_eq!(rename_plan.kind, OperationKind::Rename);
        assert_eq!(rename_plan.target_name.as_deref(), Some("renamed.txt"));
    }
}
