//! Indexing and Search Crate
//! SQLite FTS5 trigram filename search, crawler, and scoped reconciliation.

pub mod crawl;
pub mod db;
pub mod query;
pub mod reconcile;
pub mod service;

pub use crawl::{CrawlStats, MetadataCrawler};
pub use db::{IndexDb, IndexedRoot, RootState};
pub use query::{QueryEngine, SearchResponse, SearchResultItem};
pub use service::IndexService;
