use crate::db::{IndexDb, bytes_to_wide};
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_win::path::wide_to_path;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use tracing::debug;

pub const MAX_QUERY_LEN: usize = 256;
pub const MAX_QUERY_TERMS: usize = 16;
pub const MAX_SEARCH_RESULTS: usize = 1000;
pub const DEFAULT_PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub text_terms: Vec<String>,
    pub phrase_terms: Vec<String>,
    pub extension_filter: Option<String>,
    pub kind_filter: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub id: i64,
    pub root_id: String,
    pub path: String,
    pub path_utf16: Vec<u16>,
    pub display_name: String,
    pub extension: String,
    pub kind: String, // "file" | "directory" | "reparse_point"
    pub size_bytes: Option<u64>,
    pub modified_filetime: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResultItem>,
    pub total_matches: usize,
    pub is_capped: bool,
    pub page: usize,
    pub page_size: usize,
}

/// Parses a user search string according to Section 14.2 MVP query language rules.
pub fn parse_query(raw: &str) -> Result<ParsedQuery, ExplorerError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Search query cannot be empty",
            "parse_query",
        ));
    }

    if trimmed.chars().count() > MAX_QUERY_LEN {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            format!("Query exceeds maximum length of {MAX_QUERY_LEN} characters"),
            "parse_query",
        ));
    }

    let mut text_terms = Vec::new();
    let mut phrase_terms = Vec::new();
    let mut extension_filter = None;
    let mut kind_filter = None;

    let mut chars = trimmed.chars().peekable();
    let mut current_token = String::new();

    while let Some(&ch) = chars.peek() {
        if ch == '"' {
            chars.next(); // consume opening quote
            let mut phrase = String::new();
            let mut closed = false;
            for inner_ch in chars.by_ref() {
                if inner_ch == '"' {
                    closed = true;
                    break;
                }
                phrase.push(inner_ch);
            }
            if !closed {
                return Err(ExplorerError::new(
                    ErrorCode::InvalidName,
                    "Unmatched quote in search query; please close quotes",
                    "parse_query",
                ));
            }
            let phrase_norm = phrase.trim().to_lowercase();
            if !phrase_norm.is_empty() {
                phrase_terms.push(phrase_norm);
            }
        } else if ch.is_whitespace() {
            chars.next();
            if !current_token.is_empty() {
                process_token(
                    &current_token,
                    &mut text_terms,
                    &mut extension_filter,
                    &mut kind_filter,
                )?;
                current_token.clear();
            }
        } else {
            current_token.push(ch);
            chars.next();
        }
    }

    if !current_token.is_empty() {
        process_token(
            &current_token,
            &mut text_terms,
            &mut extension_filter,
            &mut kind_filter,
        )?;
    }

    let total_terms = text_terms.len() + phrase_terms.len();
    if total_terms > MAX_QUERY_TERMS {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            format!("Query exceeds maximum of {MAX_QUERY_TERMS} terms"),
            "parse_query",
        ));
    }

    // Check minimum character rule
    let has_3char_term = text_terms.iter().any(|t| t.chars().count() >= 3)
        || phrase_terms.iter().any(|p| p.chars().count() >= 3);
    let has_metadata_filter = extension_filter.is_some() || kind_filter.is_some();

    if !has_3char_term && !has_metadata_filter {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Use at least 3 characters for indexed search",
            "parse_query",
        ));
    }

    Ok(ParsedQuery {
        text_terms,
        phrase_terms,
        extension_filter,
        kind_filter,
    })
}

fn process_token(
    token: &str,
    text_terms: &mut Vec<String>,
    extension_filter: &mut Option<String>,
    kind_filter: &mut Option<i32>,
) -> Result<(), ExplorerError> {
    let lower = token.to_lowercase();
    if let Some(ext) = lower.strip_prefix("ext:") {
        let clean_ext = ext.trim().trim_start_matches('.');
        if clean_ext.is_empty() {
            return Err(ExplorerError::new(
                ErrorCode::InvalidName,
                "Extension filter cannot be empty (e.g. ext:pdf)",
                "parse_query",
            ));
        }
        *extension_filter = Some(clean_ext.to_string());
    } else if let Some(ext) = lower.strip_prefix("*.") {
        let clean_ext = ext.trim();
        if clean_ext != "*"
            && !clean_ext.is_empty()
            && !clean_ext.contains('*')
            && !clean_ext.contains('?')
        {
            *extension_filter = Some(clean_ext.to_string());
        }
    } else if lower.starts_with('.') && lower.len() > 1 {
        let clean_ext = lower.trim_start_matches('.');
        if !clean_ext.is_empty()
            && !clean_ext.contains('.')
            && !clean_ext.contains('/')
            && !clean_ext.contains('\\')
            && !clean_ext.contains('*')
            && !clean_ext.contains('?')
            && !clean_ext.contains(':')
        {
            *extension_filter = Some(clean_ext.to_string());
        } else {
            text_terms.push(lower);
        }
    } else if let Some(k) = lower.strip_prefix("type:") {
        match k.trim() {
            "folder" | "dir" | "directory" => *kind_filter = Some(1),
            "file" => *kind_filter = Some(0),
            other => {
                return Err(ExplorerError::new(
                    ErrorCode::InvalidName,
                    format!("Unknown type filter '{other}'; use type:file or type:folder"),
                    "parse_query",
                ));
            }
        }
    } else if lower.contains(':') {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            format!("Unknown search filter '{token}'"),
            "parse_query",
        ));
    } else if lower.starts_with('*') || lower.ends_with('*') {
        let clean_term = lower.trim_matches('*');
        if !clean_term.is_empty() && !clean_term.contains('*') && !clean_term.contains('?') {
            text_terms.push(clean_term.to_string());
        } else {
            text_terms.push(lower);
        }
    } else {
        text_terms.push(lower);
    }
    Ok(())
}

/// Search query engine executing parsed queries against the SQLite index database.
pub struct QueryEngine {
    db: Arc<IndexDb>,
}

impl QueryEngine {
    pub fn new(db: Arc<IndexDb>) -> Self {
        Self { db }
    }

    /// Execute an indexed search across all roots or a specific target root.
    pub fn execute_search(
        &self,
        query_str: &str,
        root_id: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<SearchResponse, ExplorerError> {
        let parsed = parse_query(query_str)?;
        let page_size = if page_size == 0 {
            DEFAULT_PAGE_SIZE
        } else {
            page_size
        };
        let page = if page == 0 { 1 } else { page };

        // Determine if we should use FTS match or metadata-only query
        let fts_terms: Vec<String> = parsed
            .phrase_terms
            .iter()
            .map(|p| format!("\"{}\"", p.replace('"', "\"\"")))
            .chain(
                parsed
                    .text_terms
                    .iter()
                    .filter(|t| t.chars().count() >= 3)
                    .map(|t| format!("\"{}\"", t.replace('"', "\"\""))),
            )
            .collect();

        let use_fts = !fts_terms.is_empty();

        let mut sql = String::new();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

        if use_fts {
            let fts_expr = fts_terms.join(" AND ");
            sql.push_str(
                "SELECT e.id, e.root_id, e.path_utf16le, e.name_display, e.name_norm, e.extension_norm, e.kind, e.size_bytes, e.modified_filetime
                 FROM filename_fts f
                 JOIN entries e ON e.id = f.rowid
                 WHERE filename_fts MATCH ? ",
            );
            params_vec.push(rusqlite::types::Value::Text(fts_expr));
        } else {
            sql.push_str(
                "SELECT e.id, e.root_id, e.path_utf16le, e.name_display, e.name_norm, e.extension_norm, e.kind, e.size_bytes, e.modified_filetime
                 FROM entries e
                 WHERE 1=1 ",
            );
        }

        if let Some(rid) = root_id {
            sql.push_str("AND e.root_id = ? ");
            params_vec.push(rusqlite::types::Value::Text(rid.to_string()));
        }

        if let Some(kind) = parsed.kind_filter {
            sql.push_str("AND e.kind = ? ");
            params_vec.push(rusqlite::types::Value::Integer(kind as i64));
        }

        if let Some(ref ext) = parsed.extension_filter {
            let dot_ext = format!(".{ext}");
            sql.push_str("AND (e.extension_norm = ? OR e.name_norm = ?) ");
            params_vec.push(rusqlite::types::Value::Text(ext.clone()));
            params_vec.push(rusqlite::types::Value::Text(dot_ext));
        }

        // Apply short terms (< 3 chars) as instr predicates
        for short_term in parsed.text_terms.iter().filter(|t| t.chars().count() < 3) {
            sql.push_str("AND instr(e.name_norm, ?) > 0 ");
            params_vec.push(rusqlite::types::Value::Text(short_term.clone()));
        }

        // Bounded fetch: max 1000 items
        sql.push_str("LIMIT 1000");

        debug!("Executing index search SQL: {}", sql);

        let conn = self.db.raw_conn();
        let guard = conn.lock().unwrap();
        let mut stmt = guard.prepare(&sql).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to prepare search query: {e}"),
                "QueryEngine::execute_search",
            )
        })?;

        let rows = stmt
            .query_map(params_from_iter(params_vec), |row| {
                let id: i64 = row.get(0)?;
                let root_id_col: String = row.get(1)?;
                let path_bytes: Vec<u8> = row.get(2)?;
                let name_display: String = row.get(3)?;
                let name_norm: String = row.get(4)?;
                let extension: String = row.get(5)?;
                let kind_int: i32 = row.get(6)?;
                let size_bytes: Option<i64> = row.get(7)?;
                let modified_filetime: Option<i64> = row.get(8)?;

                let path_buf = wide_to_path(&bytes_to_wide(&path_bytes));
                let kind_str = match kind_int {
                    1 => "directory",
                    2 => "reparse_point",
                    _ => "file",
                };

                Ok((
                    id,
                    root_id_col,
                    path_buf.to_string_lossy().to_string(),
                    path_buf.as_os_str().encode_wide().collect::<Vec<_>>(),
                    name_display,
                    name_norm,
                    extension,
                    kind_str.to_string(),
                    size_bytes.map(|s| s as u64),
                    modified_filetime.map(|m| m as u64),
                ))
            })
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Search query execution failed: {e}"),
                    "QueryEngine::execute_search",
                )
            })?;

        // Primary search term for ranking
        let primary_term = parsed
            .text_terms
            .first()
            .or_else(|| parsed.phrase_terms.first())
            .cloned()
            .unwrap_or_default();

        let mut ranked = Vec::new();
        for (id, rid, path, path_utf16, name, name_norm, ext, kind, size, mtime) in rows.flatten() {
            let score = if !primary_term.is_empty() && name_norm == primary_term {
                1 // Exact match
            } else if !primary_term.is_empty() && name_norm.starts_with(&primary_term) {
                2 // Prefix match
            } else {
                3 // Substring match
            };

            ranked.push((
                score,
                SearchResultItem {
                    id,
                    root_id: rid,
                    path,
                    path_utf16,
                    display_name: name,
                    extension: ext,
                    kind,
                    size_bytes: size,
                    modified_filetime: mtime,
                },
            ));
        }

        // Rank by score ascending, then display name, then id
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.display_name.cmp(&b.1.display_name))
                .then_with(|| a.1.id.cmp(&b.1.id))
        });

        let total_matches = ranked.len();
        let is_capped = total_matches >= MAX_SEARCH_RESULTS;

        // Keyset/offset paging
        let offset = (page.saturating_sub(1)) * page_size;
        let paged_results: Vec<SearchResultItem> = ranked
            .into_iter()
            .skip(offset)
            .take(page_size)
            .map(|(_, item)| item)
            .collect();

        Ok(SearchResponse {
            query: query_str.to_string(),
            results: paged_results,
            total_matches,
            is_capped,
            page,
            page_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::IndexEntryRecord;
    use std::path::PathBuf;

    #[test]
    fn test_parse_query_valid_and_invalid() {
        // Simple term
        let q1 = parse_query("invoice").unwrap();
        assert_eq!(q1.text_terms, vec!["invoice"]);

        // Phrase
        let q2 = parse_query("\"annual report\"").unwrap();
        assert_eq!(q2.phrase_terms, vec!["annual report"]);

        // Extension and type filters
        let q3 = parse_query("invoice ext:pdf type:file").unwrap();
        assert_eq!(q3.text_terms, vec!["invoice"]);
        assert_eq!(q3.extension_filter, Some("pdf".to_string()));
        assert_eq!(q3.kind_filter, Some(0));

        // Short term rejected without 3+ chars
        assert!(parse_query("in").is_err());

        // Short term allowed with metadata filter
        let q4 = parse_query("in ext:pdf").unwrap();
        assert_eq!(q4.text_terms, vec!["in"]);
        assert_eq!(q4.extension_filter, Some("pdf".to_string()));

        // Unmatched quote rejected
        assert!(parse_query("\"unclosed phrase").is_err());

        // Unknown filter rejected
        assert!(parse_query("invoice foo:bar").is_err());
    }

    #[test]
    fn test_execute_search_ranking_and_filters() {
        let db = Arc::new(IndexDb::open_in_memory().unwrap());
        let root = db.add_root(&PathBuf::from(r"C:\test")).unwrap();

        let entries = vec![
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"C:\test\invoice.pdf"),
                name_display: "invoice.pdf".to_string(),
                name_norm: "invoice.pdf".to_string(),
                extension_norm: "pdf".to_string(),
                kind: 0,
                size_bytes: Some(100),
                modified_filetime: Some(1),
                attributes: 32,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"C:\test\invoice_2026.docx"),
                name_display: "invoice_2026.docx".to_string(),
                name_norm: "invoice_2026.docx".to_string(),
                extension_norm: "docx".to_string(),
                kind: 0,
                size_bytes: Some(200),
                modified_filetime: Some(2),
                attributes: 32,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"C:\test\my_invoice.pdf"),
                name_display: "my_invoice.pdf".to_string(),
                name_norm: "my_invoice.pdf".to_string(),
                extension_norm: "pdf".to_string(),
                kind: 0,
                size_bytes: Some(300),
                modified_filetime: Some(3),
                attributes: 32,
                seen_epoch: 1,
            },
        ];

        db.batch_upsert_entries(&entries).unwrap();

        let engine = QueryEngine::new(db);

        // Search for "invoice"
        let res = engine.execute_search("invoice", None, 1, 10).unwrap();
        assert_eq!(res.total_matches, 3);
        // Prefix/exact first
        assert_eq!(res.results[0].display_name, "invoice.pdf");
        assert_eq!(res.results[1].display_name, "invoice_2026.docx");
        assert_eq!(res.results[2].display_name, "my_invoice.pdf");

        // Filter by ext:pdf
        let res_pdf = engine
            .execute_search("invoice ext:pdf", None, 1, 10)
            .unwrap();
        assert_eq!(res_pdf.total_matches, 2);
        assert_eq!(res_pdf.results[0].display_name, "invoice.pdf");
        assert_eq!(res_pdf.results[1].display_name, "my_invoice.pdf");
    }

    #[test]
    fn test_search_extensions_and_wildcards() {
        let db = Arc::new(IndexDb::open_in_memory().unwrap());
        let root = db.add_root(&PathBuf::from(r"C:\test")).unwrap();

        let entries = vec![
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"C:\test\rust-explorer.exe"),
                name_display: "rust-explorer.exe".to_string(),
                name_norm: "rust-explorer.exe".to_string(),
                extension_norm: "exe".to_string(),
                kind: 0,
                size_bytes: Some(100),
                modified_filetime: Some(1),
                attributes: 32,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"C:\test\readme.txt"),
                name_display: "readme.txt".to_string(),
                name_norm: "readme.txt".to_string(),
                extension_norm: "txt".to_string(),
                kind: 0,
                size_bytes: Some(200),
                modified_filetime: Some(2),
                attributes: 32,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"C:\test\main.c"),
                name_display: "main.c".to_string(),
                name_norm: "main.c".to_string(),
                extension_norm: "c".to_string(),
                kind: 0,
                size_bytes: Some(50),
                modified_filetime: Some(3),
                attributes: 32,
                seen_epoch: 1,
            },
        ];

        db.batch_upsert_entries(&entries).unwrap();
        let engine = QueryEngine::new(db);

        let res_dot = engine.execute_search(".exe", None, 1, 10).unwrap();
        assert_eq!(res_dot.total_matches, 1);
        assert_eq!(res_dot.results[0].display_name, "rust-explorer.exe");

        let res_plain = engine.execute_search("exe", None, 1, 10).unwrap();
        assert_eq!(res_plain.total_matches, 1);
        assert_eq!(res_plain.results[0].display_name, "rust-explorer.exe");

        let res_star = engine.execute_search("*.exe", None, 1, 10).unwrap();
        assert_eq!(res_star.total_matches, 1);
        assert_eq!(res_star.results[0].display_name, "rust-explorer.exe");

        let res_c = engine.execute_search(".c", None, 1, 10).unwrap();
        assert_eq!(res_c.total_matches, 1);
        assert_eq!(res_c.results[0].display_name, "main.c");

        let res_star_c = engine.execute_search("*.c", None, 1, 10).unwrap();
        assert_eq!(res_star_c.total_matches, 1);
        assert_eq!(res_star_c.results[0].display_name, "main.c");

        let res_combo = engine.execute_search("rust *.exe", None, 1, 10).unwrap();
        assert_eq!(res_combo.total_matches, 1);
        assert_eq!(res_combo.results[0].display_name, "rust-explorer.exe");

        let res_wild = engine.execute_search("*explorer*", None, 1, 10).unwrap();
        assert_eq!(res_wild.total_matches, 1);
        assert_eq!(res_wild.results[0].display_name, "rust-explorer.exe");
    }
}
