use crate::db::{IndexDb, bytes_to_wide, path_prefix_range_bytes};
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_win::path::{path_to_wide, wide_to_path};
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
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
    pub root_count: usize,
    pub indexing: bool,
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
type PrefixRange = (Vec<u8>, Vec<u8>);

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
        self.execute_search_scoped(query_str, root_id, None, page, page_size)
    }

    /// Execute an indexed search, optionally restricted to a recursive folder subtree
    /// when `scope_folder` falls inside an indexed root.
    pub fn execute_search_scoped(
        &self,
        query_str: &str,
        root_id: Option<&str>,
        scope_folder: Option<&Path>,
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

        let primary_term = parsed
            .text_terms
            .first()
            .or_else(|| parsed.phrase_terms.first())
            .cloned()
            .unwrap_or_default();

        let resolved_scope: Option<(String, Option<PrefixRange>)> =
            scope_folder.and_then(|folder| self.resolve_folder_scope(folder, root_id));
        if let Some(folder) = scope_folder
            && resolved_scope.is_none()
        {
            if folder.is_dir() {
                return self
                    .execute_live_folder_search(folder, &parsed, query_str, page, page_size);
            } else {
                return Ok(SearchResponse {
                    query: query_str.to_string(),
                    results: Vec::new(),
                    total_matches: 0,
                    is_capped: false,
                    page,
                    page_size,
                    root_count: 0,
                    indexing: false,
                });
            }
        }
        let effective_root = resolved_scope
            .as_ref()
            .map(|(rid, _)| rid.as_str())
            .or(root_id);
        let prefix_range = resolved_scope
            .as_ref()
            .and_then(|(_, range)| range.as_ref());

        let (ranked, root_count, indexing) =
            self.db.read("QueryEngine::execute_search", |conn| {
                let (root_count, indexing) = conn
                    .query_row(
                        "SELECT count(*), COALESCE(SUM(state IN ('scanning', 'not_indexed')), 0) FROM roots",
                        [],
                        |row| {
                            let total: i64 = row.get(0)?;
                            let active: i64 = row.get(1)?;
                            Ok((total.max(0) as usize, active > 0))
                        },
                    )
                    .map_err(|e| {
                        ExplorerError::new(
                            ErrorCode::Internal,
                            format!("Failed to read root status: {e}"),
                            "QueryEngine::execute_search",
                        )
                    })?;

                let mut ranked: Vec<(u8, SearchResultItem)> = Vec::new();

                if !primary_term.is_empty() {
                    // Tier 1 (exact match) and Tier 2 (prefix match) use the B-tree index on
                    // `entries(name_norm)` directly so exact/prefix hits are never pushed past
                    // `MAX_SEARCH_RESULTS` by thousands of arbitrary FTS substring matches.
                    Self::collect_name_tier(
                        conn,
                        &parsed,
                        effective_root,
                        prefix_range,
                        &primary_term,
                        true,
                        MAX_SEARCH_RESULTS,
                        &mut ranked,
                    )?;
                    let rem = MAX_SEARCH_RESULTS.saturating_sub(ranked.len());
                    if rem > 0 {
                        Self::collect_name_tier(
                            conn,
                            &parsed,
                            effective_root,
                            prefix_range,
                            &primary_term,
                            false,
                            rem,
                            &mut ranked,
                        )?;
                    }
                }

                let rem = MAX_SEARCH_RESULTS.saturating_sub(ranked.len());
                if rem > 0 {
                    Self::collect_general_tier(
                        conn,
                        &parsed,
                        &fts_terms,
                        effective_root,
                        prefix_range,
                        &primary_term,
                        rem,
                        &mut ranked,
                    )?;
                }

                Ok((ranked, root_count, indexing))
            })?;

        let mut ranked = ranked;
        if let Some(scope) = scope_folder {
            let scope_prefix = explorer_win::path::strip_extended_prefix(scope);
            let scope_lc = scope_prefix.to_string_lossy().to_lowercase();
            let scope_trim = scope_lc.trim_end_matches(['\\', '/']);
            ranked.retain(|(_, item)| {
                let item_p = explorer_win::path::strip_extended_prefix(Path::new(&item.path));
                let item_lc = item_p.to_string_lossy().to_lowercase();
                item_lc == scope_trim
                    || item_lc
                        .strip_prefix(scope_trim)
                        .is_some_and(|rest| rest.starts_with('\\') || rest.starts_with('/'))
            });

            if ranked.is_empty() && scope.is_dir() {
                return self.execute_live_folder_search(scope, &parsed, query_str, page, page_size);
            }
        }
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.display_name.cmp(&b.1.display_name))
                .then_with(|| a.1.id.cmp(&b.1.id))
        });

        let total_matches = ranked.len();
        let is_capped = total_matches >= MAX_SEARCH_RESULTS;

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
            root_count,
            indexing,
        })
    }

    fn execute_live_folder_search(
        &self,
        scope_folder: &Path,
        parsed: &ParsedQuery,
        query_str: &str,
        page: usize,
        page_size: usize,
    ) -> Result<SearchResponse, ExplorerError> {
        let mut ranked: Vec<(u8, SearchResultItem)> = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(scope_folder.to_path_buf());

        let primary_term = parsed
            .text_terms
            .first()
            .or_else(|| parsed.phrase_terms.first())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let mut processed_dirs = 0usize;
        const MAX_DIRS: usize = 2000;
        let mut synthetic_id = 1i64;

        while let Some(current_dir) = queue.pop_front() {
            processed_dirs += 1;
            if processed_dirs > MAX_DIRS {
                break;
            }

            let entries = match fs::read_dir(&current_dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.filter_map(|e| e.ok()) {
                let full_path = entry.path();
                let file_name = entry.file_name();
                let display_name = file_name.to_string_lossy().to_string();
                let name_norm = display_name.to_lowercase();

                let metadata = entry.metadata().ok();
                let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let is_file = metadata.as_ref().map(|m| m.is_file()).unwrap_or(false);

                let mut score: u8 = 3;
                let mut matches = true;

                if !primary_term.is_empty() {
                    if name_norm == primary_term {
                        score = 1;
                    } else if name_norm.starts_with(&primary_term) {
                        score = 2;
                    } else if name_norm.contains(&primary_term) {
                        score = 3;
                    } else {
                        matches = false;
                    }
                }

                if matches {
                    for term in parsed.text_terms.iter().chain(parsed.phrase_terms.iter()) {
                        let term_lc = term.to_lowercase();
                        if !name_norm.contains(&term_lc) {
                            matches = false;
                            break;
                        }
                    }
                }

                if matches && let Some(ref ext_filter) = parsed.extension_filter {
                    let ext = full_path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    if ext != *ext_filter {
                        matches = false;
                    }
                }

                if matches && let Some(kind_filter) = parsed.kind_filter {
                    let entry_kind_code = if is_dir {
                        1
                    } else if is_file {
                        2
                    } else {
                        3
                    };
                    if entry_kind_code != kind_filter {
                        matches = false;
                    }
                }

                if matches {
                    let path_str = full_path.to_string_lossy().to_string();
                    let path_utf16 = path_to_wide(&full_path);
                    let ext = full_path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    let kind_str = if is_dir {
                        "directory"
                    } else if is_file {
                        "file"
                    } else {
                        "reparse_point"
                    };

                    let size_bytes = metadata
                        .as_ref()
                        .and_then(|m| if is_file { Some(m.len()) } else { None });
                    let modified_filetime = metadata.as_ref().and_then(|m| {
                        m.modified().ok().and_then(|st| {
                            st.duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .ok()
                                .map(|d| d.as_secs())
                        })
                    });

                    ranked.push((
                        score,
                        SearchResultItem {
                            id: synthetic_id,
                            root_id: "scoped_folder".to_string(),
                            path: path_str,
                            path_utf16,
                            display_name: display_name.clone(),
                            extension: ext,
                            kind: kind_str.to_string(),
                            size_bytes,
                            modified_filetime,
                        },
                    ));
                    synthetic_id += 1;

                    if ranked.len() >= MAX_SEARCH_RESULTS {
                        break;
                    }
                }

                if is_dir && !display_name.starts_with('.') {
                    queue.push_back(full_path);
                }
            }

            if ranked.len() >= MAX_SEARCH_RESULTS {
                break;
            }
        }

        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| {
                    a.1.display_name
                        .to_lowercase()
                        .cmp(&b.1.display_name.to_lowercase())
                })
                .then_with(|| a.1.path.cmp(&b.1.path))
        });

        let total_matches = ranked.len();
        let is_capped = total_matches >= MAX_SEARCH_RESULTS;

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
            root_count: 1,
            indexing: false,
        })
    }

    fn resolve_folder_scope(
        &self,
        scope_folder: &Path,
        explicit_root_id: Option<&str>,
    ) -> Option<(String, Option<PrefixRange>)> {
        let roots = self.db.list_roots().ok()?;
        let folder_stripped = explorer_win::path::strip_extended_prefix(scope_folder);
        let folder_lc = folder_stripped.to_string_lossy().to_lowercase();
        let folder_trim = folder_lc.trim_end_matches(['\\', '/']);

        let matched = roots.into_iter().find(|r| {
            if explicit_root_id.is_some_and(|id| id != r.id) {
                return false;
            }
            let root_stripped = explorer_win::path::strip_extended_prefix(&r.path);
            let root_lc = root_stripped.to_string_lossy().to_lowercase();
            let root_trim = root_lc.trim_end_matches(['\\', '/']);
            folder_trim == root_trim
                || folder_trim
                    .strip_prefix(root_trim)
                    .is_some_and(|rest| rest.starts_with('\\') || rest.starts_with('/'))
        })?;

        let stored_dir = self.db.resolve_stored_dir(&matched.id, scope_folder)?;
        if stored_dir == matched.path {
            Some((matched.id, None))
        } else {
            Some((matched.id, Some(path_prefix_range_bytes(&stored_dir))))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_name_tier(
        conn: &rusqlite::Connection,
        parsed: &ParsedQuery,
        root_id: Option<&str>,
        prefix_range: Option<&PrefixRange>,
        primary_term: &str,
        exact: bool,
        limit: usize,
        out: &mut Vec<(u8, SearchResultItem)>,
    ) -> Result<(), ExplorerError> {
        let mut sql = String::from(
            "SELECT e.id, e.root_id, e.path_utf16le, e.name_display, e.extension_norm, e.kind, e.size_bytes, e.modified_filetime
             FROM entries e INDEXED BY entries_name_norm WHERE ",
        );
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

        let score = if exact {
            sql.push_str("e.name_norm = ? ");
            params_vec.push(rusqlite::types::Value::Text(primary_term.to_string()));
            1
        } else {
            let upper = format!("{primary_term}\u{10FFFF}");
            sql.push_str("e.name_norm > ? AND e.name_norm < ? ");
            params_vec.push(rusqlite::types::Value::Text(primary_term.to_string()));
            params_vec.push(rusqlite::types::Value::Text(upper));
            2
        };

        Self::append_common_filters(&mut sql, &mut params_vec, parsed, root_id, prefix_range);
        for term in parsed
            .text_terms
            .iter()
            .chain(parsed.phrase_terms.iter())
            .filter(|t| t.as_str() != primary_term)
        {
            sql.push_str("AND instr(e.name_norm, ?) > 0 ");
            params_vec.push(rusqlite::types::Value::Text(term.clone()));
        }
        sql.push_str("LIMIT ?");
        params_vec.push(rusqlite::types::Value::Integer(limit as i64));

        Self::run_row_query(conn, &sql, params_vec, score, out)
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_general_tier(
        conn: &rusqlite::Connection,
        parsed: &ParsedQuery,
        fts_terms: &[String],
        root_id: Option<&str>,
        prefix_range: Option<&PrefixRange>,
        primary_term: &str,
        limit: usize,
        out: &mut Vec<(u8, SearchResultItem)>,
    ) -> Result<(), ExplorerError> {
        let mut sql = String::new();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

        if !fts_terms.is_empty() {
            sql.push_str(
                "SELECT e.id, e.root_id, e.path_utf16le, e.name_display, e.extension_norm, e.kind, e.size_bytes, e.modified_filetime
                 FROM filename_fts f
                 JOIN entries e ON e.id = f.rowid
                 WHERE filename_fts MATCH ? ",
            );
            params_vec.push(rusqlite::types::Value::Text(fts_terms.join(" AND ")));
        } else {
            sql.push_str(
                "SELECT e.id, e.root_id, e.path_utf16le, e.name_display, e.extension_norm, e.kind, e.size_bytes, e.modified_filetime
                 FROM entries e
                 WHERE 1=1 ",
            );
        }

        if !primary_term.is_empty() {
            let upper = format!("{primary_term}\u{10FFFF}");
            sql.push_str("AND (e.name_norm < ? OR e.name_norm >= ?) ");
            params_vec.push(rusqlite::types::Value::Text(primary_term.to_string()));
            params_vec.push(rusqlite::types::Value::Text(upper));
        }

        Self::append_common_filters(&mut sql, &mut params_vec, parsed, root_id, prefix_range);
        for short_term in parsed.text_terms.iter().filter(|t| t.chars().count() < 3) {
            sql.push_str("AND instr(e.name_norm, ?) > 0 ");
            params_vec.push(rusqlite::types::Value::Text(short_term.clone()));
        }
        sql.push_str("LIMIT ?");
        params_vec.push(rusqlite::types::Value::Integer(limit as i64));

        debug!("Executing index search SQL: {}", sql);
        Self::run_row_query(conn, &sql, params_vec, 3, out)
    }

    fn append_common_filters(
        sql: &mut String,
        params_vec: &mut Vec<rusqlite::types::Value>,
        parsed: &ParsedQuery,
        root_id: Option<&str>,
        prefix_range: Option<&PrefixRange>,
    ) {
        if let Some(rid) = root_id {
            sql.push_str("AND e.root_id = ? ");
            params_vec.push(rusqlite::types::Value::Text(rid.to_string()));
        }
        if let Some((lo, hi)) = prefix_range {
            sql.push_str("AND e.path_utf16le >= ? AND e.path_utf16le < ? ");
            params_vec.push(rusqlite::types::Value::Blob(lo.clone()));
            params_vec.push(rusqlite::types::Value::Blob(hi.clone()));
        }
        if let Some(kind) = parsed.kind_filter {
            sql.push_str("AND e.kind = ? ");
            params_vec.push(rusqlite::types::Value::Integer(kind as i64));
        }
        if let Some(ext) = &parsed.extension_filter {
            sql.push_str("AND e.extension_norm = ? ");
            params_vec.push(rusqlite::types::Value::Text(ext.clone()));
        }
    }

    fn run_row_query(
        conn: &rusqlite::Connection,
        sql: &str,
        params_vec: Vec<rusqlite::types::Value>,
        score: u8,
        out: &mut Vec<(u8, SearchResultItem)>,
    ) -> Result<(), ExplorerError> {
        let mut stmt = conn.prepare(sql).map_err(|e| {
            ExplorerError::new(
                ErrorCode::Internal,
                format!("Failed to prepare search query: {e}"),
                "QueryEngine::execute_search",
            )
        })?;

        let rows = stmt
            .query_map(params_from_iter(params_vec), |row| {
                let id: i64 = row.get(0)?;
                let root_id: String = row.get(1)?;
                let path_bytes: Vec<u8> = row.get(2)?;
                let display_name: String = row.get(3)?;
                let extension: String = row.get(4)?;
                let kind_int: i32 = row.get(5)?;
                let size_bytes: Option<i64> = row.get(6)?;
                let modified_filetime: Option<i64> = row.get(7)?;

                let path_buf = wide_to_path(&bytes_to_wide(&path_bytes));
                let kind = match kind_int {
                    1 => "directory",
                    2 => "reparse_point",
                    _ => "file",
                };

                Ok(SearchResultItem {
                    id,
                    root_id,
                    path: path_buf.to_string_lossy().to_string(),
                    path_utf16: path_buf.as_os_str().encode_wide().collect(),
                    display_name,
                    extension,
                    kind: kind.to_string(),
                    size_bytes: size_bytes.map(|s| s as u64),
                    modified_filetime: modified_filetime.map(|m| m as u64),
                })
            })
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Search query execution failed: {e}"),
                    "QueryEngine::execute_search",
                )
            })?;

        for item in rows.flatten() {
            out.push((score, item));
        }
        Ok(())
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

    #[test]
    fn exact_and_prefix_matches_survive_when_substrings_exceed_result_cap() {
        let db = Arc::new(IndexDb::open_in_memory().unwrap());
        let root = db.add_root(&PathBuf::from(r"C:\test")).unwrap();

        let mut entries: Vec<IndexEntryRecord> = (0..1100)
            .map(|i| {
                let name = format!("aa_{i:04}_readme_notes.txt");
                IndexEntryRecord {
                    root_id: root.id.clone(),
                    parent_id: None,
                    path: PathBuf::from(format!(r"C:\test\{name}")),
                    name_display: name.clone(),
                    name_norm: name,
                    extension_norm: "txt".to_string(),
                    kind: 0,
                    size_bytes: Some(10),
                    modified_filetime: Some(1),
                    attributes: 32,
                    seen_epoch: 1,
                }
            })
            .collect();
        for name in ["readme", "readme.md"] {
            entries.push(IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(format!(r"C:\test\{name}")),
                name_display: name.to_string(),
                name_norm: name.to_string(),
                extension_norm: String::new(),
                kind: 0,
                size_bytes: Some(10),
                modified_filetime: Some(1),
                attributes: 32,
                seen_epoch: 1,
            });
        }
        db.batch_upsert_entries(&entries).unwrap();

        let engine = QueryEngine::new(db);
        let res = engine.execute_search("readme", None, 1, 10).unwrap();
        assert!(res.is_capped);
        assert_eq!(res.results[0].display_name, "readme");
        assert_eq!(res.results[1].display_name, "readme.md");
    }

    #[test]
    fn scoped_folder_search_returns_recursive_descendants_case_insensitively() {
        let db = Arc::new(IndexDb::open_in_memory().unwrap());
        let root = db.add_root(&PathBuf::from(r"\\?\D:\")).unwrap();

        let entries = vec![
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"\\?\D:\Dev"),
                name_display: "Dev".to_string(),
                name_norm: "dev".to_string(),
                extension_norm: String::new(),
                kind: 1,
                size_bytes: None,
                modified_filetime: Some(1),
                attributes: 16,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"\\?\D:\Dev\ProjectA"),
                name_display: "ProjectA".to_string(),
                name_norm: "projecta".to_string(),
                extension_norm: String::new(),
                kind: 1,
                size_bytes: None,
                modified_filetime: Some(1),
                attributes: 16,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"\\?\D:\Dev\ProjectA\nested_target.rs"),
                name_display: "nested_target.rs".to_string(),
                name_norm: "nested_target.rs".to_string(),
                extension_norm: "rs".to_string(),
                kind: 0,
                size_bytes: Some(42),
                modified_filetime: Some(1),
                attributes: 32,
                seen_epoch: 1,
            },
            IndexEntryRecord {
                root_id: root.id.clone(),
                parent_id: None,
                path: PathBuf::from(r"\\?\D:\Other\outside_target.rs"),
                name_display: "outside_target.rs".to_string(),
                name_norm: "outside_target.rs".to_string(),
                extension_norm: "rs".to_string(),
                kind: 0,
                size_bytes: Some(42),
                modified_filetime: Some(1),
                attributes: 32,
                seen_epoch: 1,
            },
        ];
        db.batch_upsert_entries(&entries).unwrap();

        let engine = QueryEngine::new(db);
        // Scope using lowercase unprefixed path `d:\dev` while entries are stored under `\\?\D:\Dev`
        let scoped = engine
            .execute_search_scoped("target", None, Some(Path::new(r"d:\dev")), 1, 10)
            .unwrap();
        assert_eq!(scoped.total_matches, 1);
        assert_eq!(scoped.results[0].display_name, "nested_target.rs");

        // Outside any indexed root with non-existent directory -> 0 matches, never leaks other roots
        let fallback = engine
            .execute_search_scoped("target", None, Some(Path::new(r"C:\Unindexed")), 1, 10)
            .unwrap();
        assert_eq!(fallback.total_matches, 0);
    }

    #[test]
    fn scoped_folder_search_in_unindexed_folder_uses_live_search() {
        let temp = tempfile::tempdir().unwrap();
        let unindexed_dir = temp.path().join("OneDrive");
        std::fs::create_dir(&unindexed_dir).unwrap();
        std::fs::write(unindexed_dir.join("onedrive_doc.txt"), "hello").unwrap();

        let db = Arc::new(IndexDb::open_in_memory().unwrap());
        let engine = QueryEngine::new(db);
        let res = engine
            .execute_search_scoped("onedrive_doc", None, Some(&unindexed_dir), 1, 10)
            .unwrap();
        assert_eq!(res.total_matches, 1);
        assert_eq!(res.results[0].display_name, "onedrive_doc.txt");
        assert_eq!(res.results[0].kind, "file");
    }
}
