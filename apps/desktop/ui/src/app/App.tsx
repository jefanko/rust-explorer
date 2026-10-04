import { useEffect, useRef, useState, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { client } from "../bridge/client";
import {
  DriveItem,
  AppSettings,
  FileEntry,
  JobSummary,
  KnownFolderItem,
  SortColumn,
  SortDirection,
  IndexedRoot,
  SearchResultItem,
} from "../bridge/types";

function formatBytes(bytes?: number | null): string {
  if (bytes === null || bytes === undefined) return "";
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatFiletime(filetime?: number | null): string {
  if (!filetime) return "";
  const unixMs = (filetime - 116444736000000000) / 10000;
  if (unixMs <= 0) return "";
  const date = new Date(unixMs);
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function getFileIcon(entry: { kind: string; extension: string }) {
  if (entry.kind === "directory") {
    return "📁";
  }
  const ext = entry.extension.toLowerCase();
  if (["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg"].includes(ext)) return "🖼️";
  if (["mp4", "mkv", "avi", "mov", "wmv"].includes(ext)) return "🎬";
  if (["mp3", "wav", "flac", "m4a", "ogg"].includes(ext)) return "🎵";
  if (["zip", "rar", "7z", "tar", "gz"].includes(ext)) return "📦";
  if (["exe", "msi", "bat", "cmd", "ps1"].includes(ext)) return "⚙️";
  if (["pdf"].includes(ext)) return "📕";
  if (["txt", "md", "rs", "ts", "tsx", "js", "json", "toml", "html", "css"].includes(ext)) return "📄";
  return "📄";
}

function getTabTitle(path: string): string {
  if (!path) return "New Tab";
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts.pop() || path || "PC";
}

export function matchFilterQuery(
  entry: { display_name: string; extension: string; kind: string },
  filterQuery: string
): boolean {
  const q = filterQuery.trim().toLowerCase();
  if (!q) return true;

  // Direct extension filter like *.exe or *.pdf
  if (q.startsWith("*.")) {
    const ext = q.slice(2).trim();
    if (ext && ext !== "*") {
      return (
        entry.extension.toLowerCase() === ext ||
        entry.display_name.toLowerCase().endsWith("." + ext)
      );
    }
  }

  // Direct dot-extension filter like .exe or .pdf
  if (
    q.startsWith(".") &&
    q.length > 1 &&
    !q.slice(1).includes(".") &&
    !q.includes(" ") &&
    !q.includes(":") &&
    !q.includes("/") &&
    !q.includes("\\")
  ) {
    const ext = q.slice(1);
    return (
      entry.extension.toLowerCase() === ext ||
      entry.display_name.toLowerCase().endsWith("." + ext) ||
      entry.display_name.toLowerCase().includes(q)
    );
  }

  // ext: filter
  if (q.startsWith("ext:")) {
    const ext = q.slice(4).trim().replace(/^\./, "");
    return entry.extension.toLowerCase() === ext;
  }

  // type: filter
  if (q.startsWith("type:")) {
    const type = q.slice(5).trim();
    if (type === "folder" || type === "dir" || type === "directory") {
      return entry.kind === "directory";
    }
    if (type === "file") {
      return entry.kind === "file";
    }
  }

  // Wildcard glob if contains * or ?
  if (q.includes("*") || q.includes("?")) {
    const regexPattern =
      "^" +
      q
        .replace(/[-/\\^$+.,{}[\]()]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".") +
      "$";
    try {
      const re = new RegExp(regexPattern, "i");
      return re.test(entry.display_name);
    } catch {
      // Fall back to substring match below
    }
  }

  return entry.display_name.toLowerCase().includes(q);
}

function sameWindowsPath(left: number[], right: number[]): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    const normalize = (unit: number) => (unit >= 65 && unit <= 90 ? unit + 32 : unit);
    if (normalize(left[index]) !== normalize(right[index])) return false;
  }
  return true;
}

interface TabState {
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

interface ContextMenuState {
  x: number;
  y: number;
  entry?: FileEntry;
  searchResult?: SearchResultItem;
}

interface ModalState {
  type: "create_folder" | "rename" | "recycle";
  title: string;
  value: string;
  folderToken: string;
  itemToken?: string;
  targetTokens?: string[];
  error?: string | null;
}

interface TabLocation {
  pathDisplay: string;
  pathUtf16: number[];
}

function createInitialTab(id = "tab_1", initialPath = ""): TabState {
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

export default function App() {
  const [knownFolders, setKnownFolders] = useState<KnownFolderItem[]>([]);
  const [drives, setDrives] = useState<DriveItem[]>([]);
  const [favorites, setFavorites] = useState<string[]>([]);
  const [theme, setTheme] = useState<"system" | "light" | "dark">("system");
  const [showHiddenFiles, setShowHiddenFiles] = useState<boolean>(false);
  const settingsWriteQueue = useRef<Promise<void>>(Promise.resolve());

  // Multi-tab state
  const [tabs, setTabs] = useState<TabState[]>([createInitialTab()]);
  const [activeTabIndex, setActiveTabIndex] = useState<number>(0);

  // Context menu & Modal state
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [modal, setModal] = useState<ModalState | null>(null);

  // Jobs state
  const [jobs, setJobs] = useState<JobSummary[]>([]);
  const [showJobsDrawer, setShowJobsDrawer] = useState<boolean>(false);

  // Search state
  const [searchScope, setSearchScope] = useState<"folder" | "indexed">("folder");
  const [indexedSearchQuery, setIndexedSearchQuery] = useState<string>("");
  const [searchResults, setSearchResults] = useState<SearchResultItem[]>([]);
  const [searchTotalMatches, setSearchTotalMatches] = useState<number>(0);
  const [searchIsCapped, setSearchIsCapped] = useState<boolean>(false);
  const [searchLoading, setSearchLoading] = useState<boolean>(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [selectedSearchIndex, setSelectedSearchIndex] = useState<number>(-1);

  // Indexed roots state
  const [indexedRoots, setIndexedRoots] = useState<IndexedRoot[]>([]);

  // Refs for element focus and callbacks
  const addressInputRef = useRef<HTMLInputElement>(null);
  const filterInputRef = useRef<HTMLInputElement>(null);
  const modalInputRef = useRef<HTMLInputElement>(null);
  const parentRef = useRef<HTMLDivElement>(null);

  // State refs to ensure listeners always see current values
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  const navigationRequests = useRef(new Map<string, number>());
  const directoryRequests = useRef(new Map<string, number>());
  const activeTabIndexRef = useRef(activeTabIndex);
  activeTabIndexRef.current = activeTabIndex;

  const activeTab = tabs[activeTabIndex] || tabs[0];
  const activeTabRef = useRef(activeTab);
  activeTabRef.current = activeTab;

  function nextRequest(requests: Map<string, number>, tabId: string): number {
    const requestId = (requests.get(tabId) || 0) + 1;
    requests.set(tabId, requestId);
    return requestId;
  }

  function persistSettings(
    update:
      | Partial<AppSettings>
      | ((current: AppSettings) => Partial<AppSettings> | null)
  ) {
    const save = settingsWriteQueue.current.catch(() => undefined).then(async () => {
      const current = await client.loadSettings();
      const patch = typeof update === "function" ? update(current) : update;
      if (patch) await client.saveSettings({ ...current, ...patch });
    });
    settingsWriteQueue.current = save;
    return save;
  }

  // Filtered entries for active tab
  const displayedEntries = useMemo(() => {
    const visibleEntries = showHiddenFiles
      ? activeTab.entries
      : activeTab.entries.filter((entry) => !entry.is_hidden);
    if (!activeTab.filterQuery.trim()) return visibleEntries;
    return visibleEntries.filter((e) => matchFilterQuery(e, activeTab.filterQuery));
  }, [activeTab.entries, activeTab.filterQuery, showHiddenFiles]);

  const visibleEntryCount = useMemo(
    () =>
      showHiddenFiles
        ? activeTab.entries.length
        : activeTab.entries.filter((entry) => !entry.is_hidden).length,
    [activeTab.entries, showHiddenFiles]
  );

  const rowVirtualizer = useVirtualizer({
    count: displayedEntries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 20,
  });

  const isSearchActive = searchScope === "indexed" && Boolean(indexedSearchQuery.trim());

  const searchVirtualizer = useVirtualizer({
    count: searchResults.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 20,
  });

  // Apply theme to document
  function applyTheme(t: "system" | "light" | "dark") {
    if (t === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", t);
    }
  }

  function toggleTheme() {
    const nextTheme: "system" | "light" | "dark" =
      theme === "system" ? "light" : theme === "light" ? "dark" : "system";
    setTheme(nextTheme);
    applyTheme(nextTheme);
    void persistSettings({ theme: nextTheme });
  }

  function toggleHiddenFiles() {
    const next = !showHiddenFiles;
    setShowHiddenFiles(next);
    updateActiveTab({
      selectedTokens: new Set(),
      focusedIndex: -1,
      anchorIndex: -1,
    });
    void persistSettings({ show_hidden_files: next });
  }

  // Update a tab by ID
  function updateTab(tabId: string, updater: Partial<TabState> | ((prev: TabState) => TabState)) {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => {
        if (tab.id !== tabId) return tab;
        if (typeof updater === "function") {
          return updater(tab);
        }
        return { ...tab, ...updater };
      })
    );
  }

  // Update active tab
  function updateActiveTab(updater: Partial<TabState> | ((prev: TabState) => TabState)) {
    const currentId = activeTabRef.current.id;
    updateTab(currentId, updater);
  }

  // Load directory items
  async function loadDirectory(
    folderToken: string,
    generation: number,
    tabId: string,
    col = activeTabRef.current.sortColumn,
    dir = activeTabRef.current.sortDirection
  ) {
    const requestId = nextRequest(directoryRequests.current, tabId);
    updateTab(tabId, {
      loading: true,
      error: null,
      entries: [],
      totalEntries: 0,
      selectedTokens: new Set(),
      focusedIndex: -1,
      anchorIndex: -1,
    });
    try {
      const pageSize = 256;
      let offset = 0;
      const allEntries: FileEntry[] = [];
      let publishedCount = 0;
      let page: Awaited<ReturnType<typeof client.listPage>>;
      do {
        page = await client.listPage(folderToken, generation, offset, pageSize, col, dir);
        if (directoryRequests.current.get(tabId) !== requestId) return;
        allEntries.push(...page.entries);
        offset += page.entries.length;
        if (allEntries.length - publishedCount >= 1024 || page.is_last_page) {
          const visibleEntries = allEntries.slice();
          updateTab(tabId, {
            entries: visibleEntries,
            totalEntries: page.total_entries,
          });
          publishedCount = allEntries.length;
        }
        if (page.is_last_page || page.entries.length === 0) break;
      } while (allEntries.length < page.total_entries);
      if (directoryRequests.current.get(tabId) !== requestId) return;
      if (!page.is_last_page && allEntries.length < page.total_entries) {
        throw new Error("Directory page sequence ended before all entries were loaded");
      }
      updateTab(tabId, {
        entries: allEntries,
        totalEntries: page.total_entries,
        selectedTokens: new Set(),
        focusedIndex: -1,
        anchorIndex: -1,
        loading: false,
      });
    } catch (err: any) {
      if (directoryRequests.current.get(tabId) !== requestId) return;
      updateTab(tabId, {
        error: err?.user_message || "Failed to load directory items",
        loading: false,
      });
    }
  }

  // Navigate to path
  async function navigateToPath(path: string | number[], pushHistory = true, tabId?: string) {
    if (typeof path === "string" && !path.trim()) return;
    const targetTabId = tabId || activeTabRef.current.id;
    const requestId = nextRequest(navigationRequests.current, targetTabId);
    nextRequest(directoryRequests.current, targetTabId);
    updateTab(targetTabId, { loading: true, error: null });

    try {
      const nav = Array.isArray(path)
        ? await client.navigateNativePath(path)
        : await client.navigate(path);
      if (navigationRequests.current.get(targetTabId) !== requestId) return;
      updateTab(targetTabId, (prev) => {
        const nextIndex = pushHistory ? prev.historyIndex + 1 : prev.historyIndex;
        const newHist = pushHistory
          ? [
              ...prev.history.slice(0, nextIndex),
              { pathDisplay: nav.path_display, pathUtf16: nav.path_utf16 },
            ]
          : prev.history;

        return {
          ...prev,
          path: nav.path_display,
          addressInput: nav.path_display,
          folderToken: nav.folder_token,
          nativePathUtf16: nav.path_utf16,
          generation: nav.generation,
          title: getTabTitle(nav.path_display),
          history: newHist,
          historyIndex: nextIndex,
          filterQuery: "",
        };
      });

      // Update watch subscriptions
      try {
        await client.unwatchAll(targetTabId);
        if (navigationRequests.current.get(targetTabId) === requestId) {
          await client.watchFolder(nav.folder_token, targetTabId);
        }
      } catch {
        // Directory browsing remains available if watcher registration is degraded.
      }
      if (navigationRequests.current.get(targetTabId) !== requestId) return;

      await loadDirectory(nav.folder_token, nav.generation, targetTabId);
    } catch (err: any) {
      if (navigationRequests.current.get(targetTabId) !== requestId) return;
      updateTab(targetTabId, {
        error:
          err?.user_message ||
          `Cannot access ${typeof path === "string" ? path : "selected native path"}`,
        loading: false,
      });
    }
  }

  // Load jobs list from backend
  async function refreshJobs() {
    try {
      const list = await client.listJobs(30);
      setJobs(list);
    } catch {
      // Ignore
    }
  }

  // Indexed roots operations
  async function loadIndexedRoots() {
    try {
      const roots = await client.listIndexedRoots();
      setIndexedRoots(roots);
    } catch (err) {
      console.error("Failed to load indexed roots:", err);
    }
  }

  async function handleIndexCurrentFolder() {
    if (!activeTab.folderToken) return;
    try {
      await client.addIndexedRoot(activeTab.folderToken);
      await loadIndexedRoots();
    } catch (err: any) {
      alert(err?.user_message || err?.message || "Failed to index current folder");
    }
  }

  async function handleSearchSubfolders(overrideQuery?: string) {
    const q = (overrideQuery ?? (searchScope === "folder" ? activeTab.filterQuery : indexedSearchQuery)).trim();
    if (!q) return;

    // Check if current folder or a parent folder is already covered by an indexed root
    const normalizedActivePath = activeTab.path.toLowerCase().replace(/[\\/]+$/, "");
    const matchingRoot = indexedRoots.find((r) => {
      const rPath = r.display_path.toLowerCase().replace(/[\\/]+$/, "");
      return normalizedActivePath === rPath || normalizedActivePath.startsWith(rPath + "\\");
    });

    if (!matchingRoot && activeTab.folderToken && indexedRoots.length < 8) {
      try {
        await client.addIndexedRoot(activeTab.folderToken);
        await loadIndexedRoots();
      } catch (err: any) {
        console.warn("Could not auto-index root:", err);
      }
    }

    setSearchScope("indexed");
    setIndexedSearchQuery(q);
    setTimeout(() => {
      filterInputRef.current?.focus();
    }, 50);
  }

  async function handleRemoveRoot(rootId: string) {
    try {
      await client.removeIndexedRoot(rootId);
      await loadIndexedRoots();
    } catch (err: any) {
      console.error("Failed to remove indexed root:", err);
    }
  }

  async function handleRecrawlRoot(rootId: string) {
    try {
      await client.recrawlIndexedRoot(rootId);
      await loadIndexedRoots();
    } catch (err: any) {
      console.error("Failed to recrawl indexed root:", err);
    }
  }

  async function handleSearchResultDoubleClick(item: SearchResultItem) {
    if (item.kind === "directory") {
      navigateToPath(item.path_utf16);
    } else {
      try {
        const nav = await client.openPath(item.path_utf16);
        if (nav) {
          navigateToPath(item.path_utf16);
        }
      } catch (err: any) {
        alert(err?.user_message || err?.message || `Failed to open ${item.display_name}`);
      }
    }
  }

  function handleSearchResultContextMenu(item: SearchResultItem, e: React.MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      searchResult: item,
    });
  }

  const isAnyRootScanning = indexedRoots.some((r) => r.state === "scanning");

  // Poll for status updates if any indexed root is scanning
  useEffect(() => {
    if (!isAnyRootScanning) return;
    const interval = setInterval(loadIndexedRoots, 1200);
    return () => clearInterval(interval);
  }, [isAnyRootScanning]);

  // Debounced indexed search with query validation and cancellation
  useEffect(() => {
    if (searchScope !== "indexed") return;
    const q = indexedSearchQuery.trim();
    if (!q) {
      setSearchResults([]);
      setSearchTotalMatches(0);
      setSearchIsCapped(false);
      setSearchLoading(false);
      setSearchError(null);
      setSelectedSearchIndex(-1);
      return;
    }

    const hasMetadataFilter = /(?:ext:|\*\.|\.)[a-z0-9_-]+|type:(?:folder|file)/i.test(q);
    const textWithoutFilters = q.replace(/(?:ext:|\*\.|\.)[a-z0-9_-]+|type:[^\s]+/gi, "").trim();
    if (!hasMetadataFilter && textWithoutFilters.length < 3) {
      setSearchResults([]);
      setSearchTotalMatches(0);
      setSearchIsCapped(false);
      setSearchLoading(false);
      setSearchError("Use at least 3 characters for indexed search");
      setSelectedSearchIndex(-1);
      return;
    }

    setSearchLoading(true);
    setSearchError(null);

    let canceled = false;
    const timer = setTimeout(async () => {
      try {
        const resp = await client.searchIndexed(q, null, 1, 100);
        if (!canceled) {
          setSearchResults(resp.results);
          setSearchTotalMatches(resp.total_matches);
          setSearchIsCapped(resp.is_capped);
          setSearchLoading(false);
          setSelectedSearchIndex(-1);
        }
      } catch (err: any) {
        if (!canceled) {
          setSearchError(err?.user_message || err?.message || "Search failed");
          setSearchLoading(false);
        }
      }
    }, 150);

    return () => {
      canceled = true;
      clearTimeout(timer);
    };
  }, [searchScope, indexedSearchQuery, indexedRoots]);

  // Bootstrap app and load settings
  useEffect(() => {
    let mounted = true;

    Promise.all([client.bootstrap(), client.loadSettings(), client.listJobs(20), client.listIndexedRoots()])
      .then(([bootData, settings, jobList, rootsList]) => {
        if (!mounted) return;
        setKnownFolders(bootData.known_folders);
        setDrives(bootData.drives);
        setFavorites(settings.favorites || []);
        setTheme(settings.theme || "system");
        setShowHiddenFiles(Boolean(settings.show_hidden_files));
        setJobs(jobList || []);
        setIndexedRoots(rootsList || []);
        applyTheme(settings.theme || "system");

        if (settings.restore_tabs && settings.saved_tabs && settings.saved_tabs.length > 0) {
          const restoredTabs: TabState[] = settings.saved_tabs.map((path, idx) =>
            createInitialTab(`tab_${idx + 1}`, path)
          );
          setTabs(restoredTabs);
          setActiveTabIndex(0);
          restoredTabs.forEach((tab) => {
            navigateToPath(tab.path, true, tab.id);
          });
        } else {
          navigateToPath(bootData.initial_path, true, tabs[0].id);
        }
      })
      .catch((err) => {
        if (!mounted) return;
        updateActiveTab({
          error: typeof err === "string" ? err : err.user_message || "Failed to initialize",
          loading: false,
        });
      });

    return () => {
      mounted = false;
    };
  }, []);

  // Save tabs for restoration when tabs or active tab path changes
  useEffect(() => {
    const paths = tabs.map((t) => t.path).filter(Boolean);
    if (paths.length > 0) {
      void persistSettings((settings) =>
        settings.restore_tabs ? { saved_tabs: paths } : null
      );
    }
  }, [tabs]);

  // Listen for live filesystem notifications and reconcile affected tab views
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    client
      .onWatchNotification((notif) => {
        if (notif.is_overflow && notif.dir_path_utf16.length === 0) {
          for (const tab of tabsRef.current) {
            if (tab.folderToken) refresh(tab.id);
          }
          return;
        }
        for (const tab of tabsRef.current) {
          if (sameWindowsPath(tab.nativePathUtf16, notif.dir_path_utf16)) {
            refresh(tab.id);
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Tab switching
  function switchTab(newIndex: number) {
    if (newIndex === activeTabIndex || newIndex < 0 || newIndex >= tabs.length) return;
    if (parentRef.current) {
      updateActiveTab({ scrollTop: parentRef.current.scrollTop });
    }
    setActiveTabIndex(newIndex);
    const targetScroll = tabs[newIndex].scrollTop || 0;
    requestAnimationFrame(() => {
      if (parentRef.current) {
        parentRef.current.scrollTop = targetScroll;
      }
    });
  }

  function createNewTab(path?: string) {
    const newId = `tab_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
    const initial = path || activeTab.path || "C:\\";
    const newTab = createInitialTab(newId, initial);
    const newIndex = tabs.length;
    setTabs((prev) => [...prev, newTab]);
    setActiveTabIndex(newIndex);
    navigateToPath(initial, true, newId);
  }

  function closeTab(indexToClose: number, e?: React.MouseEvent) {
    if (e) e.stopPropagation();
    const closedTab = tabsRef.current[indexToClose];
    if (closedTab) {
      client.unwatchAll(closedTab.id).catch(() => {});
    }

    if (tabs.length <= 1) {
      const defaultPath = knownFolders[0]?.path || drives[0]?.path || "C:\\";
      navigateToPath(defaultPath, true, tabs[0].id);
      return;
    }

    setTabs((prevTabs) => {
      const nextTabs = prevTabs.filter((_, idx) => idx !== indexToClose);
      let nextActive = activeTabIndexRef.current;
      if (activeTabIndexRef.current === indexToClose) {
        nextActive = Math.max(0, indexToClose - 1);
      } else if (activeTabIndexRef.current > indexToClose) {
        nextActive = activeTabIndexRef.current - 1;
      }
      setActiveTabIndex(nextActive);
      return nextTabs;
    });
  }

  function cycleTab(direction: number) {
    setActiveTabIndex((prev) => {
      const len = tabsRef.current.length;
      if (len <= 1) return prev;
      return (prev + direction + len) % len;
    });
  }

  // Navigation history
  function goBack() {
    const cur = activeTabRef.current;
    if (cur.historyIndex > 0) {
      const prevIndex = cur.historyIndex - 1;
      updateActiveTab({ historyIndex: prevIndex });
      const target = cur.history[prevIndex];
      navigateToPath(target.pathUtf16.length ? target.pathUtf16 : target.pathDisplay, false);
    }
  }

  function goForward() {
    const cur = activeTabRef.current;
    if (cur.historyIndex < cur.history.length - 1) {
      const nextIndex = cur.historyIndex + 1;
      updateActiveTab({ historyIndex: nextIndex });
      const target = cur.history[nextIndex];
      navigateToPath(target.pathUtf16.length ? target.pathUtf16 : target.pathDisplay, false);
    }
  }

  function goUp() {
    const cur = activeTabRef.current;
    const units = cur.nativePathUtf16;
    if (units.length === 0) return;
    const separator = Math.max(units.lastIndexOf(92), units.lastIndexOf(47));
    if (separator < 0) return;
    const isDriveRootSeparator = separator === 2 && units[1] === 58;
    const parent = units.slice(0, separator + (isDriveRootSeparator ? 1 : 0));
    if (parent.length > 0 && parent.length < units.length) navigateToPath(parent);
  }

  function refresh(targetTabId?: string | React.MouseEvent) {
    const cur =
      typeof targetTabId === "string"
        ? tabsRef.current.find((t) => t.id === targetTabId) || activeTabRef.current
        : activeTabRef.current;
    if (!cur.folderToken) return;
    const requestId = nextRequest(navigationRequests.current, cur.id);
    nextRequest(directoryRequests.current, cur.id);
    updateTab(cur.id, { loading: true });
    client
      .refresh(cur.folderToken)
      .then((nav) => {
        if (navigationRequests.current.get(cur.id) !== requestId) return;
        updateTab(cur.id, {
          folderToken: nav.folder_token,
          nativePathUtf16: nav.path_utf16,
          generation: nav.generation,
        });
        return loadDirectory(nav.folder_token, nav.generation, cur.id);
      })
      .catch((err: any) => {
        if (navigationRequests.current.get(cur.id) !== requestId) return;
        updateTab(cur.id, {
          error: err?.user_message || "Failed to refresh directory",
          loading: false,
        });
      });
  }

  function handleSort(col: SortColumn) {
    const cur = activeTabRef.current;
    let nextDir: SortDirection = "ascending";
    if (cur.sortColumn === col) {
      nextDir = cur.sortDirection === "ascending" ? "descending" : "ascending";
    }
    updateActiveTab({ sortColumn: col, sortDirection: nextDir });
    if (cur.folderToken && cur.generation) {
      loadDirectory(cur.folderToken, cur.generation, cur.id, col, nextDir);
    }
  }

  async function handleItemDoubleClick(entry: FileEntry) {
    const cur = activeTabRef.current;
    if (!cur.folderToken) return;
    const requestId = nextRequest(navigationRequests.current, cur.id);
    nextRequest(directoryRequests.current, cur.id);
    try {
      const nav = await client.openItem(cur.folderToken, entry.token);
      if (navigationRequests.current.get(cur.id) !== requestId) return;
      if (nav) {
        updateTab(cur.id, (prev) => {
          const nextIndex = prev.historyIndex + 1;
          const newHist = [
            ...prev.history.slice(0, nextIndex),
            { pathDisplay: nav.path_display, pathUtf16: nav.path_utf16 },
          ];
          return {
            ...prev,
            path: nav.path_display,
            addressInput: nav.path_display,
            folderToken: nav.folder_token,
            nativePathUtf16: nav.path_utf16,
            generation: nav.generation,
            title: getTabTitle(nav.path_display),
            history: newHist,
            historyIndex: nextIndex,
          filterQuery: "",
        };
      });
      try {
        await client.unwatchAll(cur.id);
        if (navigationRequests.current.get(cur.id) === requestId) {
          await client.watchFolder(nav.folder_token, cur.id);
        }
      } catch {
        // Keep navigation usable if watch registration is temporarily unavailable.
      }
      if (navigationRequests.current.get(cur.id) !== requestId) return;
      await loadDirectory(nav.folder_token, nav.generation, cur.id);
      }
    } catch (err: any) {
      if (navigationRequests.current.get(cur.id) !== requestId) return;
      updateTab(cur.id, {
        error: err?.user_message || "Failed to open item",
      });
    }
  }

  // Row selection handlers
  function handleRowClick(entry: FileEntry, index: number, e: React.MouseEvent) {
    const cur = activeTabRef.current;
    if (e.ctrlKey) {
      const next = new Set(cur.selectedTokens);
      if (next.has(entry.token)) next.delete(entry.token);
      else next.add(entry.token);
      updateActiveTab({
        selectedTokens: next,
        focusedIndex: index,
        anchorIndex: index,
      });
    } else if (e.shiftKey) {
      const anchor = cur.anchorIndex >= 0 ? cur.anchorIndex : 0;
      const start = Math.min(anchor, index);
      const end = Math.max(anchor, index);
      const next = new Set<string>();
      for (let i = start; i <= end; i++) {
        if (displayedEntries[i]) {
          next.add(displayedEntries[i].token);
        }
      }
      updateActiveTab({
        selectedTokens: next,
        focusedIndex: index,
      });
    } else {
      updateActiveTab({
        selectedTokens: new Set([entry.token]),
        focusedIndex: index,
        anchorIndex: index,
      });
    }
  }

  // Keyboard navigation helpers
  function navigateRow(delta: number, isShift: boolean) {
    if (displayedEntries.length === 0) return;
    const cur = activeTabRef.current;
    let next = cur.focusedIndex + delta;
    if (cur.focusedIndex === -1) {
      next = delta > 0 ? 0 : displayedEntries.length - 1;
    }
    next = Math.max(0, Math.min(displayedEntries.length - 1, next));
    const anchor = cur.anchorIndex >= 0 ? cur.anchorIndex : next;

    if (isShift) {
      const start = Math.min(anchor, next);
      const end = Math.max(anchor, next);
      const newSelected = new Set<string>();
      for (let i = start; i <= end; i++) {
        newSelected.add(displayedEntries[i].token);
      }
      updateActiveTab({
        focusedIndex: next,
        anchorIndex: anchor,
        selectedTokens: newSelected,
      });
    } else {
      updateActiveTab({
        focusedIndex: next,
        anchorIndex: next,
        selectedTokens: new Set([displayedEntries[next].token]),
      });
    }

    rowVirtualizer.scrollToIndex(next, { align: "auto" });
  }

  function jumpToRow(index: number, isShift: boolean) {
    if (displayedEntries.length === 0) return;
    const cur = activeTabRef.current;
    const target = Math.max(0, Math.min(displayedEntries.length - 1, index));
    const anchor = cur.anchorIndex >= 0 ? cur.anchorIndex : target;

    if (isShift) {
      const start = Math.min(anchor, target);
      const end = Math.max(anchor, target);
      const newSelected = new Set<string>();
      for (let i = start; i <= end; i++) {
        newSelected.add(displayedEntries[i].token);
      }
      updateActiveTab({
        focusedIndex: target,
        anchorIndex: anchor,
        selectedTokens: newSelected,
      });
    } else {
      updateActiveTab({
        focusedIndex: target,
        anchorIndex: target,
        selectedTokens: new Set([displayedEntries[target].token]),
      });
    }

    rowVirtualizer.scrollToIndex(target, { align: "auto" });
  }

  function selectAll() {
    const allTokens = new Set<string>(displayedEntries.map((e) => e.token));
    updateActiveTab({ selectedTokens: allTokens });
  }

  // Favorites management
  async function handleAddFavorite(path: string) {
    if (!path) return;
    try {
      await client.addFavorite(path);
      setFavorites((prev) => (prev.includes(path) ? prev : [...prev, path]));
    } catch {
      // Ignore
    }
  }

  async function handleRemoveFavorite(path: string) {
    try {
      await client.removeFavorite(path);
      setFavorites((prev) => prev.filter((p) => p !== path));
    } catch {
      // Ignore
    }
  }

  // Modal dialog handlers (New Folder & Rename)
  function openCreateFolderModal() {
    if (!activeTab.folderToken) return;
    setContextMenu(null);
    setModal({
      type: "create_folder",
      title: "New Folder",
      value: "New folder",
      folderToken: activeTab.folderToken,
    });
    setTimeout(() => {
      modalInputRef.current?.focus();
      modalInputRef.current?.select();
    }, 50);
  }

  function openRenameModal(entry?: FileEntry) {
    const targetEntry =
      entry ||
      (activeTab.selectedTokens.size === 1
        ? displayedEntries.find((e) => e.token === Array.from(activeTab.selectedTokens)[0])
        : undefined);

    if (!targetEntry || !activeTab.folderToken) return;
    setContextMenu(null);
    setModal({
      type: "rename",
      title: `Rename "${targetEntry.display_name}"`,
      value: targetEntry.display_name,
      folderToken: activeTab.folderToken,
      itemToken: targetEntry.token,
    });
    setTimeout(() => {
      modalInputRef.current?.focus();
      modalInputRef.current?.select();
    }, 50);
  }

  function openRecycleModal(entry?: FileEntry) {
    const targetTokens = entry
      ? [entry.token]
      : Array.from(activeTab.selectedTokens);
    if (targetTokens.length === 0) return;

    const targetEntries = displayedEntries.filter((e) =>
      targetTokens.includes(e.token)
    );
    if (targetEntries.length === 0) return;

    setContextMenu(null);
    setModal({
      type: "recycle",
      title:
        targetEntries.length === 1
          ? `Recycle "${targetEntries[0].display_name}"?`
          : `Recycle ${targetEntries.length} items?`,
      value:
        targetEntries.length === 1
          ? targetEntries[0].display_name
          : `${targetEntries.length} items`,
      folderToken: activeTab.folderToken || "",
      targetTokens: targetEntries.map((entry) => entry.token),
    });
  }

  async function handleCopy() {
    if (activeTab.selectedTokens.size === 0) return;
    const itemTokens = displayedEntries
      .filter((e) => activeTab.selectedTokens.has(e.token))
      .map((entry) => entry.token);
    if (itemTokens.length === 0) return;
    try {
      await client.clipboardWriteItems(activeTab.folderToken, itemTokens, false);
    } catch (err: any) {
      console.error("Failed to copy:", err);
    }
  }

  async function handleCut() {
    if (activeTab.selectedTokens.size === 0) return;
    const itemTokens = displayedEntries
      .filter((e) => activeTab.selectedTokens.has(e.token))
      .map((entry) => entry.token);
    if (itemTokens.length === 0) return;
    try {
      await client.clipboardWriteItems(activeTab.folderToken, itemTokens, true);
    } catch (err: any) {
      console.error("Failed to cut:", err);
    }
  }

  function showJobResult(job: JobSummary) {
    setJobs((current) => [job, ...current.filter((existing) => existing.id !== job.id)]);
    if (job.state !== "succeeded") setShowJobsDrawer(true);
  }

  async function handlePaste() {
    if (!activeTab.folderToken) return;
    try {
      const plan = await client.planPaste(activeTab.folderToken);
      if (!plan) return;
      showJobResult(await client.commitPlan(plan.id));
      refresh();
      refreshJobs();
    } catch (err: any) {
      refreshJobs();
      alert(err?.user_message || "Paste operation failed");
    }
  }

  async function handleModalSubmit(e?: React.FormEvent) {
    if (e) e.preventDefault();
    if (!modal) return;

    try {
      if (modal.type === "recycle") {
        if (modal.targetTokens && modal.targetTokens.length > 0) {
          const plan = await client.planRecycle(modal.folderToken, modal.targetTokens);
          showJobResult(await client.commitPlan(plan.id));
        }
      } else {
        const name = modal.value;
        if (!name.trim()) {
          setModal({ ...modal, error: "Name cannot be empty" });
          return;
        }
        if (modal.type === "create_folder") {
          showJobResult(await client.createFolder(modal.folderToken, name));
        } else if (modal.type === "rename" && modal.itemToken) {
          showJobResult(await client.renameItem(modal.folderToken, modal.itemToken, name));
        }
      }
      setModal(null);
      refresh();
      refreshJobs();
    } catch (err: any) {
      setModal({ ...modal, error: err?.user_message || "Operation failed" });
      refreshJobs();
    }
  }

  // Context menu opening
  function handleRowContextMenu(entry: FileEntry, e: React.MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const cur = activeTabRef.current;
    if (!cur.selectedTokens.has(entry.token)) {
      const idx = displayedEntries.findIndex((it) => it.token === entry.token);
      updateActiveTab({
        selectedTokens: new Set([entry.token]),
        focusedIndex: idx,
        anchorIndex: idx,
      });
    }
    const x = Math.min(e.clientX, window.innerWidth - 220);
    const y = Math.min(e.clientY, window.innerHeight - 220);
    setContextMenu({ x, y, entry });
  }

  function handleBackgroundContextMenu(e: React.MouseEvent) {
    e.preventDefault();
    const x = Math.min(e.clientX, window.innerWidth - 220);
    const y = Math.min(e.clientY, window.innerHeight - 220);
    setContextMenu({ x, y, entry: undefined });
  }

  // Close context menu on outside click
  useEffect(() => {
    function handleClickOutside() {
      setContextMenu(null);
    }
    if (contextMenu) {
      window.addEventListener("click", handleClickOutside);
      return () => window.removeEventListener("click", handleClickOutside);
    }
  }, [contextMenu]);

  // Global Keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Escape -> Dismiss modal, context menu, drawer, or clear selection
      if (e.key === "Escape") {
        if (modal) {
          setModal(null);
          return;
        }
        if (contextMenu) {
          setContextMenu(null);
          return;
        }
        if (showJobsDrawer) {
          setShowJobsDrawer(false);
          return;
        }
        if (
          document.activeElement instanceof HTMLInputElement ||
          document.activeElement instanceof HTMLTextAreaElement
        ) {
          (document.activeElement as HTMLElement).blur();
        } else {
          updateActiveTab({ selectedTokens: new Set() });
        }
        return;
      }

      // If modal is active, enter submits
      if (modal) {
        if (e.key === "Enter") {
          e.preventDefault();
          handleModalSubmit();
        }
        return;
      }

      const isInputActive =
        document.activeElement instanceof HTMLInputElement ||
        document.activeElement instanceof HTMLTextAreaElement;

      // Ctrl+Shift+N -> New Folder
      if (e.ctrlKey && e.shiftKey && (e.key === "n" || e.key === "N")) {
        e.preventDefault();
        openCreateFolderModal();
        return;
      }

      // F2 -> Rename single selected item
      if (e.key === "F2" && !isInputActive) {
        e.preventDefault();
        openRenameModal();
        return;
      }

      // Alt+Left -> Back
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        goBack();
        return;
      }
      // Alt+Right -> Forward
      if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault();
        goForward();
        return;
      }
      // Alt+Up -> Parent
      if (e.altKey && e.key === "ArrowUp") {
        e.preventDefault();
        goUp();
        return;
      }
      // Ctrl+T -> New Tab
      if (e.ctrlKey && (e.key === "t" || e.key === "T")) {
        e.preventDefault();
        createNewTab();
        return;
      }
      // Ctrl+W -> Close Tab
      if (e.ctrlKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        closeTab(activeTabIndexRef.current);
        return;
      }
      // Ctrl+Tab / Ctrl+Shift+Tab -> Cycle tabs
      if (e.ctrlKey && e.key === "Tab") {
        e.preventDefault();
        cycleTab(e.shiftKey ? -1 : 1);
        return;
      }
      // Ctrl+L -> Focus address bar
      if (e.ctrlKey && (e.key === "l" || e.key === "L")) {
        e.preventDefault();
        addressInputRef.current?.focus();
        addressInputRef.current?.select();
        return;
      }
      // Ctrl+F -> Focus filter
      if (e.ctrlKey && (e.key === "f" || e.key === "F")) {
        e.preventDefault();
        filterInputRef.current?.focus();
        filterInputRef.current?.select();
        return;
      }
      // F5 -> Refresh
      if (e.key === "F5") {
        e.preventDefault();
        refresh();
        return;
      }

      // Keys that must NOT trigger table navigation while typing in inputs
      if (isInputActive) return;

      // Ctrl+C -> Copy
      if (e.ctrlKey && (e.key === "c" || e.key === "C")) {
        e.preventDefault();
        handleCopy();
        return;
      }

      // Ctrl+X -> Cut
      if (e.ctrlKey && (e.key === "x" || e.key === "X")) {
        e.preventDefault();
        handleCut();
        return;
      }

      // Ctrl+V -> Paste
      if (e.ctrlKey && (e.key === "v" || e.key === "V")) {
        e.preventDefault();
        handlePaste();
        return;
      }

      // Delete -> Recycle selected items
      if (e.key === "Delete") {
        e.preventDefault();
        if (e.shiftKey) {
          alert("Permanent deletion is not supported. Use Recycle Bin instead.");
          return;
        }
        openRecycleModal();
        return;
      }

      // Ctrl+A -> Select all
      if (e.ctrlKey && (e.key === "a" || e.key === "A")) {
        e.preventDefault();
        selectAll();
        return;
      }

      // Row navigation
      if (e.key === "ArrowDown") {
        e.preventDefault();
        navigateRow(1, e.shiftKey);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        navigateRow(-1, e.shiftKey);
        return;
      }
      if (e.key === "Home") {
        e.preventDefault();
        jumpToRow(0, e.shiftKey);
        return;
      }
      if (e.key === "End") {
        e.preventDefault();
        jumpToRow(displayedEntries.length - 1, e.shiftKey);
        return;
      }

      // Enter -> Open selected item
      if (e.key === "Enter") {
        const cur = activeTabRef.current;
        if (cur.selectedTokens.size === 1) {
          const token = Array.from(cur.selectedTokens)[0];
          const entry = displayedEntries.find((item) => item.token === token);
          if (entry) {
            e.preventDefault();
            handleItemDoubleClick(entry);
          }
        }
        return;
      }

      // Indexed search results keyboard navigation
      if (isSearchActive) {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          setSelectedSearchIndex((prev) => Math.min(prev + 1, searchResults.length - 1));
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          setSelectedSearchIndex((prev) => Math.max(prev - 1, 0));
          return;
        }
        if (e.key === "Enter") {
          if (selectedSearchIndex >= 0 && selectedSearchIndex < searchResults.length) {
            e.preventDefault();
            handleSearchResultDoubleClick(searchResults[selectedSearchIndex]);
          }
          return;
        }
        if (e.key === "Escape") {
          setIndexedSearchQuery("");
          return;
        }
      }

      // Alt+Enter -> Native properties
      if (e.altKey && e.key === "Enter") {
        e.preventDefault();
        const cur = activeTabRef.current;
        if (cur.selectedTokens.size === 1) {
          const token = Array.from(cur.selectedTokens)[0];
          client.showProperties(cur.folderToken, token);
        } else if (cur.folderToken) {
          client.showProperties(cur.folderToken, null);
        }
        return;
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [displayedEntries, contextMenu, modal, showJobsDrawer, isSearchActive, searchResults, selectedSearchIndex]);

  return (
    <div className="app-container" onContextMenu={handleBackgroundContextMenu}>
      {/* Tab bar and Navigation Toolbar */}
      <header className="app-header">
        <div className="tab-bar" role="tablist">
          {tabs.map((tab, idx) => (
            <div
              key={tab.id}
              className={`tab-item ${idx === activeTabIndex ? "active" : ""}`}
              role="tab"
              aria-selected={idx === activeTabIndex}
              onClick={() => switchTab(idx)}
              title={tab.path}
            >
              <span className="tab-title">{tab.title}</span>
              <button
                className="tab-close-btn"
                aria-label="Close tab"
                title="Close Tab (Ctrl+W)"
                onClick={(e) => closeTab(idx, e)}
              >
                ×
              </button>
            </div>
          ))}
          <button
            className="new-tab-btn"
            aria-label="New tab"
            title="New Tab (Ctrl+T)"
            onClick={() => createNewTab()}
          >
            +
          </button>
        </div>

        <div className="nav-toolbar">
          <div className="nav-buttons">
            <button
              onClick={goBack}
              disabled={activeTab.historyIndex <= 0}
              aria-label="Back"
              title="Back (Alt+Left)"
            >
              ←
            </button>
            <button
              onClick={goForward}
              disabled={activeTab.historyIndex >= activeTab.history.length - 1}
              aria-label="Forward"
              title="Forward (Alt+Right)"
            >
              →
            </button>
            <button onClick={goUp} aria-label="Up" title="Up to Parent (Alt+Up)">
              ↑
            </button>
            <button onClick={refresh} aria-label="Refresh" title="Refresh (F5)">
              🔄
            </button>
          </div>

          <button
            className="action-btn"
            onClick={openCreateFolderModal}
            title="New Folder (Ctrl+Shift+N)"
            aria-label="New folder"
          >
            <span>📁+</span>
            <span>New Folder</span>
          </button>

          <button
            className="action-btn"
            onClick={handleCopy}
            disabled={activeTab.selectedTokens.size === 0}
            title="Copy (Ctrl+C)"
            aria-label="Copy"
          >
            <span>📋</span>
            <span>Copy</span>
          </button>

          <button
            className="action-btn"
            onClick={handleCut}
            disabled={activeTab.selectedTokens.size === 0}
            title="Cut (Ctrl+X)"
            aria-label="Cut"
          >
            <span>✂️</span>
            <span>Cut</span>
          </button>

          <button
            className="action-btn"
            onClick={handlePaste}
            title="Paste (Ctrl+V)"
            aria-label="Paste"
          >
            <span>📥</span>
            <span>Paste</span>
          </button>

          <button
            className="action-btn"
            onClick={() => openRecycleModal()}
            disabled={activeTab.selectedTokens.size === 0}
            title="Recycle (Delete)"
            aria-label="Delete"
          >
            <span>🗑️</span>
            <span>Delete</span>
          </button>

          <form
            className="address-bar-container"
            onSubmit={(e) => {
              e.preventDefault();
              navigateToPath(activeTab.addressInput);
            }}
          >
            <input
              ref={addressInputRef}
              type="text"
              className="address-input"
              value={activeTab.addressInput}
              onChange={(e) => updateActiveTab({ addressInput: e.target.value })}
              aria-label="Address"
              placeholder="Enter a path... (Ctrl+L)"
            />
          </form>

          <div className="search-container">
            <div className="search-scope-switcher">
              <button
                type="button"
                className={`scope-btn ${searchScope === "folder" ? "active" : ""}`}
                onClick={() => {
                  setSearchScope("folder");
                  filterInputRef.current?.focus();
                }}
                title="Filter items in current folder only"
              >
                Current Folder
              </button>
              <button
                type="button"
                className={`scope-btn ${searchScope === "indexed" ? "active" : ""}`}
                onClick={() => {
                  setSearchScope("indexed");
                  if (activeTab.filterQuery && !indexedSearchQuery) {
                    setIndexedSearchQuery(activeTab.filterQuery);
                  }
                  filterInputRef.current?.focus();
                }}
                title="Search subfolders & indexed files (SQLite FTS5)"
              >
                Subfolders (Indexed)
              </button>
            </div>
            <div className="search-input-wrapper">
              <input
                ref={filterInputRef}
                type="text"
                className="search-input"
                placeholder={
                  searchScope === "folder"
                    ? "Filter current folder (Press Enter to search subfolders)..."
                    : "Search subfolders (e.g. invoice ext:pdf)..."
                }
                value={searchScope === "folder" ? activeTab.filterQuery : indexedSearchQuery}
                onChange={(e) => {
                  if (searchScope === "folder") {
                    updateActiveTab({ filterQuery: e.target.value });
                  } else {
                    setIndexedSearchQuery(e.target.value);
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && searchScope === "folder" && activeTab.filterQuery.trim()) {
                    e.preventDefault();
                    handleSearchSubfolders();
                  }
                }}
                aria-label={searchScope === "folder" ? "Filter current folder" : "Search subfolders"}
              />
              {searchScope === "folder" && activeTab.filterQuery.trim().length >= 2 && (
                <button
                  type="button"
                  className="search-subfolders-quick-btn"
                  onClick={() => handleSearchSubfolders()}
                  title="Search inside subfolders"
                >
                  🔍 Subfolders
                </button>
              )}
              {searchScope === "indexed" && searchLoading && (
                <span className="search-spinner" title="Searching...">⏳</span>
              )}
            </div>
          </div>

          <button
            className="theme-toggle-btn"
            onClick={toggleTheme}
            title={`Current theme: ${theme}. Click to switch theme.`}
            aria-label="Toggle theme"
          >
            {theme === "system" ? "💻 Auto" : theme === "dark" ? "🌙 Dark" : "☀️ Light"}
          </button>
          <button
            className="theme-toggle-btn"
            onClick={toggleHiddenFiles}
            title="Show or hide hidden files"
            aria-label="Toggle hidden files"
            aria-pressed={showHiddenFiles}
          >
            {showHiddenFiles ? "Hidden On" : "Hidden Off"}
          </button>
        </div>
      </header>

      {/* Main Body */}
      <div className="app-body">
        {/* Sidebar */}
        <aside className="sidebar">
          {/* Favorites */}
          <section className="sidebar-section">
            <h3>Favorites</h3>
            {favorites.length === 0 ? (
              <div className="sidebar-empty-note">No pinned favorites</div>
            ) : (
              <ul>
                {favorites.map((favPath) => (
                  <li
                    key={favPath}
                    className={activeTab.path === favPath ? "active" : ""}
                    onClick={() => navigateToPath(favPath)}
                  >
                    <div className="sidebar-fav-item">
                      <span className="sidebar-icon">⭐</span>
                      <span className="sidebar-label" title={favPath}>
                        {getTabTitle(favPath)}
                      </span>
                    </div>
                    <button
                      className="fav-remove-btn"
                      title="Remove from favorites"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRemoveFavorite(favPath);
                      }}
                    >
                      ×
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          {/* Known Folders */}
          <section className="sidebar-section">
            <h3>Known Folders</h3>
            <ul>
              {knownFolders.map((kf) => (
                <li
                  key={kf.id}
                  onClick={() => navigateToPath(kf.path)}
                  className={activeTab.path === kf.path ? "active" : ""}
                >
                  <div className="sidebar-fav-item">
                    <span className="sidebar-icon">📁</span>
                    <span className="sidebar-label">{kf.name}</span>
                  </div>
                </li>
              ))}
            </ul>
          </section>

          {/* Drives */}
          <section className="sidebar-section">
            <h3>Drives</h3>
            <ul>
              {drives.map((d) => (
                <li
                  key={d.path}
                  onClick={() => navigateToPath(d.path)}
                  className={activeTab.path === d.path ? "active" : ""}
                >
                  <div className="sidebar-fav-item">
                    <span className="sidebar-icon">💾</span>
                    <span className="sidebar-label">{d.name}</span>
                  </div>
                </li>
              ))}
            </ul>
          </section>

          {/* Indexed Roots */}
          <section className="sidebar-section">
            <div className="sidebar-section-header">
              <h3>Indexed Roots ({indexedRoots.length}/8)</h3>
              <button
                className="sidebar-add-btn"
                title="Index current folder"
                disabled={indexedRoots.length >= 8 || !activeTab.folderToken}
                onClick={handleIndexCurrentFolder}
              >
                +
              </button>
            </div>
            {indexedRoots.length === 0 ? (
              <div className="sidebar-empty-note">
                No indexed roots
                {activeTab.folderToken && (
                  <button
                    className="btn-link"
                    onClick={handleIndexCurrentFolder}
                  >
                    Index current folder
                  </button>
                )}
              </div>
            ) : (
              <ul className="indexed-roots-list">
                {indexedRoots.map((root) => (
                  <li key={root.id} className="indexed-root-item">
                    <div
                      className="indexed-root-info"
                      onClick={() => navigateToPath(root.path_utf16)}
                      title={`Path: ${root.display_path}\nState: ${root.state}\nEpoch: ${root.completed_epoch}`}
                    >
                      <span className="root-name">{getTabTitle(root.display_path)}</span>
                      <span className={`root-badge badge-${root.state}`}>{root.state}</span>
                    </div>
                    <div className="root-actions">
                      <button
                        className="root-recrawl-btn"
                        title="Re-crawl index"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleRecrawlRoot(root.id);
                        }}
                      >
                        🔄
                      </button>
                      <button
                        className="fav-remove-btn"
                        title="Remove from index"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleRemoveRoot(root.id);
                        }}
                      >
                        ×
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </aside>

        {/* Content Pane */}
        <main className="content-pane" role="region" aria-label="Folder contents">
          {isSearchActive ? (
            <>
              <div className="file-table-header">
                <span className="col col-name">Name</span>
                <span className="col col-path">Location</span>
                <span className="col col-size">Size</span>
                <span className="col col-date">Date modified</span>
              </div>

              <div
                className="file-table-body"
                ref={parentRef}
                onContextMenu={handleBackgroundContextMenu}
              >
                {searchError && (
                  <div className="error-banner">
                    <p>⚠️ {searchError}</p>
                  </div>
                )}

                {isAnyRootScanning && (
                  <div
                    className="scanning-banner"
                    style={{
                      padding: "8px 16px",
                      background: "rgba(59, 130, 246, 0.12)",
                      borderBottom: "1px solid rgba(59, 130, 246, 0.2)",
                      fontSize: "0.85rem",
                      color: "var(--accent-color, #3b82f6)",
                      display: "flex",
                      alignItems: "center",
                      gap: "8px",
                    }}
                  >
                    <span>🔄</span> Indexing in progress... Results update live as files are scanned.
                  </div>
                )}

                {searchLoading && searchResults.length === 0 ? (
                  <div className="empty-state">
                    <p>Searching indexed files...</p>
                  </div>
                ) : searchResults.length === 0 ? (
                  <div className="empty-state">
                    {isAnyRootScanning ? (
                      <p>🔄 Indexing folder in progress... Matching files will appear as they are scanned.</p>
                    ) : (
                      <p>No results found for "{indexedSearchQuery}".</p>
                    )}
                  </div>
                ) : (
                  <div
                    style={{
                      height: `${searchVirtualizer.getTotalSize()}px`,
                      width: "100%",
                      position: "relative",
                    }}
                  >
                    {searchVirtualizer.getVirtualItems().map((virtualRow) => {
                      const item = searchResults[virtualRow.index];
                      const isSelected = selectedSearchIndex === virtualRow.index;

                      return (
                        <div
                          key={item.id}
                          className={`file-row ${isSelected ? "selected" : ""}`}
                          onClick={() => setSelectedSearchIndex(virtualRow.index)}
                          onDoubleClick={() => handleSearchResultDoubleClick(item)}
                          onContextMenu={(e) => handleSearchResultContextMenu(item, e)}
                          style={{
                            position: "absolute",
                            top: 0,
                            left: 0,
                            width: "100%",
                            height: `${virtualRow.size}px`,
                            transform: `translateY(${virtualRow.start}px)`,
                          }}
                        >
                          <span className="col col-name file-name-cell">
                            <span className="file-icon">{getFileIcon(item)}</span>
                            <span className="file-name" title={item.display_name}>
                              {item.display_name}
                            </span>
                          </span>
                          <span className="col col-path" title={item.path}>
                            {item.path}
                          </span>
                          <span className="col col-size">
                            {formatBytes(item.size_bytes)}
                          </span>
                          <span className="col col-date">
                            {formatFiletime(item.modified_filetime)}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </>
          ) : searchScope === "indexed" ? (
            <>
              <div className="file-table-header">
                <span className="col col-name">Name</span>
                <span className="col col-path">Location</span>
                <span className="col col-size">Size</span>
                <span className="col col-date">Date modified</span>
              </div>
              <div
                className="file-table-body"
                ref={parentRef}
                onContextMenu={handleBackgroundContextMenu}
              >
                <div className="empty-state">
                  <p>Type at least 3 characters to search across indexed roots.</p>
                  <p style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "4px" }}>
                    Tip: Use filters like <code>ext:pdf</code>, <code>type:folder</code>, or <code>"exact phrase"</code>.
                  </p>
                </div>
              </div>
            </>
          ) : (
            <>
              <div className="file-table-header">
                <span
                  className={`col col-name sortable ${activeTab.sortColumn === "name" ? "sorted" : ""}`}
                  onClick={() => handleSort("name")}
                >
                  Name {activeTab.sortColumn === "name" && (activeTab.sortDirection === "ascending" ? "▲" : "▼")}
                </span>
                <span
                  className={`col col-type sortable ${activeTab.sortColumn === "type" ? "sorted" : ""}`}
                  onClick={() => handleSort("type")}
                >
                  Type {activeTab.sortColumn === "type" && (activeTab.sortDirection === "ascending" ? "▲" : "▼")}
                </span>
                <span
                  className={`col col-size sortable ${activeTab.sortColumn === "size" ? "sorted" : ""}`}
                  onClick={() => handleSort("size")}
                >
                  Size {activeTab.sortColumn === "size" && (activeTab.sortDirection === "ascending" ? "▲" : "▼")}
                </span>
                <span
                  className={`col col-date sortable ${activeTab.sortColumn === "modified" ? "sorted" : ""}`}
                  onClick={() => handleSort("modified")}
                >
                  Date modified {activeTab.sortColumn === "modified" && (activeTab.sortDirection === "ascending" ? "▲" : "▼")}
                </span>
              </div>

              <div
                className="file-table-body"
                ref={parentRef}
                onContextMenu={handleBackgroundContextMenu}
              >
                {activeTab.error && (
                  <div className="error-banner">
                    <p>⚠️ {activeTab.error}</p>
                  </div>
                )}

                {activeTab.loading && activeTab.entries.length === 0 ? (
                  <div className="empty-state">
                    <p>Loading folder contents...</p>
                  </div>
                ) : displayedEntries.length === 0 ? (
                  <div className="empty-state">
                    {activeTab.filterQuery ? (
                      <div className="filter-empty-prompt">
                        <p>No items named "{activeTab.filterQuery}" found directly in this folder.</p>
                        <button
                          type="button"
                          className="action-btn"
                          style={{
                            marginTop: "12px",
                            padding: "8px 16px",
                            fontSize: "13px",
                            cursor: "pointer",
                            fontWeight: 500,
                          }}
                          onClick={() => handleSearchSubfolders()}
                        >
                          🔍 Search inside subfolders for "{activeTab.filterQuery}"
                        </button>
                      </div>
                    ) : !showHiddenFiles && activeTab.entries.some((entry) => entry.is_hidden) ? (
                      <p>No visible items. Hidden files are turned off.</p>
                    ) : (
                      <p>This folder is empty.</p>
                    )}
                  </div>
                ) : (
                  <div
                    style={{
                      height: `${rowVirtualizer.getTotalSize()}px`,
                      width: "100%",
                      position: "relative",
                    }}
                    role="listbox"
                    aria-label={`Folder items in ${activeTab.path}`}
                    aria-multiselectable="true"
                    aria-busy={activeTab.loading}
                  >
                    {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                      const entry = displayedEntries[virtualRow.index];
                      const isSelected = activeTab.selectedTokens.has(entry.token);

                      return (
                        <div
                          key={entry.token}
                          className={`file-row ${isSelected ? "selected" : ""}`}
                          role="option"
                          aria-selected={isSelected}
                          tabIndex={
                            activeTab.focusedIndex === virtualRow.index ||
                            (activeTab.focusedIndex < 0 && virtualRow.index === 0)
                              ? 0
                              : -1
                          }
                          onClick={(e) => handleRowClick(entry, virtualRow.index, e)}
                          onDoubleClick={() => handleItemDoubleClick(entry)}
                          onContextMenu={(e) => handleRowContextMenu(entry, e)}
                          style={{
                            position: "absolute",
                            top: 0,
                            left: 0,
                            width: "100%",
                            height: `${virtualRow.size}px`,
                            transform: `translateY(${virtualRow.start}px)`,
                          }}
                        >
                          <span className="col col-name file-name-cell">
                            <span className="file-icon">{getFileIcon(entry)}</span>
                            <span className="file-name" title={entry.display_name}>
                              {entry.display_name}
                            </span>
                          </span>
                          <span className="col col-type">
                            {entry.kind === "directory"
                              ? "File folder"
                              : entry.extension
                              ? `${entry.extension.toUpperCase()} File`
                              : "File"}
                          </span>
                          <span className="col col-size">
                            {formatBytes(entry.size_bytes)}
                          </span>
                          <span className="col col-date">
                            {formatFiletime(entry.modified_filetime)}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </>
          )}
        </main>
      </div>

      {/* Status Bar */}
      <footer className="app-statusbar">
        <span>
          {isSearchActive ? (
            <>
              {searchTotalMatches} {searchTotalMatches === 1 ? "match" : "matches"}
              {searchIsCapped && " (First 1,000 matches; refine search)"}
              {selectedSearchIndex >= 0 && " | 1 selected"}
            </>
          ) : (
            <>
              {displayedEntries.length} {displayedEntries.length === 1 ? "item" : "items"}
              {activeTab.filterQuery && ` (filtered from ${visibleEntryCount})`}
              {activeTab.selectedTokens.size > 0 && ` | ${activeTab.selectedTokens.size} selected`}
            </>
          )}
        </span>
        <span className="status-spacer"></span>
        <button
          className="action-btn"
          style={{ padding: "2px 8px", fontSize: "11px", marginRight: "8px" }}
          onClick={() => {
            setShowJobsDrawer(!showJobsDrawer);
            refreshJobs();
          }}
          title="Toggle Jobs Drawer"
        >
          ⚡ Jobs {jobs.length > 0 && `(${jobs.length})`}
        </button>
        <span>{activeTab.path}</span>
      </footer>

      {/* Context Menu */}
      {contextMenu && (
        <div
          className="context-menu"
          style={{ top: `${contextMenu.y}px`, left: `${contextMenu.x}px` }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.searchResult ? (
            <>
              <div
                className="context-menu-item"
                onClick={() => {
                  const item = contextMenu.searchResult!;
                  setContextMenu(null);
                  handleSearchResultDoubleClick(item);
                }}
              >
                <span>Open</span>
                <span className="context-menu-shortcut">Enter</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  const item = contextMenu.searchResult!;
                  setContextMenu(null);
                  const separator = Math.max(
                    item.path_utf16.lastIndexOf(92),
                    item.path_utf16.lastIndexOf(47)
                  );
                  if (separator > 0) {
                    const driveRootSeparator =
                      separator === 2 && item.path_utf16[1] === 58;
                    navigateToPath(
                      item.path_utf16.slice(0, separator + (driveRootSeparator ? 1 : 0))
                    );
                  }
                }}
              >
                <span>Open containing folder</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  const item = contextMenu.searchResult!;
                  setContextMenu(null);
                  client.clipboardWritePath(item.path_utf16);
                }}
              >
                <span>Copy path</span>
              </div>
            </>
          ) : contextMenu.entry ? (
            <>
              <div
                className="context-menu-item"
                onClick={() => {
                  const entry = contextMenu.entry!;
                  setContextMenu(null);
                  handleItemDoubleClick(entry);
                }}
              >
                <span>Open</span>
                <span className="context-menu-shortcut">Enter</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  handleCopy();
                }}
              >
                <span>Copy</span>
                <span className="context-menu-shortcut">Ctrl+C</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  handleCut();
                }}
              >
                <span>Cut</span>
                <span className="context-menu-shortcut">Ctrl+X</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  const entry = contextMenu.entry!;
                  openRenameModal(entry);
                }}
              >
                <span>Rename</span>
                <span className="context-menu-shortcut">F2</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  const entry = contextMenu.entry!;
                  openRecycleModal(entry);
                }}
              >
                <span>Delete</span>
                <span className="context-menu-shortcut">Del</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  const entry = contextMenu.entry!;
                  setContextMenu(null);
                  client.openInExplorer(activeTab.folderToken, entry.token);
                }}
              >
                <span>Open in Windows Explorer</span>
              </div>
              {contextMenu.entry.kind === "directory" && (
                <div
                  className="context-menu-item"
                  onClick={() => {
                    const entry = contextMenu.entry!;
                    setContextMenu(null);
                    const fullPath = activeTab.path.endsWith("\\")
                      ? activeTab.path + entry.display_name
                      : activeTab.path + "\\" + entry.display_name;
                    handleAddFavorite(fullPath);
                  }}
                >
                  <span>Add to Favorites</span>
                </div>
              )}
              <div className="context-menu-separator" />
              <div
                className="context-menu-item"
                onClick={() => {
                  const entry = contextMenu.entry!;
                  setContextMenu(null);
                  client.showProperties(activeTab.folderToken, entry.token);
                }}
              >
                <span>Properties</span>
                <span className="context-menu-shortcut">Alt+Enter</span>
              </div>
            </>
          ) : (
            <>
              <div
                className="context-menu-item"
                onClick={() => {
                  openCreateFolderModal();
                }}
              >
                <span>New Folder</span>
                <span className="context-menu-shortcut">Ctrl+Shift+N</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  handlePaste();
                }}
              >
                <span>Paste</span>
                <span className="context-menu-shortcut">Ctrl+V</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  refresh();
                }}
              >
                <span>Refresh</span>
                <span className="context-menu-shortcut">F5</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  client.openInExplorer(activeTab.folderToken, null);
                }}
              >
                <span>Open in Windows Explorer</span>
              </div>
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  handleAddFavorite(activeTab.path);
                }}
              >
                <span>Add Current Folder to Favorites</span>
              </div>
              <div className="context-menu-separator" />
              <div
                className="context-menu-item"
                onClick={() => {
                  setContextMenu(null);
                  client.showProperties(activeTab.folderToken, null);
                }}
              >
                <span>Properties</span>
                <span className="context-menu-shortcut">Alt+Enter</span>
              </div>
            </>
          )}
        </div>
      )}

      {/* Modal Dialog (New Folder / Rename / Recycle) */}
      {modal && (
        <div className="modal-overlay" onClick={() => setModal(null)}>
          <div className="modal-container" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">{modal.title}</div>
            <form onSubmit={handleModalSubmit}>
              <div className="modal-body">
                {modal.type === "recycle" ? (
                  <div style={{ fontSize: "14px", lineHeight: "1.5" }}>
                    Are you sure you want to send{" "}
                    <strong>
                      {modal.targetTokens?.length === 1
                        ? `"${modal.value}"`
                        : `${modal.targetTokens?.length} items`}
                    </strong>{" "}
                    to the Recycle Bin?
                    <div style={{ marginTop: "8px", fontSize: "12px", color: "var(--text-muted)" }}>
                      Permanent deletion fallback is disabled for safety.
                    </div>
                  </div>
                ) : (
                  <input
                    ref={modalInputRef}
                    type="text"
                    className="modal-input"
                    value={modal.value}
                    onChange={(e) => setModal({ ...modal, value: e.target.value, error: null })}
                    placeholder="Enter name..."
                  />
                )}
                {modal.error && (
                  <div style={{ color: "var(--error-text)", fontSize: "12px", marginTop: "8px" }}>
                    ⚠️ {modal.error}
                  </div>
                )}
              </div>
              <div className="modal-footer">
                <button
                  type="button"
                  className="modal-btn modal-btn-secondary"
                  onClick={() => setModal(null)}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className={`modal-btn ${modal.type === "recycle" ? "modal-btn-danger" : "modal-btn-primary"}`}
                >
                  {modal.type === "create_folder" ? "Create" : modal.type === "rename" ? "Rename" : "Recycle"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Job Drawer */}
      {showJobsDrawer && (
        <div className="jobs-drawer">
          <div className="jobs-header">
            <span>Operation Jobs ({jobs.length})</span>
            <div className="jobs-header-actions">
              <button
                className="jobs-close-btn"
                onClick={() => setShowJobsDrawer(false)}
                title="Close"
              >
                ×
              </button>
            </div>
          </div>
          <div className="jobs-list">
            {jobs.length === 0 ? (
              <div className="jobs-empty-note">No recent operation jobs</div>
            ) : (
              jobs.map((job) => (
                <div key={job.id} className="job-card">
                  <div className="job-card-header">
                    <span className="job-card-title">{job.kind.replace("_", " ")}</span>
                    <span className={`job-badge ${job.state}`}>{job.state}</span>
                  </div>
                  <div className="job-card-details">
                    <span>
                      {job.completed_items} succeeded · {job.failed_items} failed ·{" "}
                      {job.canceled_items} canceled · {job.skipped_items} skipped
                    </span>
                    <span>
                      {new Date(job.created_at_epoch * 1000).toLocaleTimeString([], {
                        hour: "2-digit",
                        minute: "2-digit",
                        second: "2-digit",
                      })}
                    </span>
                  </div>
                  {job.error_message && (
                    <div className="job-card-error">⚠️ {job.error_message}</div>
                  )}
                  {job.item_outcomes.length > 0 && (
                    <details className="job-item-outcomes">
                      <summary>Item outcomes ({job.item_outcomes.length})</summary>
                      <ul>
                        {job.item_outcomes.slice(0, 100).map((outcome, index) => (
                          <li key={`${job.id}-${index}`} title={outcome.error_message || undefined}>
                            <strong>{outcome.status}</strong> — {outcome.item_display}
                            {(outcome.actual_destination_display || outcome.requested_destination_display) && (
                              <> → {outcome.actual_destination_display || outcome.requested_destination_display}</>
                            )}
                          </li>
                        ))}
                      </ul>
                      {job.item_outcomes.length > 100 && (
                        <p>Showing first 100 of {job.item_outcomes.length} outcomes.</p>
                      )}
                    </details>
                  )}
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
