import React, { useEffect, useRef, useState, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { client } from "../bridge/client";
import type {
  DriveItem,
  AppSettings,
  FileEntry,
  JobSummary,
  KnownFolderItem,
  SortColumn,
  SortDirection,
  IndexedRoot,
  SearchResultItem,
  PreviewData,
} from "../bridge/types";
import { formatBytes, formatFiletime } from "../lib/format";
import { matchFilterQuery } from "../lib/filter";
import {
  getTabTitle,
  getParentPath,
  sameWindowsPath,
} from "../lib/paths";
import { moveSelection, nextFocusIndex } from "../lib/selection";
import { validateIndexedQuery } from "../lib/searchQuery";
import { recycleDialogText } from "../lib/recycle";
import { TabState, createInitialTab } from "../state/tabs";
import type { ContextMenuState, ModalState, ThemeMode } from "./types";
import { FluentIcon } from "../components/FluentIcon";
import { getFileIcon } from "../components/icons";
import { TabBar } from "../components/TabBar";
import { NavToolbar } from "../components/NavToolbar";
import { CommandBar } from "../components/CommandBar";
import { Sidebar } from "../components/Sidebar";
import { StatusBar } from "../components/StatusBar";
import { ContextMenu } from "../components/ContextMenu";
import { ModalDialog } from "../components/ModalDialog";
import { JobsDrawer } from "../components/JobsDrawer";
import { PreviewPane } from "../components/PreviewPane";

// Re-export pure helpers for backwards compatibility
export {
  matchFilterQuery,
} from "../lib/filter";
export {
  getBreadcrumbs,
  getParentPathUtf16,
  getParentPathString,
  getParentPath,
} from "../lib/paths";

export default function App() {
  const [knownFolders, setKnownFolders] = useState<KnownFolderItem[]>([]);
  const [drives, setDrives] = useState<DriveItem[]>([]);
  const [favorites, setFavorites] = useState<string[]>([]);
  const [theme, setTheme] = useState<ThemeMode>("system");
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
  const [isEditingAddress, setIsEditingAddress] = useState<boolean>(false);

  // Indexed roots state
  const [indexedRoots, setIndexedRoots] = useState<IndexedRoot[]>([]);

  // Preview Pane state
  const [showPreviewPane, setShowPreviewPane] = useState<boolean>(false);
  const [previewData, setPreviewData] = useState<PreviewData | null>(null);
  const [previewLoading, setPreviewLoading] = useState<boolean>(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  // Drag and drop state
  const [draggedTokens, setDraggedTokens] = useState<string[]>([]);
  const [dragSourceFolderToken, setDragSourceFolderToken] = useState<string>("");
  const [dropTargetToken, setDropTargetToken] = useState<string | null>(null);

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
  const canGoUp = Boolean(getParentPath(activeTab?.nativePathUtf16, activeTab?.path));
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

  // Single selected entry for preview pane
  const selectedEntry = useMemo(() => {
    if (activeTab.selectedTokens.size !== 1) return null;
    const token = Array.from(activeTab.selectedTokens)[0];
    return displayedEntries.find((e) => e.token === token) || null;
  }, [activeTab.selectedTokens, displayedEntries]);

  // Load preview data whenever selected entry changes and preview is enabled
  useEffect(() => {
    if (!showPreviewPane || !selectedEntry || !activeTab.folderToken) {
      setPreviewData(null);
      setPreviewLoading(false);
      setPreviewError(null);
      return;
    }

    let canceled = false;
    setPreviewLoading(true);
    setPreviewError(null);

    client
      .readPreview(activeTab.folderToken, selectedEntry.token)
      .then((data) => {
        if (!canceled) {
          setPreviewData(data);
          setPreviewLoading(false);
        }
      })
      .catch((err: any) => {
        if (!canceled) {
          setPreviewError(err?.user_message || "Failed to load preview");
          setPreviewLoading(false);
        }
      });

    return () => {
      canceled = true;
    };
  }, [showPreviewPane, selectedEntry?.token, activeTab.folderToken]);

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
  function applyTheme(t: ThemeMode) {
    if (t === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", t);
    }
  }

  function toggleTheme() {
    const nextTheme: ThemeMode =
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
    const prev = indexedRoots;
    setIndexedRoots((curr) => curr.filter((r) => r.id !== rootId));
    try {
      await client.removeIndexedRoot(rootId);
      await loadIndexedRoots();
    } catch (err: any) {
      console.error("Failed to remove indexed root:", err);
      setIndexedRoots(prev);
      alert(err?.user_message || err?.message || "Failed to remove indexed root");
    }
  }

  async function handleRecrawlRoot(rootId: string) {
    setIndexedRoots((curr) =>
      curr.map((r) => (r.id === rootId ? { ...r, state: "scanning" as const } : r))
    );
    try {
      await client.recrawlIndexedRoot(rootId);
      await loadIndexedRoots();
    } catch (err: any) {
      console.error("Failed to recrawl indexed root:", err);
      await loadIndexedRoots();
      alert(err?.user_message || err?.message || "Failed to recrawl indexed root");
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

  useEffect(() => {
    if (!isAnyRootScanning) return;
    const interval = setInterval(loadIndexedRoots, 1200);
    return () => clearInterval(interval);
  }, [isAnyRootScanning]);

  // Debounced indexed search with query validation
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

    const validation = validateIndexedQuery(q);
    if (!validation.valid) {
      setSearchResults([]);
      setSearchTotalMatches(0);
      setSearchIsCapped(false);
      setSearchLoading(false);
      setSearchError(validation.error || null);
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

  // Save tabs for restoration
  useEffect(() => {
    const paths = tabs.map((t) => t.path).filter(Boolean);
    if (paths.length > 0) {
      void persistSettings((settings) =>
        settings.restore_tabs ? { saved_tabs: paths } : null
      );
    }
  }, [tabs]);

  // Listen for live filesystem notifications
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
    const parent = getParentPath(cur.nativePathUtf16, cur.path);
    if (!parent) return;
    if (parent.utf16) {
      navigateToPath(parent.utf16);
    } else if (parent.display) {
      navigateToPath(parent.display);
    }
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

  // Row selection handler
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
      const move = moveSelection(displayedEntries, cur.anchorIndex, index, true);
      if (move) {
        updateActiveTab(move);
      }
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
    const target = nextFocusIndex(cur.focusedIndex, delta, displayedEntries.length);
    const move = moveSelection(displayedEntries, cur.anchorIndex, target, isShift);
    if (move) {
      updateActiveTab(move);
      rowVirtualizer.scrollToIndex(move.focusedIndex, { align: "auto" });
    }
  }

  function jumpToRow(index: number, isShift: boolean) {
    if (displayedEntries.length === 0) return;
    const cur = activeTabRef.current;
    const move = moveSelection(displayedEntries, cur.anchorIndex, index, isShift);
    if (move) {
      updateActiveTab(move);
      rowVirtualizer.scrollToIndex(move.focusedIndex, { align: "auto" });
    }
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
    const dialogInfo = recycleDialogText(targetEntries.map((e) => e.display_name));
    setModal({
      type: "recycle",
      title: dialogInfo.title,
      value: dialogInfo.value,
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

  // Drag and Drop operation handler
  async function handleTransfer(
    destinationFolderToken: string,
    destinationItemToken: string | null = null,
    isMove = true
  ) {
    if (!dragSourceFolderToken || draggedTokens.length === 0) return;
    try {
      const plan = await client.planTransfer(
        dragSourceFolderToken,
        draggedTokens,
        destinationFolderToken,
        destinationItemToken,
        isMove
      );
      showJobResult(await client.commitPlan(plan.id));
      refresh();
      refreshJobs();
    } catch (err: any) {
      refreshJobs();
      alert(err?.user_message || "Transfer operation failed");
    } finally {
      setDraggedTokens([]);
      setDragSourceFolderToken("");
      setDropTargetToken(null);
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

      // Ctrl+P / Alt+P -> Toggle Preview Pane
      if ((e.ctrlKey && (e.key === "p" || e.key === "P")) || (e.altKey && (e.key === "p" || e.key === "P"))) {
        e.preventDefault();
        setShowPreviewPane((prev) => !prev);
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
      // Ctrl+L / Alt+D -> Focus address bar
      if ((e.ctrlKey && (e.key === "l" || e.key === "L")) || (e.altKey && (e.key === "d" || e.key === "D"))) {
        e.preventDefault();
        setIsEditingAddress(true);
        setTimeout(() => {
          addressInputRef.current?.focus();
          addressInputRef.current?.select();
        }, 50);
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
      {/* Tab bar, Navigation Toolbar, and Command Bar */}
      <header className="app-header">
        <TabBar
          tabs={tabs}
          activeTabIndex={activeTabIndex}
          onSwitchTab={switchTab}
          onCloseTab={closeTab}
          onNewTab={() => createNewTab()}
        />

        <NavToolbar
          activeTab={activeTab}
          canGoUp={canGoUp}
          onGoBack={goBack}
          onGoForward={goForward}
          onGoUp={goUp}
          onNavigate={(path) => navigateToPath(path)}
          onRefresh={refresh}
          isEditingAddress={isEditingAddress}
          setIsEditingAddress={setIsEditingAddress}
          addressInputRef={addressInputRef}
          filterInputRef={filterInputRef}
          onAddressInputChange={(val) => updateActiveTab({ addressInput: val })}
          searchScope={searchScope}
          onSetSearchScope={setSearchScope}
          indexedSearchQuery={indexedSearchQuery}
          onIndexedSearchQueryChange={setIndexedSearchQuery}
          onFilterQueryChange={(val) => updateActiveTab({ filterQuery: val })}
          onSearchSubfolders={handleSearchSubfolders}
          searchLoading={searchLoading}
        />

        <CommandBar
          selectedCount={activeTab.selectedTokens.size}
          showHiddenFiles={showHiddenFiles}
          showPreviewPane={showPreviewPane}
          showJobsDrawer={showJobsDrawer}
          theme={theme}
          jobs={jobs}
          onCreateFolder={openCreateFolderModal}
          onCut={handleCut}
          onCopy={handleCopy}
          onPaste={handlePaste}
          onRename={() => openRenameModal()}
          onRecycle={() => openRecycleModal()}
          onToggleHiddenFiles={toggleHiddenFiles}
          onTogglePreviewPane={() => setShowPreviewPane((prev) => !prev)}
          onToggleJobsDrawer={() => {
            setShowJobsDrawer(!showJobsDrawer);
            refreshJobs();
          }}
          onToggleTheme={toggleTheme}
        />
      </header>

      {/* Main Body */}
      <div className="app-body">
        <Sidebar
          favorites={favorites}
          knownFolders={knownFolders}
          drives={drives}
          indexedRoots={indexedRoots}
          activePath={activeTab.path}
          hasActiveFolderToken={Boolean(activeTab.folderToken)}
          onNavigate={(path) => navigateToPath(path)}
          onRemoveFavorite={handleRemoveFavorite}
          onIndexCurrentFolder={handleIndexCurrentFolder}
          onRecrawlRoot={handleRecrawlRoot}
          onRemoveRoot={handleRemoveRoot}
        />

        {/* Content Pane */}
        <main
          className="content-pane"
          role="region"
          aria-label="Folder contents"
          onDragOver={(e) => {
            if (draggedTokens.length > 0) {
              e.preventDefault();
              e.dataTransfer.dropEffect = e.ctrlKey ? "copy" : "move";
            }
          }}
          onDrop={(e) => {
            if (draggedTokens.length > 0 && activeTab.folderToken) {
              e.preventDefault();
              handleTransfer(activeTab.folderToken, null, !e.ctrlKey);
            }
          }}
        >
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
                    <p style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                      <FluentIcon name="warning" size={14} />
                      <span>{searchError}</span>
                    </p>
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
                    <FluentIcon name="refresh" size={14} className="search-spinner" />
                    <span>Indexing in progress... Results update live as files are scanned.</span>
                  </div>
                )}

                {searchLoading && searchResults.length === 0 ? (
                  <div className="empty-state">
                    <p>Searching indexed files...</p>
                  </div>
                ) : searchResults.length === 0 ? (
                  <div className="empty-state">
                    {isAnyRootScanning ? (
                      <p style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                        <FluentIcon name="refresh" size={14} className="search-spinner" />
                        <span>Indexing folder in progress... Matching files will appear as they are scanned.</span>
                      </p>
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
                            <span className="col-path-text">{item.path}</span>
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
                  Name{" "}
                  {activeTab.sortColumn === "name" && (
                    <FluentIcon
                      name={activeTab.sortDirection === "ascending" ? "sort-asc" : "sort-desc"}
                      size={12}
                      className="sort-indicator"
                    />
                  )}
                </span>
                <span
                  className={`col col-type sortable ${activeTab.sortColumn === "type" ? "sorted" : ""}`}
                  onClick={() => handleSort("type")}
                >
                  Type{" "}
                  {activeTab.sortColumn === "type" && (
                    <FluentIcon
                      name={activeTab.sortDirection === "ascending" ? "sort-asc" : "sort-desc"}
                      size={12}
                      className="sort-indicator"
                    />
                  )}
                </span>
                <span
                  className={`col col-size sortable ${activeTab.sortColumn === "size" ? "sorted" : ""}`}
                  onClick={() => handleSort("size")}
                >
                  Size{" "}
                  {activeTab.sortColumn === "size" && (
                    <FluentIcon
                      name={activeTab.sortDirection === "ascending" ? "sort-asc" : "sort-desc"}
                      size={12}
                      className="sort-indicator"
                    />
                  )}
                </span>
                <span
                  className={`col col-date sortable ${activeTab.sortColumn === "modified" ? "sorted" : ""}`}
                  onClick={() => handleSort("modified")}
                >
                  Date modified{" "}
                  {activeTab.sortColumn === "modified" && (
                    <FluentIcon
                      name={activeTab.sortDirection === "ascending" ? "sort-asc" : "sort-desc"}
                      size={12}
                      className="sort-indicator"
                    />
                  )}
                </span>
              </div>

              <div
                className="file-table-body"
                ref={parentRef}
                onContextMenu={handleBackgroundContextMenu}
              >
                {activeTab.error && (
                  <div className="error-banner">
                    <p style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                      <FluentIcon name="warning" size={14} />
                      <span>{activeTab.error}</span>
                    </p>
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
                            display: "inline-flex",
                            alignItems: "center",
                            gap: "6px",
                          }}
                          onClick={() => handleSearchSubfolders()}
                        >
                          <FluentIcon name="search" size={14} />
                          <span>Search inside subfolders for "{activeTab.filterQuery}"</span>
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
                      const isDropTarget = dropTargetToken === entry.token;

                      return (
                        <div
                          key={entry.token}
                          className={`file-row ${isSelected ? "selected" : ""} ${isDropTarget ? "drop-target" : ""}`}
                          role="option"
                          aria-selected={isSelected}
                          tabIndex={
                            activeTab.focusedIndex === virtualRow.index ||
                            (activeTab.focusedIndex < 0 && virtualRow.index === 0)
                              ? 0
                              : -1
                          }
                          draggable={true}
                          onDragStart={(e) => {
                            const tokens = isSelected
                              ? Array.from(activeTab.selectedTokens)
                              : [entry.token];
                            setDraggedTokens(tokens);
                            setDragSourceFolderToken(activeTab.folderToken);
                            e.dataTransfer.setData("text/plain", entry.display_name);
                            e.dataTransfer.effectAllowed = "copyMove";
                          }}
                          onDragEnd={() => {
                            setDraggedTokens([]);
                            setDragSourceFolderToken("");
                            setDropTargetToken(null);
                          }}
                          onDragOver={(e) => {
                            if (entry.kind === "directory" && draggedTokens.length > 0 && !draggedTokens.includes(entry.token)) {
                              e.preventDefault();
                              e.stopPropagation();
                              e.dataTransfer.dropEffect = e.ctrlKey ? "copy" : "move";
                              if (dropTargetToken !== entry.token) {
                                setDropTargetToken(entry.token);
                              }
                            }
                          }}
                          onDragLeave={() => {
                            if (dropTargetToken === entry.token) {
                              setDropTargetToken(null);
                            }
                          }}
                          onDrop={(e) => {
                            if (entry.kind === "directory" && draggedTokens.length > 0) {
                              e.preventDefault();
                              e.stopPropagation();
                              handleTransfer(activeTab.folderToken, entry.token, !e.ctrlKey);
                            }
                          }}
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

        {/* File Preview Pane */}
        {showPreviewPane && (
          <PreviewPane
            entry={selectedEntry}
            preview={previewData}
            loading={previewLoading}
            error={previewError}
            onClose={() => setShowPreviewPane(false)}
          />
        )}
      </div>

      {/* Status Bar */}
      <StatusBar
        isSearchActive={isSearchActive}
        searchTotalMatches={searchTotalMatches}
        searchIsCapped={searchIsCapped}
        selectedSearchIndex={selectedSearchIndex}
        displayedEntriesCount={displayedEntries.length}
        visibleEntryCount={visibleEntryCount}
        filterQuery={activeTab.filterQuery}
        selectedCount={activeTab.selectedTokens.size}
        activePath={activeTab.path}
        jobs={jobs}
        showJobsDrawer={showJobsDrawer}
        onToggleJobsDrawer={() => {
          setShowJobsDrawer(!showJobsDrawer);
          refreshJobs();
        }}
      />

      {/* Context Menu */}
      {contextMenu && (
        <ContextMenu
          contextMenu={contextMenu}
          activePath={activeTab.path}
          onClose={() => setContextMenu(null)}
          onOpenEntry={handleItemDoubleClick}
          onOpenSearchResult={handleSearchResultDoubleClick}
          onOpenContainingFolder={(item) => {
            const parent = getParentPath(item.path_utf16, item.path);
            if (parent) {
              if (parent.utf16) navigateToPath(parent.utf16);
              else if (parent.display) navigateToPath(parent.display);
            }
          }}
          onCopySearchResultPath={(item) => client.clipboardWritePath(item.path_utf16)}
          onCut={handleCut}
          onCopy={handleCopy}
          onPaste={handlePaste}
          onRefresh={refresh}
          onOpenRenameModal={openRenameModal}
          onOpenRecycleModal={openRecycleModal}
          onOpenCreateFolderModal={openCreateFolderModal}
          onOpenInExplorer={(token) => client.openInExplorer(activeTab.folderToken, token)}
          onAddFavorite={handleAddFavorite}
          onShowProperties={(token) => client.showProperties(activeTab.folderToken, token)}
        />
      )}

      {/* Modal Dialog */}
      {modal && (
        <ModalDialog
          modal={modal}
          modalInputRef={modalInputRef}
          onClose={() => setModal(null)}
          onSubmit={handleModalSubmit}
          onValueChange={(val) => setModal({ ...modal, value: val, error: null })}
        />
      )}

      {/* Jobs Drawer */}
      {showJobsDrawer && (
        <JobsDrawer
          jobs={jobs}
          onClose={() => setShowJobsDrawer(false)}
        />
      )}
    </div>
  );
}
