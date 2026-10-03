import { useEffect, useRef, useState, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { client } from "../bridge/client";
import {
  DriveItem,
  FileEntry,
  KnownFolderItem,
  SortColumn,
  SortDirection,
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
  // Win32 FILETIME epoch starts Jan 1, 1601 UTC.
  // Unix epoch starts Jan 1, 1970 UTC.
  // Difference in 100-ns intervals: 116444736000000000
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

function getFileIcon(entry: FileEntry) {
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

export default function App() {
  const [knownFolders, setKnownFolders] = useState<KnownFolderItem[]>([]);
  const [drives, setDrives] = useState<DriveItem[]>([]);
  const [currentPath, setCurrentPath] = useState<string>("");
  const [addressInput, setAddressInput] = useState<string>("");
  const [folderToken, setFolderToken] = useState<string>("");
  const [generation, setGeneration] = useState<number>(0);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [totalEntries, setTotalEntries] = useState<number>(0);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Navigation History
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);

  // Sorting
  const [sortColumn, setSortColumn] = useState<SortColumn>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("ascending");

  // Selection
  const [selectedTokens, setSelectedTokens] = useState<Set<string>>(new Set());

  // Search/Filter in current folder
  const [filterQuery, setFilterQuery] = useState<string>("");

  // Virtualization parent ref
  const parentRef = useRef<HTMLDivElement>(null);

  // Filtered entries
  const displayedEntries = useMemo(() => {
    if (!filterQuery.trim()) return entries;
    const q = filterQuery.toLowerCase();
    return entries.filter((e) => e.display_name.toLowerCase().includes(q));
  }, [entries, filterQuery]);

  const rowVirtualizer = useVirtualizer({
    count: displayedEntries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 20,
  });

  // Initial bootstrap
  useEffect(() => {
    let mounted = true;
    client
      .bootstrap()
      .then((data) => {
        if (!mounted) return;
        setKnownFolders(data.known_folders);
        setDrives(data.drives);
        navigateToPath(data.initial_path, true);
      })
      .catch((err) => {
        if (!mounted) return;
        setError(typeof err === "string" ? err : err.user_message || "Failed to initialize");
        setLoading(false);
      });

    return () => {
      mounted = false;
    };
  }, []);

  // Fetch directory listing
  async function loadDirectory(token: string, gen: number, col = sortColumn, dir = sortDirection) {
    setLoading(true);
    setError(null);
    try {
      // Load all entries in one or paged requests
      const page = await client.listPage(token, gen, 0, 5000, col, dir);
      setEntries(page.entries);
      setTotalEntries(page.total_entries);
      setSelectedTokens(new Set());
    } catch (err: any) {
      setError(err?.user_message || "Failed to load directory items");
    } finally {
      setLoading(false);
    }
  }

  // Navigate to path
  async function navigateToPath(path: string, pushHistory = true) {
    if (!path.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const nav = await client.navigate(path);
      setCurrentPath(nav.path_display);
      setAddressInput(nav.path_display);
      setFolderToken(nav.folder_token);
      setGeneration(nav.generation);

      if (pushHistory) {
        const nextIndex = historyIndex + 1;
        const newHist = history.slice(0, nextIndex);
        newHist.push(nav.path_display);
        setHistory(newHist);
        setHistoryIndex(nextIndex);
      }

      await loadDirectory(nav.folder_token, nav.generation);
    } catch (err: any) {
      setError(err?.user_message || `Cannot access ${path}`);
      setLoading(false);
    }
  }

  function goBack() {
    if (historyIndex > 0) {
      const prevIndex = historyIndex - 1;
      setHistoryIndex(prevIndex);
      navigateToPath(history[prevIndex], false);
    }
  }

  function goForward() {
    if (historyIndex < history.length - 1) {
      const nextIndex = historyIndex + 1;
      setHistoryIndex(nextIndex);
      navigateToPath(history[nextIndex], false);
    }
  }

  function goUp() {
    if (!currentPath) return;
    const trimmed = currentPath.replace(/\\$/, "");
    const lastSlash = trimmed.lastIndexOf("\\");
    if (lastSlash > 0) {
      const parent = trimmed.substring(0, lastSlash + 1);
      navigateToPath(parent);
    } else if (lastSlash === 0) {
      // Root like C:\
      navigateToPath(trimmed + "\\");
    }
  }

  function refresh() {
    if (!folderToken) return;
    setLoading(true);
    client
      .refresh(folderToken)
      .then((nav) => {
        setFolderToken(nav.folder_token);
        setGeneration(nav.generation);
        return loadDirectory(nav.folder_token, nav.generation);
      })
      .catch((err: any) => {
        setError(err?.user_message || "Failed to refresh directory");
        setLoading(false);
      });
  }

  function handleSort(col: SortColumn) {
    let nextDir: SortDirection = "ascending";
    if (sortColumn === col) {
      nextDir = sortDirection === "ascending" ? "descending" : "ascending";
    }
    setSortColumn(col);
    setSortDirection(nextDir);
    if (folderToken && generation) {
      loadDirectory(folderToken, generation, col, nextDir);
    }
  }

  async function handleItemDoubleClick(entry: FileEntry) {
    if (!folderToken) return;
    try {
      const nav = await client.openItem(folderToken, entry.token);
      if (nav) {
        // Navigated into folder
        setCurrentPath(nav.path_display);
        setAddressInput(nav.path_display);
        setFolderToken(nav.folder_token);
        setGeneration(nav.generation);

        const nextIndex = historyIndex + 1;
        const newHist = history.slice(0, nextIndex);
        newHist.push(nav.path_display);
        setHistory(newHist);
        setHistoryIndex(nextIndex);

        await loadDirectory(nav.folder_token, nav.generation);
      }
    } catch (err: any) {
      setError(err?.user_message || "Failed to open item");
    }
  }

  function handleRowClick(token: string, e: React.MouseEvent) {
    if (e.ctrlKey) {
      const next = new Set(selectedTokens);
      if (next.has(token)) next.delete(token);
      else next.add(token);
      setSelectedTokens(next);
    } else {
      setSelectedTokens(new Set([token]));
    }
  }

  return (
    <div className="app-container">
      {/* Tab bar and Navigation Toolbar */}
      <header className="app-header">
        <div className="tab-bar" role="tablist">
          <div className="tab-item active" role="tab" aria-selected="true">
            <span>{currentPath.split("\\").filter(Boolean).pop() || "PC"}</span>
          </div>
          <button className="new-tab-btn" aria-label="New tab" title="New Tab (Ctrl+T)">
            +
          </button>
        </div>

        <div className="nav-toolbar">
          <div className="nav-buttons">
            <button
              onClick={goBack}
              disabled={historyIndex <= 0}
              aria-label="Back"
              title="Back (Alt+Left)"
            >
              ←
            </button>
            <button
              onClick={goForward}
              disabled={historyIndex >= history.length - 1}
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

          <form
            className="address-bar-container"
            onSubmit={(e) => {
              e.preventDefault();
              navigateToPath(addressInput);
            }}
          >
            <input
              type="text"
              className="address-input"
              value={addressInput}
              onChange={(e) => setAddressInput(e.target.value)}
              aria-label="Address"
              placeholder="Enter a path..."
            />
          </form>

          <div className="search-container">
            <input
              type="text"
              className="search-input"
              placeholder="Filter current folder (Ctrl+F)"
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
              aria-label="Filter"
            />
          </div>
        </div>
      </header>

      {/* Main Body */}
      <div className="app-body">
        {/* Sidebar */}
        <aside className="sidebar">
          <section className="sidebar-section">
            <h3>Known Folders</h3>
            <ul>
              {knownFolders.map((kf) => (
                <li
                  key={kf.id}
                  onClick={() => navigateToPath(kf.path)}
                  className={currentPath === kf.path ? "active" : ""}
                >
                  <span className="sidebar-icon">📁</span>
                  <span className="sidebar-label">{kf.name}</span>
                </li>
              ))}
            </ul>
          </section>

          <section className="sidebar-section">
            <h3>Drives</h3>
            <ul>
              {drives.map((d) => (
                <li
                  key={d.path}
                  onClick={() => navigateToPath(d.path)}
                  className={currentPath === d.path ? "active" : ""}
                >
                  <span className="sidebar-icon">💾</span>
                  <span className="sidebar-label">{d.name}</span>
                </li>
              ))}
            </ul>
          </section>
        </aside>

        {/* Content Pane */}
        <main className="content-pane" role="region" aria-label="Folder contents">
          <div className="file-table-header">
            <span
              className={`col col-name sortable ${sortColumn === "name" ? "sorted" : ""}`}
              onClick={() => handleSort("name")}
            >
              Name {sortColumn === "name" && (sortDirection === "ascending" ? "▲" : "▼")}
            </span>
            <span
              className={`col col-type sortable ${sortColumn === "type" ? "sorted" : ""}`}
              onClick={() => handleSort("type")}
            >
              Type {sortColumn === "type" && (sortDirection === "ascending" ? "▲" : "▼")}
            </span>
            <span
              className={`col col-size sortable ${sortColumn === "size" ? "sorted" : ""}`}
              onClick={() => handleSort("size")}
            >
              Size {sortColumn === "size" && (sortDirection === "ascending" ? "▲" : "▼")}
            </span>
            <span
              className={`col col-date sortable ${sortColumn === "modified" ? "sorted" : ""}`}
              onClick={() => handleSort("modified")}
            >
              Date modified {sortColumn === "modified" && (sortDirection === "ascending" ? "▲" : "▼")}
            </span>
          </div>

          <div className="file-table-body" ref={parentRef}>
            {error && (
              <div className="error-banner">
                <p>⚠️ {error}</p>
              </div>
            )}

            {loading && entries.length === 0 ? (
              <div className="empty-state">
                <p>Loading folder contents...</p>
              </div>
            ) : displayedEntries.length === 0 ? (
              <div className="empty-state">
                <p>This folder is empty.</p>
              </div>
            ) : (
              <div
                style={{
                  height: `${rowVirtualizer.getTotalSize()}px`,
                  width: "100%",
                  position: "relative",
                }}
              >
                {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                  const entry = displayedEntries[virtualRow.index];
                  const isSelected = selectedTokens.has(entry.token);

                  return (
                    <div
                      key={entry.token}
                      className={`file-row ${isSelected ? "selected" : ""}`}
                      onClick={(e) => handleRowClick(entry.token, e)}
                      onDoubleClick={() => handleItemDoubleClick(entry)}
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
                        {entry.kind === "directory" ? "File folder" : entry.extension ? `${entry.extension.toUpperCase()} File` : "File"}
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
        </main>
      </div>

      {/* Status Bar */}
      <footer className="app-statusbar">
        <span>
          {displayedEntries.length} {displayedEntries.length === 1 ? "item" : "items"}
          {filterQuery && ` (filtered from ${totalEntries})`}
          {selectedTokens.size > 0 && ` | ${selectedTokens.size} selected`}
        </span>
        <span className="status-spacer"></span>
        <span>{currentPath}</span>
      </footer>
    </div>
  );
}
