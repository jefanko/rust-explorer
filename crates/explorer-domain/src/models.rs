use crate::ids::{FolderToken, ItemToken};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Directory,
    ReparsePoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub token: ItemToken,
    pub parent_token: Option<FolderToken>,
    pub display_name: String,
    pub escaped_name_hint: Option<String>,
    pub extension: String,
    pub kind: EntryKind,
    pub size_bytes: Option<u64>,
    pub modified_filetime: Option<u64>,
    pub attributes: u32,
    pub is_hidden: bool,
    pub is_readonly: bool,
    pub is_system: bool,
}
