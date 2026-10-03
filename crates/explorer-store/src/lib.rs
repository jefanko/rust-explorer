//! Storage Crate
//! SQLite persistence for settings, favorites, and job journals.

pub mod journal;
pub mod migrations;
pub mod settings;

pub use settings::{AppSettings, SettingsStore};
