use explorer_index::{SearchResponse, SearchResultItem};
use std::path::PathBuf;

pub const RESULT_LIMIT: usize = 1000;
pub const DEBOUNCE_MS: u32 = 150;
pub const REFRESH_MS: u32 = 1500;

#[derive(Default)]
pub struct SearchView {
    pub active: bool,
    pub query: String,
    pub results: Vec<SearchResultItem>,
    pub total_matches: usize,
    pub is_capped: bool,
    pub pending: bool,
    pub indexing: bool,
    pub error: Option<String>,
    pub generation: u64,
    pub auto_indexed_path: Option<PathBuf>,
    pub auto_index_pending: bool,
    pub auto_index_attempts: u8,
}

impl SearchView {
    /// Starts a new query generation. Returns the generation to stamp on the
    /// request so late responses for older queries are ignored.
    pub fn begin(&mut self, query: impl Into<String>) -> u64 {
        self.active = true;
        self.query = query.into();
        self.generation = self.generation.wrapping_add(1);
        self.pending = true;
        self.error = None;
        self.results.clear();
        self.total_matches = 0;
        self.is_capped = false;
        self.generation
    }

    pub fn apply(&mut self, response: SearchResponse) {
        self.active = true;
        self.results = response.results;
        self.total_matches = response.total_matches;
        self.is_capped = response.is_capped;
        self.pending = false;
        self.error = None;
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.active = true;
        self.results.clear();
        self.total_matches = 0;
        self.is_capped = false;
        self.pending = false;
        self.error = Some(message.into());
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.query.clear();
        self.results.clear();
        self.total_matches = 0;
        self.is_capped = false;
        self.pending = false;
        self.indexing = false;
        self.error = None;
        self.auto_indexed_path = None;
        self.auto_index_pending = false;
        self.auto_index_attempts = 0;
    }

    /// Status-bar text: (search label, match/error/indexing text).
    pub fn status_parts(&self) -> (String, String) {
        let part1 = if self.query.is_empty() {
            "Search".to_string()
        } else {
            format!("Search: {}", self.query)
        };

        let mut part2 = if let Some(error) = &self.error {
            error.clone()
        } else if self.pending {
            "Searching indexed roots...".to_string()
        } else if self.is_capped {
            format!("{}+ matches (refine search)", self.total_matches)
        } else {
            format!(
                "{} match{}",
                self.total_matches,
                if self.total_matches == 1 { "" } else { "es" }
            )
        };

        if self.indexing && !self.pending && self.error.is_none() {
            part2.push_str(" (indexing...)");
        }

        (part1, part2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_response(query: &str, total: usize, capped: bool) -> SearchResponse {
        SearchResponse {
            query: query.to_string(),
            results: Vec::new(),
            total_matches: total,
            is_capped: capped,
            page: 1,
            page_size: RESULT_LIMIT,
        }
    }

    #[test]
    fn default_search_is_inactive() {
        let search = SearchView::default();
        assert!(!search.active);
        assert!(!search.pending);
        assert_eq!(search.status_parts().0, "Search");
    }

    #[test]
    fn begin_increments_generation_and_clears_previous_results() {
        let mut search = SearchView::default();
        let first = search.begin("first");
        assert_eq!(first, 1);
        assert!(search.active);
        assert!(search.pending);
        assert_eq!(search.query, "first");

        search.apply(empty_response("first", 4, false));
        assert!(!search.pending);
        assert_eq!(search.total_matches, 4);

        let second = search.begin("second");
        assert_eq!(second, 2);
        assert!(search.pending);
        assert!(search.results.is_empty());
        assert_eq!(search.total_matches, 0);
    }

    #[test]
    fn fail_keeps_query_and_clears_results() {
        let mut search = SearchView::default();
        search.begin("broken");
        search.fail("Use at least 3 characters for indexed search");

        assert!(search.active);
        assert!(!search.pending);
        assert_eq!(search.query, "broken");
        assert!(search.results.is_empty());
        assert_eq!(
            search.error.as_deref(),
            Some("Use at least 3 characters for indexed search")
        );
        assert!(search.status_parts().1.contains("3 characters"));
    }

    #[test]
    fn capped_status_and_indexing_suffix() {
        let mut search = SearchView::default();
        search.begin("wide");
        search.apply(empty_response("wide", RESULT_LIMIT, true));
        search.indexing = true;

        let (part1, part2) = search.status_parts();
        assert_eq!(part1, "Search: wide");
        assert!(part2.contains("1000+ matches"));
        assert!(part2.contains("indexing"));
    }

    #[test]
    fn clear_returns_to_inactive_and_resets_auto_index() {
        let mut search = SearchView::default();
        search.begin("query");
        search.auto_indexed_path = Some(PathBuf::from(r"C:\Temp"));
        search.auto_index_pending = true;
        search.auto_index_attempts = 2;

        search.clear();
        assert!(!search.active);
        assert!(search.query.is_empty());
        assert!(search.auto_indexed_path.is_none());
        assert!(!search.auto_index_pending);
        assert_eq!(search.auto_index_attempts, 0);
    }
}
