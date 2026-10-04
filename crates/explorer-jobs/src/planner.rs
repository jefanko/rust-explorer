//! Mutation plan creation and validation

use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{CommitToken, PlanId};
use explorer_domain::operations::{FileIdentity, OperationKind, OperationPlan};
use explorer_fs::policy::MutationPolicy;
use explorer_win::identity::get_file_identity;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validates that a filename complies with Windows naming restrictions.
pub fn validate_file_name(name: &str) -> Result<(), ExplorerError> {
    if name.is_empty() || name.trim().is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "File or folder name cannot be empty",
            "validate_file_name",
        ));
    }

    if name == "." || name == ".." {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Name cannot be '.' or '..'",
            "validate_file_name",
        ));
    }

    if name.ends_with('.') || name.ends_with(' ') {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Name cannot end with a period or space",
            "validate_file_name",
        ));
    }

    if name.encode_utf16().count() > 255 {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Name exceeds maximum component limit of 255 characters",
            "validate_file_name",
        ));
    }

    for c in name.chars() {
        if FORBIDDEN_CHARS.contains(&c) || (c as u32) < 32 {
            return Err(ExplorerError::new(
                ErrorCode::InvalidName,
                format!("Name contains invalid character: '{c}'"),
                "validate_file_name",
            ));
        }
    }

    // Check reserved names (e.g. "CON", "aux.txt")
    let base_stem = name.split('.').next().unwrap_or(name);
    for &reserved in RESERVED_NAMES {
        if base_stem.eq_ignore_ascii_case(reserved) {
            return Err(ExplorerError::new(
                ErrorCode::InvalidName,
                format!("'{name}' is a reserved Windows device name"),
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

    MutationPolicy::for_application()?.prepare(
        OperationKind::CreateFolder,
        &[],
        Some(parent),
        &AtomicBool::new(false),
    )?;
    let native_parent = parent.canonicalize().map_err(|e| {
        ExplorerError::new(
            ErrorCode::UnsupportedPath,
            e.to_string(),
            "plan_create_folder",
        )
    })?;
    let parent = native_parent.as_path();
    validate_file_name(folder_name)?;
    MutationPolicy::for_application()?.prepare(
        OperationKind::CreateFolder,
        &[],
        Some(parent),
        &AtomicBool::new(false),
    )?;

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
        source_identities: Vec::new(),
        source_parent_identities: Vec::new(),
        destination_identity: Some(get_file_identity(parent)?),
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

    // Policy must inspect the original ancestors before resolution hides links.
    MutationPolicy::for_application()?.prepare(
        OperationKind::Rename,
        &[source.to_path_buf()],
        source.parent(),
        &AtomicBool::new(false),
    )?;
    let native_source = source.canonicalize().map_err(|e| {
        ExplorerError::new(ErrorCode::UnsupportedPath, e.to_string(), "plan_rename")
    })?;
    let source = native_source.as_path();
    validate_file_name(new_name)?;

    let parent = source.parent().unwrap_or(source);
    MutationPolicy::for_application()?.prepare(
        OperationKind::Rename,
        &[source.to_path_buf()],
        Some(parent),
        &AtomicBool::new(false),
    )?;
    let target_path = parent.join(new_name);
    if source.file_name().is_some_and(|name| name == new_name) {
        return Err(ExplorerError::new(
            ErrorCode::AlreadyExists,
            "Rename source and destination are identical",
            "plan_rename",
        ));
    }
    // Allow a case-only rename of the same entry; distinct entries/hard links still conflict.
    if target_path.exists()
        && target_path
            .canonicalize()
            .map_err(|e| ExplorerError::new(ErrorCode::StaleItem, e.to_string(), "plan_rename"))?
            != source
    {
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
        source_identities: vec![get_file_identity(source)?],
        source_parent_identities: vec![get_file_identity(parent)?],
        destination_identity: Some(get_file_identity(parent)?),
        target_name: Some(new_name.to_string()),
        items_count: 1,
        expires_at: now + 300,
    })
}

/// Create conservative, metadata-preflighted transfer plans.
pub fn plan_copy(sources: &[PathBuf], destination: &Path) -> Result<OperationPlan, ExplorerError> {
    plan_transfer(OperationKind::Copy, sources, Some(destination))
}
pub fn plan_move(sources: &[PathBuf], destination: &Path) -> Result<OperationPlan, ExplorerError> {
    plan_transfer(OperationKind::Move, sources, Some(destination))
}
pub fn plan_recycle(sources: &[PathBuf]) -> Result<OperationPlan, ExplorerError> {
    plan_transfer(OperationKind::Recycle, sources, None)
}
fn plan_transfer(
    kind: OperationKind,
    sources: &[PathBuf],
    destination: Option<&Path>,
) -> Result<OperationPlan, ExplorerError> {
    if sources.is_empty() || sources.len() > 10_000 {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "Select between 1 and 10,000 entries; split oversized operations",
            "plan_transfer",
        ));
    }
    if destination.is_some_and(|p| !p.is_dir()) {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            "Destination directory is unavailable",
            "plan_transfer",
        ));
    }
    let sources = MutationPolicy::for_application()?.prepare(
        kind,
        sources,
        destination,
        &AtomicBool::new(false),
    )?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(OperationPlan {
        id: PlanId::new(),
        commit_token: CommitToken::new(),
        kind,
        source_identities: capture_identities(&sources)?,
        source_parent_identities: sources
            .iter()
            .map(|p| get_file_identity(p.parent().expect("preflight excludes roots")))
            .collect::<Result<_, _>>()?,
        destination_path: destination.map(Path::to_path_buf),
        destination_identity: destination.map(get_file_identity).transpose()?,
        items_count: sources.len(),
        source_paths: sources,
        target_name: None,
        expires_at: now + 300,
    })
}

fn capture_identities(paths: &[PathBuf]) -> Result<Vec<FileIdentity>, ExplorerError> {
    paths.iter().map(|path| get_file_identity(path)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tempdir() -> std::io::Result<tempfile::TempDir> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join(explorer_fs::policy::FIXTURE_MARKER),
            "rust-explorer mutation fixture",
        )?;
        Ok(dir)
    }

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
        assert!(plan_rename(&file_path, "test.txt").is_err());
        assert!(plan_rename(&file_path, "TEST.txt").is_ok());

        let rename_plan = plan_rename(&file_path, "renamed.txt").expect("plan rename");
        assert_eq!(rename_plan.kind, OperationKind::Rename);
        assert_eq!(rename_plan.target_name.as_deref(), Some("renamed.txt"));
    }

    #[test]
    fn test_plan_copy_move_and_recycle() {
        let dir = tempdir().expect("create temp dir");
        let root = dir.path();

        let d1 = root.join("d1");
        let d2 = root.join("d2");
        std::fs::create_dir(&d1).unwrap();
        std::fs::create_dir(&d2).unwrap();

        let file = d1.join("item.txt");
        std::fs::write(&file, "data").unwrap();

        // Plan copy
        let copy_plan = plan_copy(std::slice::from_ref(&file), &d2).expect("plan copy");
        assert_eq!(copy_plan.kind, OperationKind::Copy);
        assert_eq!(copy_plan.items_count, 1);

        // Plan move
        let move_plan = plan_move(std::slice::from_ref(&file), &d2).expect("plan move");
        assert_eq!(move_plan.kind, OperationKind::Move);

        // Moving into own child must fail
        assert!(plan_move(std::slice::from_ref(&d1), &d1.join("nested")).is_err());

        // Plan recycle
        let recycle_plan = plan_recycle(std::slice::from_ref(&file)).expect("plan recycle");
        assert_eq!(recycle_plan.kind, OperationKind::Recycle);

        // Recycling UNC path must be rejected
        let unc = PathBuf::from(r"\\server\share\file.txt");
        assert!(plan_recycle(&[unc]).is_err());
    }
}
