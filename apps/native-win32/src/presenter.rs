#![allow(dead_code)]

use explorer_domain::ids::FolderToken;
use explorer_domain::models::{
    DirectoryPage, EntryKind, FileEntry, NavigationResponse, SortColumn, SortDirection,
};
use std::collections::{HashMap, HashSet};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

pub const PAGE_SIZE: usize = 256;
pub const MAX_CACHED_PAGES: usize = 24;

/// Bounded cache of directory rows for an active view.
pub struct BoundedRowCache {
    pages: HashMap<usize, Vec<FileEntry>>,
    access_order: Vec<usize>,
    max_pages: usize,
}

impl BoundedRowCache {
    pub fn new(max_pages: usize) -> Self {
        Self {
            pages: HashMap::new(),
            access_order: Vec::new(),
            max_pages,
        }
    }

    pub fn insert_page(&mut self, offset: usize, entries: Vec<FileEntry>) {
        if self.pages.contains_key(&offset) {
            self.access_order.retain(|&o| o != offset);
        } else if self.pages.len() >= self.max_pages && !self.access_order.is_empty() {
            let oldest = self.access_order.remove(0);
            self.pages.remove(&oldest);
        }
        self.access_order.push(offset);
        self.pages.insert(offset, entries);
    }

    pub fn get_entry(&self, index: usize) -> Option<&FileEntry> {
        let page_offset = (index / PAGE_SIZE) * PAGE_SIZE;
        let in_page = index % PAGE_SIZE;
        self.pages.get(&page_offset).and_then(|p| p.get(in_page))
    }

    /// True when the page that would hold `index` is resident.
    pub fn contains(&self, index: usize) -> bool {
        self.pages.contains_key(&((index / PAGE_SIZE) * PAGE_SIZE))
    }

    /// True when the page starting at `offset` is resident.
    pub fn contains_page(&self, offset: usize) -> bool {
        self.pages.contains_key(&offset)
    }

    pub fn clear(&mut self) {
        self.pages.clear();
        self.access_order.clear();
    }
}

/// Represents the state of a single tab.
pub struct TabPresenter {
    pub tab_id: String,
    pub view_epoch: u64,
    pub current_path: PathBuf,
    /// Last path that was successfully enumerated; navigation failures roll back to it.
    pub applied_path: PathBuf,
    pub native_path_utf16: Vec<u16>,
    pub folder_token: Option<FolderToken>,
    pub generation: u64,
    pub total_items: usize,
    pub history_back: Vec<PathBuf>,
    pub history_forward: Vec<PathBuf>,
    pub cache: BoundedRowCache,
    pub selected_indices: HashSet<usize>,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub filter_query: String,
    pub filtered_indices: Option<Vec<usize>>,
    pub show_hidden: bool,
    pub pending_page_requests: HashSet<usize>,
}

impl TabPresenter {
    pub fn new(tab_id: impl Into<String>, initial_path: PathBuf) -> Self {
        let utf16: Vec<u16> = initial_path.as_os_str().encode_wide().collect();
        Self {
            tab_id: tab_id.into(),
            view_epoch: 1,
            applied_path: initial_path.clone(),
            current_path: initial_path,
            native_path_utf16: utf16,
            folder_token: None,
            generation: 0,
            total_items: 0,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            cache: BoundedRowCache::new(MAX_CACHED_PAGES),
            selected_indices: HashSet::new(),
            sort_column: SortColumn::Name,
            sort_direction: SortDirection::Ascending,
            filter_query: String::new(),
            filtered_indices: None,
            show_hidden: false,
            pending_page_requests: HashSet::new(),
        }
    }

    pub fn apply_navigation(&mut self, nav: NavigationResponse) {
        self.view_epoch += 1;
        self.current_path = PathBuf::from(std::ffi::OsString::from_wide(&nav.path_utf16));
        self.applied_path = self.current_path.clone();
        self.native_path_utf16 = nav.path_utf16;
        self.folder_token = Some(nav.folder_token);
        self.generation = nav.generation;
        self.total_items = nav.total_entries;
        self.cache.clear();
        self.selected_indices.clear();
        self.filter_query.clear();
        self.filtered_indices = None;
        self.pending_page_requests.clear();
    }

    pub fn apply_page(&mut self, page: DirectoryPage) {
        self.pending_page_requests.remove(&page.offset);
        self.cache.insert_page(page.offset, page.entries);
        // Absolute row indices stay valid when a page arrives; only an active
        // filter can gain new matches, so avoid the O(n) rebuild otherwise.
        if !self.filter_query.is_empty() {
            self.recompute_filter();
        }
    }

    pub fn navigate_to(&mut self, target: PathBuf) -> u64 {
        if target != self.current_path {
            self.history_back.push(self.current_path.clone());
            self.history_forward.clear();
            self.current_path = target;
        }
        self.view_epoch += 1;
        self.view_epoch
    }

    pub fn can_go_back(&self) -> bool {
        !self.history_back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.history_forward.is_empty()
    }

    pub fn go_back(&mut self) -> Option<PathBuf> {
        let prev = self.history_back.pop()?;
        self.history_forward.push(self.current_path.clone());
        self.current_path = prev.clone();
        self.view_epoch += 1;
        Some(prev)
    }

    pub fn go_forward(&mut self) -> Option<PathBuf> {
        let next = self.history_forward.pop()?;
        self.history_back.push(self.current_path.clone());
        self.current_path = next.clone();
        self.view_epoch += 1;
        Some(next)
    }

    pub fn go_up(&mut self) -> Option<PathBuf> {
        let parent = self.current_path.parent()?;
        if parent == self.current_path {
            return None;
        }
        let p = parent.to_path_buf();
        Some(p)
    }

    pub fn set_filter(&mut self, query: &str) {
        self.filter_query = query.trim().to_string();
        self.selected_indices.clear();
        self.recompute_filter();
    }

    /// Rebuilds the visible-row mapping for the in-folder filter.
    ///
    /// Only entries that are already resident can be matched; hidden/system rows are
    /// already excluded by the snapshot itself, so an empty query always means
    /// "identity mapping" and costs nothing.
    pub fn recompute_filter(&mut self) {
        if self.filter_query.is_empty() {
            self.filtered_indices = None;
            return;
        }

        let q = self.filter_query.to_lowercase();
        let mut matching = Vec::new();
        for i in 0..self.total_items {
            if let Some(entry) = self.cache.get_entry(i)
                && matches_filter_criteria(entry, &q)
            {
                matching.push(i);
            }
        }

        self.filtered_indices = Some(matching);
    }

    pub fn visible_item_count(&self) -> usize {
        if let Some(ref filtered) = self.filtered_indices {
            filtered.len()
        } else {
            self.total_items
        }
    }

    pub fn get_visible_entry(&self, visible_idx: usize) -> Option<&FileEntry> {
        if let Some(ref filtered) = self.filtered_indices {
            let actual_idx = *filtered.get(visible_idx)?;
            self.cache.get_entry(actual_idx)
        } else {
            self.cache.get_entry(visible_idx)
        }
    }

    pub fn get_actual_index(&self, visible_idx: usize) -> usize {
        if let Some(ref filtered) = self.filtered_indices {
            *filtered.get(visible_idx).unwrap_or(&visible_idx)
        } else {
            visible_idx
        }
    }
}

/// Matches a file entry against in-folder filter query (P09).
fn matches_filter_criteria(entry: &FileEntry, query: &str) -> bool {
    let name = entry.display_name.to_lowercase();

    // ext:xxx
    if let Some(ext_query) = query.strip_prefix("ext:") {
        return entry.extension.to_lowercase() == ext_query.trim();
    }

    // type:folder or type:dir or type:file
    if let Some(type_query) = query.strip_prefix("type:") {
        let t = type_query.trim();
        return match t {
            "folder" | "dir" | "directory" => matches!(entry.kind, EntryKind::Directory),
            "file" => matches!(entry.kind, EntryKind::File),
            _ => true,
        };
    }

    // *.ext or .ext
    if let Some(ext) = query.strip_prefix("*.") {
        return entry.extension.to_lowercase() == ext.trim();
    }
    if let Some(ext) = query.strip_prefix('.') {
        return entry.extension.to_lowercase() == ext.trim();
    }

    // Wildcard * / ?
    if query.contains('*') || query.contains('?') {
        return wildcard_match(&name, query);
    }

    // Substring match
    name.contains(query)
}

fn wildcard_match(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let mut ti = 0;
    let mut pi = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            ti += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_idx = Some(pi);
            match_idx = ti;
            pi += 1;
        } else if let Some(si) = star_idx {
            pi = si + 1;
            match_idx += 1;
            ti = match_idx;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }

    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use explorer_domain::ids::ItemToken;

    #[test]
    fn test_tab_presenter_history_back_forward() {
        let mut tab = TabPresenter::new("tab-1", PathBuf::from(r"C:\Windows"));
        tab.navigate_to(PathBuf::from(r"C:\Users"));
        tab.navigate_to(PathBuf::from(r"C:\Program Files"));

        assert_eq!(tab.go_back(), Some(PathBuf::from(r"C:\Users")));
        assert_eq!(tab.go_back(), Some(PathBuf::from(r"C:\Windows")));
        assert_eq!(tab.go_back(), None);

        assert_eq!(tab.go_forward(), Some(PathBuf::from(r"C:\Users")));
        assert_eq!(tab.go_forward(), Some(PathBuf::from(r"C:\Program Files")));
        assert_eq!(tab.go_forward(), None);
    }

    #[test]
    fn test_bounded_row_cache_lru_eviction() {
        let mut cache = BoundedRowCache::new(2);
        cache.insert_page(0, Vec::new());
        cache.insert_page(256, Vec::new());
        assert_eq!(cache.pages.len(), 2);

        cache.insert_page(512, Vec::new());
        assert_eq!(cache.pages.len(), 2);
        assert!(!cache.pages.contains_key(&0));
        assert!(cache.pages.contains_key(&256));
        assert!(cache.pages.contains_key(&512));
    }

    #[test]
    fn test_filter_matching() {
        let entry = FileEntry {
            token: ItemToken::new(),
            parent_token: None,
            display_name: "Cargo.toml".to_string(),
            native_name_utf16: Vec::new(),
            escaped_name_hint: None,
            extension: "toml".to_string(),
            kind: EntryKind::File,
            size_bytes: Some(1024),
            modified_filetime: None,
            attributes: 0,
            is_hidden: false,
            is_readonly: false,
            is_system: false,
        };

        assert!(matches_filter_criteria(&entry, "cargo"));
        assert!(matches_filter_criteria(&entry, "ext:toml"));
        assert!(!matches_filter_criteria(&entry, "ext:rs"));
        assert!(matches_filter_criteria(&entry, "*.toml"));
        assert!(matches_filter_criteria(&entry, "type:file"));
        assert!(!matches_filter_criteria(&entry, "type:folder"));
    }

    #[test]
    fn test_tab_presenter_go_up() {
        let mut tab = TabPresenter::new("tab-1", PathBuf::from(r"C:\Windows\System32"));
        assert_eq!(tab.go_up(), Some(PathBuf::from(r"C:\Windows")));

        let mut root_tab = TabPresenter::new("tab-root", PathBuf::from(r"C:\"));
        assert_eq!(root_tab.go_up(), None);
    }

    #[test]
    fn test_filter_visible_mapping() {
        let mut tab = TabPresenter::new("tab-1", PathBuf::from(r"C:\Test"));
        tab.total_items = 2;

        let make_entry = |name: &str, ext: &str| FileEntry {
            token: ItemToken::new(),
            parent_token: None,
            display_name: name.to_string(),
            native_name_utf16: Vec::new(),
            escaped_name_hint: None,
            extension: ext.to_string(),
            kind: EntryKind::File,
            size_bytes: Some(10),
            modified_filetime: None,
            attributes: 0,
            is_hidden: false,
            is_readonly: false,
            is_system: false,
        };

        tab.cache.insert_page(
            0,
            vec![
                make_entry("visible.txt", "txt"),
                make_entry("other.log", "log"),
            ],
        );
        tab.show_hidden = false;

        // No query: identity mapping, no allocation.
        tab.recompute_filter();
        assert_eq!(tab.visible_item_count(), 2);
        assert!(tab.filtered_indices.is_none());

        tab.set_filter("ext:txt");
        assert_eq!(tab.visible_item_count(), 1);
        assert_eq!(
            tab.get_visible_entry(0).unwrap().display_name,
            "visible.txt"
        );
        assert_eq!(tab.get_actual_index(0), 0);

        tab.set_filter("");
        assert_eq!(tab.visible_item_count(), 2);
        assert_eq!(tab.get_visible_entry(1).unwrap().display_name, "other.log");
    }
}
