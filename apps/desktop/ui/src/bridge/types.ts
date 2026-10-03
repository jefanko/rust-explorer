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

export type OperationKind = "copy" | "move" | "rename" | "create_folder" | "recycle";

export type JobState =
  | "planned"
  | "queued"
  | "validating"
  | "running"
  | "succeeded"
  | "partial_failure"
  | "failed"
  | "cancel_requested"
  | "canceled"
  | "interrupted";

export interface OperationPlan {
  id: string;
  commit_token: string;
  kind: OperationKind;
  source_paths: string[];
  destination_path?: string | null;
  target_name?: string | null;
  items_count: number;
  expires_at: number;
}

export interface JobSummary {
  id: string;
  plan_id: string;
  kind: OperationKind;
  state: JobState;
  total_items: number;
  completed_items: number;
  failed_items: number;
  error_message?: string | null;
  created_at_epoch: number;
  updated_at_epoch: number;
}

export interface ClipboardPayload {
  paths: string[];
  is_cut: boolean;
}

