//! Metadata-only mutation preflight. Never follow links or hydrate placeholders.
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::operations::OperationKind;
use explorer_win::identity::get_file_identity;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const FIXTURE_MARKER: &str = ".rust-explorer-fixture-root";
pub struct MutationPolicy {
    protected: Vec<PathBuf>,
}

impl MutationPolicy {
    pub fn for_application() -> Result<Self, ExplorerError> {
        let exe = std::env::current_exe().map_err(|e| error(ErrorCode::Internal, e.to_string()))?;
        let state = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| error(ErrorCode::Internal, "Durable state location is unavailable"))?
            .join("RustExplorer");
        Self::new(vec![
            exe.parent()
                .ok_or_else(|| error(ErrorCode::Internal, "Application directory unavailable"))?
                .to_path_buf(),
            state,
        ])
    }
    pub fn new(protected: Vec<PathBuf>) -> Result<Self, ExplorerError> {
        Ok(Self {
            protected: protected
                .iter()
                .map(|p| resolve_location(p))
                .collect::<Result<_, _>>()?,
        })
    }
    pub fn prepare(
        &self,
        kind: OperationKind,
        sources: &[PathBuf],
        destination: Option<&Path>,
        canceled: &AtomicBool,
    ) -> Result<Vec<PathBuf>, ExplorerError> {
        let started = Instant::now();
        if let Some(dest) = destination {
            self.check_entry(dest, false)?;
        }
        let mut resolved = Vec::new();
        let mut existing_outputs = Vec::new();
        for source in sources {
            check_budget(canceled, started)?;
            self.check_entry(source, true)?;
            let source = resolve_location(source)?;
            if kind == OperationKind::Recycle && is_unc(&source) {
                return Err(error(
                    ErrorCode::RecycleUnsupported,
                    format!(
                        "Network recycling is unsupported: {}. Permanent deletion is forbidden.",
                        source.display()
                    ),
                ));
            }
            if let Some(dest) = destination {
                if source.is_dir() && is_within(&source, dest)? {
                    return Err(error(
                        ErrorCode::UnsupportedPath,
                        "Destination is the source directory or one of its descendants",
                    ));
                }
                if matches!(kind, OperationKind::Copy | OperationKind::Move)
                    && let Some(parent) = source.parent()
                    && same_directory(parent, dest)?
                {
                    return Err(error(
                        ErrorCode::AlreadyExists,
                        "Source is already in the destination directory",
                    ));
                }
                if matches!(kind, OperationKind::Copy | OperationKind::Move) {
                    let target = dest.join(source.file_name().ok_or_else(|| {
                        error(ErrorCode::UnsupportedPath, "Source name unavailable")
                    })?);
                    match std::fs::symlink_metadata(&target) {
                        Ok(_) => {
                            self.check_entry(&target, true)?;
                            existing_outputs.push(target);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            self.check_protected_location(&resolve_location(&target)?, false)?;
                        }
                        Err(error) => {
                            return Err(crate::policy::error(
                                ErrorCode::AccessDenied,
                                format!(
                                    "Destination tree is not validated at {}: {error}",
                                    target.display()
                                ),
                            ));
                        }
                    }
                }
            }
            // Canonical entry spelling, not file ID alone: preserve distinct hard links.
            if !resolved.contains(&source) {
                resolved.push(source);
            }
        }
        let directories: Vec<_> = resolved.iter().filter(|p| p.is_dir()).collect();
        let mut selected = Vec::new();
        for source in &resolved {
            let mut covered = false;
            for parent in &directories {
                check_budget(canceled, started)?;
                if *parent != source && is_within(parent, source)? {
                    covered = true;
                    break;
                }
            }
            if !covered {
                selected.push(source.clone());
            }
        }
        if kind != OperationKind::CreateFolder {
            let mut pending = selected.clone();
            pending.extend(existing_outputs);
            let mut seen = 0;
            while let Some(path) = pending.pop() {
                check_budget(canceled, started)?;
                seen += 1;
                if seen > 250_000 {
                    return Err(error(
                        ErrorCode::QueueFull,
                        "Tree exceeds 250,000 preflight entries; split the operation. No items submitted.",
                    ));
                }
                if checked_metadata(&path)?.is_dir() {
                    let entries = std::fs::read_dir(&path).map_err(|e| {
                        error(
                            ErrorCode::AccessDenied,
                            format!("Tree is not validated at {}: {e}", path.display()),
                        )
                    })?;
                    for entry in entries {
                        check_budget(canceled, started)?;
                        if pending.len() >= 16_384 {
                            return Err(error(
                                ErrorCode::QueueFull,
                                "Tree preflight queue is full; split the operation. No items submitted.",
                            ));
                        }
                        let entry = entry.map_err(|e| {
                            error(
                                ErrorCode::AccessDenied,
                                format!("Tree is not validated: {e}"),
                            )
                        })?;
                        if entry.file_name() == FIXTURE_MARKER {
                            return Err(error(
                                ErrorCode::UnsupportedPath,
                                format!("Active fixture boundary is protected: {}", path.display()),
                            ));
                        }
                        pending.push(entry.path());
                    }
                }
            }
        }
        Ok(selected)
    }
    fn check_entry(&self, path: &Path, source: bool) -> Result<(), ExplorerError> {
        explorer_win::path::validate_safe_path(path)?;
        if !path.is_absolute() {
            return Err(error(
                ErrorCode::UnsupportedPath,
                "Mutation paths must be absolute",
            ));
        }
        if !matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _)))
        {
            return Err(error(
                ErrorCode::UnsupportedPath,
                "Only drive and UNC filesystem namespaces are supported for mutation",
            ));
        }
        for ancestor in path.ancestors() {
            checked_metadata(ancestor)?;
        }
        let path = resolve_location(path)?;
        if source && path.file_name().is_some_and(|name| name == FIXTURE_MARKER) {
            return Err(error(
                ErrorCode::UnsupportedPath,
                "Fixture marker is protected",
            ));
        }
        if source && path.parent().is_none() {
            return Err(error(
                ErrorCode::UnsupportedPath,
                format!("Drive/share root is protected: {}", path.display()),
            ));
        }
        if source && path.is_dir() && path.join(FIXTURE_MARKER).is_file() {
            return Err(error(
                ErrorCode::UnsupportedPath,
                format!("Active fixture boundary is protected: {}", path.display()),
            ));
        }
        self.check_protected_location(&path, source)
    }
    fn check_protected_location(&self, path: &Path, source: bool) -> Result<(), ExplorerError> {
        for protected in &self.protected {
            if is_within(protected, path)?
                || (source && path.is_dir() && is_within(path, protected)?)
            {
                return Err(error(
                    ErrorCode::UnsupportedPath,
                    format!(
                        "Protected application/state location: {}",
                        protected.display()
                    ),
                ));
            }
        }
        Ok(())
    }
}
fn checked_metadata(path: &Path) -> Result<std::fs::Metadata, ExplorerError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| {
        error(
            ErrorCode::AccessDenied,
            format!("Cannot validate {}: {e}", path.display()),
        )
    })?;
    validate_attributes(metadata.file_attributes(), path)?;
    Ok(metadata)
}
pub fn validate_attributes(attributes: u32, path: &Path) -> Result<(), ExplorerError> {
    if attributes & (0x1000 | 0x40000 | 0x400000) != 0 {
        return Err(error(
            ErrorCode::PlaceholderUnavailable,
            format!(
                "Unavailable placeholder is unsupported for mutation: {}",
                path.display()
            ),
        ));
    }
    if attributes & 0x400 != 0 {
        return Err(error(
            ErrorCode::UnsupportedReparsePoint,
            format!("Link/reparse mutation is not verified: {}", path.display()),
        ));
    }
    Ok(())
}
pub fn is_unc(path: &Path) -> bool {
    matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _)))
}
fn resolve_location(path: &Path) -> Result<PathBuf, ExplorerError> {
    if path.exists() {
        return path.canonicalize().map_err(|e| {
            error(
                ErrorCode::UnsupportedPath,
                format!("Cannot resolve {}: {e}", path.display()),
            )
        });
    }
    let parent = path.parent().ok_or_else(|| {
        error(
            ErrorCode::UnsupportedPath,
            "Cannot resolve protected location",
        )
    })?;
    Ok(resolve_location(parent)?.join(
        path.file_name()
            .ok_or_else(|| error(ErrorCode::UnsupportedPath, "Missing path component"))?,
    ))
}
pub fn same_directory(a: &Path, b: &Path) -> Result<bool, ExplorerError> {
    let a = get_file_identity(a)?;
    let b = get_file_identity(b)?;
    Ok(a.volume_serial == b.volume_serial && a.file_index == b.file_index)
}
pub fn is_within(root: &Path, path: &Path) -> Result<bool, ExplorerError> {
    let root = resolve_location(root)?;
    let path = resolve_location(path)?;
    if !root.exists() {
        return Ok(path.starts_with(&root));
    }
    // With reparse ancestors refused, a shallower canonical path cannot equal root.
    // Avoid requesting attributes on unrelated, potentially ACL-protected parents.
    let depth = root.components().count();
    for ancestor in path
        .ancestors()
        .take_while(|p| p.components().count() >= depth)
        .filter(|p| p.is_dir())
    {
        if same_directory(&root, ancestor)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn check_budget(canceled: &AtomicBool, started: Instant) -> Result<(), ExplorerError> {
    if canceled.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(10) {
        return Err(error(
            ErrorCode::Canceled,
            "Tree preflight canceled or timed out; the operation is not validated and no items were submitted",
        ));
    }
    Ok(())
}
fn error(code: ErrorCode, message: impl Into<String>) -> ExplorerError {
    ExplorerError::new(code, message, "MutationPolicy::preflight")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(FIXTURE_MARKER), "mutation safety fixture").unwrap();
        root
    }
    #[test]
    fn preflight_protects_roots_and_outputs_and_reduces_selection() {
        let root = fixture();
        let source = root.path().join("source");
        let dest = root.path().join("destination");
        let state = dest.join("source");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let child = source.join("nested/file.txt");
        std::fs::write(&child, "safe").unwrap();
        let canceled = AtomicBool::new(false);
        let policy = MutationPolicy::new(vec![state.clone()]).unwrap();
        assert!(
            policy
                .prepare(
                    OperationKind::Copy,
                    std::slice::from_ref(&source),
                    Some(&dest),
                    &canceled
                )
                .is_err()
        );
        assert!(
            policy
                .prepare(
                    OperationKind::Rename,
                    std::slice::from_ref(&state),
                    Some(&dest),
                    &canceled
                )
                .is_err()
        );
        let policy = MutationPolicy::new(vec![]).unwrap();
        let selected = policy
            .prepare(
                OperationKind::Copy,
                &[child.clone(), source.clone(), child],
                Some(&dest),
                &canceled,
            )
            .unwrap();
        assert_eq!(selected, vec![source.canonicalize().unwrap()]);
        assert!(
            policy
                .prepare(
                    OperationKind::Move,
                    std::slice::from_ref(&source),
                    Some(&source.join("nested")),
                    &canceled
                )
                .is_err()
        );
        assert!(
            policy
                .prepare(
                    OperationKind::Recycle,
                    &[root.path().to_path_buf()],
                    None,
                    &canceled
                )
                .is_err()
        );
        canceled.store(true, Ordering::Relaxed);
        assert_eq!(
            policy
                .prepare(OperationKind::Copy, &[source], Some(&dest), &canceled)
                .unwrap_err()
                .code,
            ErrorCode::Canceled
        );
    }
    #[test]
    fn preflight_preserves_hard_link_entries_and_uses_directory_identity() {
        let root = fixture();
        let src = root.path().join("Source");
        let dest = root.path().join("dest");
        std::fs::create_dir(&src).unwrap();
        std::fs::create_dir(&dest).unwrap();
        let a = src.join("a");
        let b = src.join("b");
        std::fs::write(&a, "hard links").unwrap();
        std::fs::hard_link(&a, &b).unwrap();
        let policy = MutationPolicy::new(vec![]).unwrap();
        assert_eq!(
            policy
                .prepare(
                    OperationKind::Copy,
                    &[a.clone(), b],
                    Some(&dest),
                    &AtomicBool::new(false)
                )
                .unwrap()
                .len(),
            2
        );
        let alias = root.path().join("source");
        assert!(
            policy
                .prepare(
                    OperationKind::Move,
                    &[a],
                    Some(&alias),
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
    #[test]
    fn preflight_rejects_junction_tree_and_parent_without_traversal() {
        let root = fixture();
        let tree = root.path().join("tree");
        let outside = root.path().join("outside");
        let dest = root.path().join("dest");
        std::fs::create_dir(&tree).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::create_dir(&dest).unwrap();
        let file = outside.join("intact.txt");
        std::fs::write(&file, "intact").unwrap();
        let junction = tree.join("junction");
        let output = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction fixture creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let policy = MutationPolicy::new(vec![]).unwrap();
        assert_eq!(
            policy
                .prepare(
                    OperationKind::Copy,
                    &[tree],
                    Some(&dest),
                    &AtomicBool::new(false)
                )
                .unwrap_err()
                .code,
            ErrorCode::UnsupportedReparsePoint
        );
        assert!(
            policy
                .prepare(
                    OperationKind::Rename,
                    &[junction.join("intact.txt")],
                    Some(&junction),
                    &AtomicBool::new(false)
                )
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(file).unwrap(), "intact");
        std::fs::remove_dir(&junction).unwrap();
    }
    #[test]
    fn preflight_classifies_cloud_flags_and_native_unc_prefixes() {
        assert_eq!(
            validate_attributes(0x400, Path::new("fixture"))
                .unwrap_err()
                .code,
            ErrorCode::UnsupportedReparsePoint
        );
        for flag in [0x1000, 0x40000, 0x400000] {
            assert_eq!(
                validate_attributes(flag, Path::new("fixture"))
                    .unwrap_err()
                    .code,
                ErrorCode::PlaceholderUnavailable
            );
        }
        assert!(!is_unc(Path::new(r"\\?\D:\fixture")));
        assert!(is_unc(Path::new(r"\\?\UNC\server\share\fixture")));
        assert!(is_unc(Path::new(r"\\server\share\fixture")));
    }
}
