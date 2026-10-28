import type { FileEntry, SortColumn, SortDirection } from "../bridge/types";
import { getTabTitle } from "../lib/paths";

export interface TabLocation {
  pathDisplay: string;
  pathUtf16: number[];
}

export interface TabState {
  id: string;
  title: string;
  path: string;
  addressInput: string;
  folderToken: string;
  nativePathUtf16: number[];
  generation: number;
  entries: FileEntry[];
  totalEntries: number;
  loading: boolean;
  error: string | null;
  history: TabLocation[];
  historyIndex: number;
  sortColumn: SortColumn;
  sortDirection: SortDirection;
  selectedTokens: Set<string>;
  focusedIndex: number;
  anchorIndex: number;
  scrollTop: number;
  filterQuery: string;
}

export function createInitialTab(id = "tab_1", initialPath = ""): TabState {
  return {
    id,
    title: getTabTitle(initialPath),
    path: initialPath,
    addressInput: initialPath,
    folderToken: "",
    nativePathUtf16: [],
    generation: 0,
    entries: [],
    totalEntries: 0,
    loading: true,
    error: null,
    history: initialPath ? [{ pathDisplay: initialPath, pathUtf16: [] }] : [],
    historyIndex: initialPath ? 0 : -1,
    sortColumn: "name",
    sortDirection: "ascending",
    selectedTokens: new Set(),
    focusedIndex: -1,
    anchorIndex: -1,
    scrollTop: 0,
    filterQuery: "",
  };
}
