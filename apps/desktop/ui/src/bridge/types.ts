export type EntryKind = "file" | "directory" | "reparse_point";

export interface FileEntry {
  token: string;
  parent_token?: string | null;
  display_name: string;
  escaped_name_hint?: string | null;
  extension: string;
  kind: EntryKind;
  size_bytes?: number | null;
  modified_filetime?: number | null;
  attributes: number;
  is_hidden: boolean;
  is_readonly: boolean;
  is_system: boolean;
}

export type SortColumn = "name" | "type" | "size" | "modified";
export type SortDirection = "ascending" | "descending";

export interface DirectoryPage {
  folder_token: string;
  path_display: string;
  generation: number;
  offset: number;
  total_entries: number;
  entries: FileEntry[];
  is_last_page: boolean;
}

export interface KnownFolderItem {
  id: string;
  name: string;
  path: string;
}

export interface DriveItem {
  name: string;
  path: string;
  drive_type: string;
  total_bytes?: number | null;
  free_bytes?: number | null;
}

export interface BootstrapData {
  session_id: string;
  known_folders: KnownFolderItem[];
  drives: DriveItem[];
  initial_path: string;
}

export interface NavigationResponse {
  folder_token: string;
  path_display: string;
  generation: number;
  total_entries: number;
}

export interface AppSettings {
  theme: "system" | "light" | "dark";
  show_hidden_files: boolean;
  restore_tabs: boolean;
  saved_tabs: string[];
  favorites: string[];
}
