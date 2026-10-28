export type EntryKind = "file" | "directory" | "reparse_point";

export interface FileEntry {
  token: string;
  parent_token?: string | null;
  display_name: string;
  native_name_utf16: number[];
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
  path_utf16: number[];
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
  kind: OperationKind;
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
  canceled_items: number;
  skipped_items: number;
  item_outcomes: ItemOutcome[];
  error_message?: string | null;
  created_at_epoch: number;
  updated_at_epoch: number;
}

export interface ItemOutcome {
  item_display: string;
  requested_destination_display?: string | null;
  actual_destination_display?: string | null;
  status: "succeeded" | "failed" | "skipped" | "canceled";
  native_code?: number | null;
  error_message?: string | null;
}

export type WatchEventKind =
  | "create"
  | "modify"
  | "delete"
  | "rename"
  | "rescan"
  | "overflow";

export interface WatchChange {
  path_utf16: number[];
  kind: WatchEventKind;
}

export interface WatchNotification {
  dir_path_display: string;
  dir_path_utf16: number[];
  is_overflow: boolean;
  changes: WatchChange[];
}

export type RootState =
  | "not_indexed"
  | "scanning"
  | "ready"
  | "degraded"
  | "offline"
  | "needs_reconcile"
  | "error";

export interface IndexedRoot {
  id: string;
  path_utf16: number[];
  display_path: string;
  state: RootState;
  completed_epoch: number;
  reconciled_at?: string | null;
}

export interface SearchResultItem {
  id: number;
  root_id: string;
  path: string;
  path_utf16: number[];
  display_name: string;
  extension: string;
  kind: string;
  size_bytes?: number | null;
  modified_filetime?: number | null;
}

export interface SearchResponse {
  query: string;
  results: SearchResultItem[];
  total_matches: number;
  is_capped: boolean;
  page: number;
  page_size: number;
}

export type PreviewData =
  | {
      kind: "text";
      content: string;
      truncated: boolean;
      encoding: string;
      line_count: number;
    }
  | {
      kind: "image";
      mime: string;
      data_base64: string;
      byte_len: number;
    }
  | {
      kind: "folder";
      item_count?: number | null;
    }
  | {
      kind: "unsupported";
      reason: string;
    };
