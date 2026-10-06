//! Release version consistency check.
//!
//! The product version lives in several manifests that must always agree, otherwise
//! the release tag, installer file names and `BUILD_METADATA.json` disagree with
//! each other. This module extracts the version from each manifest and compares them.
//!
//! The parsers are intentionally tiny (no extra dependencies) and only understand
//! what is needed: the `version` key of `[workspace.package]` in a Cargo manifest and
//! the top-level `"version"` key of a JSON document.

use std::fs;
use std::path::Path;

/// Manifests (relative to the repository root) that must carry the same version.
const SOURCES: &[(&str, SourceKind)] = &[
    ("Cargo.toml", SourceKind::CargoWorkspace),
    ("package.json", SourceKind::Json),
    ("apps/desktop/package.json", SourceKind::Json),
    ("apps/desktop/src-tauri/tauri.conf.json", SourceKind::Json),
];

#[derive(Clone, Copy, Debug)]
enum SourceKind {
    CargoWorkspace,
    Json,
}

/// Reads all version sources under `repo_root` and verifies they are identical.
///
/// Returns the common version on success, or a multi-line human readable message
/// listing every source and its version on failure.
pub fn check_repo(repo_root: &Path) -> Result<String, String> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut problems: Vec<String> = Vec::new();

    for (rel, kind) in SOURCES {
        let path = repo_root.join(rel);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{rel}: cannot read ({e})"));
                continue;
            }
        };
        let version = match kind {
            SourceKind::CargoWorkspace => cargo_workspace_version(&text),
            SourceKind::Json => json_top_level_version(&text),
        };
        match version {
            Some(v) => found.push(((*rel).to_string(), v)),
            None => problems.push(format!("{rel}: no version found")),
        }
    }

    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    compare_versions(&found)
}

/// Compares `(source, version)` pairs. Ok(common version) if all are identical.
pub fn compare_versions(entries: &[(String, String)]) -> Result<String, String> {
    let Some((_, first)) = entries.first() else {
        return Err("no version sources to compare".to_string());
    };
    if entries.iter().all(|(_, v)| v == first) {
        return Ok(first.clone());
    }
    let width = entries.iter().map(|(s, _)| s.len()).max().unwrap_or(0);
    let mut msg = String::from("version mismatch across manifests:");
    for (source, version) in entries {
        msg.push_str(&format!("\n    {source:<width$}  {version}"));
    }
    msg.push_str("\n    Update every file above to the same version before releasing.");
    Err(msg)
}

/// Extracts `version = "x"` from the `[workspace.package]` table of a Cargo manifest.
pub fn cargo_workspace_version(text: &str) -> Option<String> {
    let mut in_section = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_section = line.split('#').next().unwrap_or("").trim() == "[workspace.package]";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "version" {
            continue;
        }
        let value = value.split('#').next().unwrap_or("").trim();
        let unquoted = value.strip_prefix('"')?.strip_suffix('"')?;
        return Some(unquoted.to_string());
    }
    None
}

/// Extracts the string value of the top-level (depth 1) `"version"` key of a JSON object.
pub fn json_top_level_version(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut depth = 0usize;
    while i < chars.len() {
        match chars[i] {
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            '"' => {
                let (s, next) = read_json_string(&chars, i)?;
                i = next;
                if depth == 1 && s == "version" {
                    let mut j = i;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if chars.get(j) == Some(&':') {
                        j += 1;
                        while j < chars.len() && chars[j].is_whitespace() {
                            j += 1;
                        }
                        if chars.get(j) == Some(&'"') {
                            return read_json_string(&chars, j).map(|(v, _)| v);
                        }
                        return None;
                    }
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Reads a JSON string starting at the opening quote at `start`.
/// Returns the (minimally unescaped) value and the index just past the closing quote.
fn read_json_string(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '"' => return Some((out, i + 1)),
            '\\' => {
                i += 1;
                out.push(*chars.get(i)?);
            }
            c => out.push(c),
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(s, v)| ((*s).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn compare_accepts_identical_versions() {
        let e = entries(&[("a", "1.2.3"), ("b", "1.2.3"), ("c", "1.2.3")]);
        assert_eq!(compare_versions(&e), Ok("1.2.3".to_string()));
    }

    #[test]
    fn compare_reports_every_source_on_mismatch() {
        let e = entries(&[("Cargo.toml", "0.1.0"), ("package.json", "0.2.0")]);
        let err = compare_versions(&e).unwrap_err();
        assert!(err.contains("version mismatch"));
        assert!(err.contains("Cargo.toml"));
        assert!(err.contains("0.1.0"));
        assert!(err.contains("package.json"));
        assert!(err.contains("0.2.0"));
    }

    #[test]
    fn compare_rejects_empty_input() {
        assert!(compare_versions(&[]).is_err());
    }

    #[test]
    fn cargo_version_is_read_from_workspace_package_only() {
        let toml = "[workspace]\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\nversion = \"0.3.1\" # comment\n\n[workspace.dependencies]\nserde = { version = \"1.0\" }\n";
        assert_eq!(cargo_workspace_version(toml), Some("0.3.1".to_string()));
    }

    #[test]
    fn cargo_version_ignores_other_sections() {
        let toml =
            "[package]\nversion = \"9.9.9\"\n\n[workspace.dependencies]\nversion = \"1.0\"\n";
        assert_eq!(cargo_workspace_version(toml), None);
    }

    #[test]
    fn json_version_is_top_level_only() {
        let json = r#"{
  "name": "x",
  "dependencies": { "version": "9.9.9", "react": "^18" },
  "version": "0.4.0",
  "nested": [ { "version": "8.8.8" } ]
}"#;
        assert_eq!(json_top_level_version(json), Some("0.4.0".to_string()));
    }

    #[test]
    fn json_version_handles_escapes_and_missing_key() {
        assert_eq!(
            json_top_level_version(r#"{"desc": "a \"version\": \"1\"", "version" : "2.0.0"}"#),
            Some("2.0.0".to_string())
        );
        assert_eq!(json_top_level_version(r#"{"name": "x"}"#), None);
        assert_eq!(json_top_level_version(r#"{"version": 3}"#), None);
    }

    #[test]
    fn repo_manifests_are_consistent() {
        // Runs against the real repository so `cargo test` also guards drift.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives in the repo root");
        if let Err(msg) = check_repo(root) {
            panic!("{msg}");
        }
    }
}
