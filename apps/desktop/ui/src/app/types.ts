import type { FileEntry, SearchResultItem } from "../bridge/types";

export type ThemeMode = "system" | "light" | "dark";

export interface ContextMenuState {
  x: number;
  y: number;
  entry?: FileEntry;
  searchResult?: SearchResultItem;
}

export interface ModalState {
  type: "create_folder" | "rename" | "recycle";
  title: string;
  value: string;
  folderToken: string;
  itemToken?: string;
  targetTokens?: string[];
  error?: string | null;
}
