use crate::ids::{FolderToken, ItemToken, SessionId};
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
    /// Exact native name code units, without a terminator. UI labels are not path authority.
    pub native_name_utf16: Vec<u16>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortColumn {
    #[default]
    Name,
    Type,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryPage {
    pub folder_token: FolderToken,
    pub path_display: String,
    pub generation: u64,
    pub offset: usize,
    pub total_entries: usize,
    pub entries: Vec<FileEntry>,
    pub is_last_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownFolderItem {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveItem {
    pub name: String,
    pub path: String,
    pub drive_type: String,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapData {
    pub session_id: SessionId,
    pub known_folders: Vec<KnownFolderItem>,
    pub drives: Vec<DriveItem>,
    pub initial_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationResponse {
    pub folder_token: FolderToken,
    pub path_display: String,
    pub path_utf16: Vec<u16>,
    pub generation: u64,
    pub total_entries: usize,
}
