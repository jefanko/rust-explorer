# Rust Explorer v0.1.0 Release Notes

**Version:** 0.1.0 (MVP)  
**Release Date:** 2026-10-04  
**Target Platform:** Windows 11 x86-64  
**Architecture:** Rust (MSVC stable 1.99.0, Edition 2024) + Tauri 2.2 + React 18 (TypeScript) + Bundled SQLite FTS5  

---

## 1. Executive Summary

Rust Explorer v0.1.0 is a modern, high-performance, local-first Windows 11 file manager built to provide a fast, safe, and reliable daily file management alternative. It launches alongside Windows File Explorer without modifying global system file associations, shell namespaces, or registry policies.

Every file mutation is governed by an immutable plan, opaque single-use tokens, and executed exclusively through native Windows APIs (`IFileOperation`) on a dedicated COM Single-Threaded Apartment (STA) worker.

---

## 2. Key Features

### Safe Browsing & High-Performance Directory Listing
- **Lossless Path Representation**: Native Win32 wide-string representations preserve lossless Unicode and extended-length path prefixes (`\\?\` and `\\?\UNC\`).
- **Zero-Allocation Natural Sort Comparator**: Custom comparator sorting 100,000 directory entries in ~102 ms without runtime heap allocations.
- **Virtualized Details Table**: Powered by `@tanstack/react-virtual` for buttery-smooth 60 FPS scrolling across directories of arbitrary size with instant column sorting (Name, Type, Size, Date modified).
- **Snapshot Caching**: Subsequent 50-item page requests within a 100k-item directory are served in ~26 microseconds via `sorted_cache`.
- **LRU Snapshot Retention**: Automatically evicts older folder snapshots (capped at 16 snapshots maximum) to eliminate memory leaks during long browsing sessions.

### Multi-Tab Navigation & Persistent State
- **Independent Tab State**: Each tab maintains its own navigation history (Back, Forward, Up), scroll position, selection set, and search query.
- **Full Keyboard Navigation**: Keyboard shortcuts for navigation (`Alt+Arrows`), address bar (`Ctrl+L`), search (`Ctrl+F`), tab management (`Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`), item selection (`Arrows`, `Shift+Arrows`, `Ctrl+A`), rename (`F2`), and new folder (`Ctrl+Shift+N`).
- **Persistent Preferences**: SQLite settings store persists user theme (Light/Dark/System), sidebar Favorites, column widths, and tab restoration between app launches.

### Native Everyday File Operations
- **Dedicated COM STA Engine**: All file operations run in an isolated STA thread, preventing UI lag and async thread pool starvation.
- **Guarded Recycle Bin Deletion**: Deletion requests use `FOFX_RECYCLEONDELETE` and COM `IFileOperationProgressSink`. If recycling is unsupported by the storage volume (e.g. UNC shares), the operation explicitly aborts rather than silently falling back to permanent deletion.
- **Windows Explorer Interoperability**: Full two-way clipboard copy/cut/paste compatibility via standard `CF_HDROP` and `Preferred DropEffect` formats.
- **Job Journal**: Persistent SQLite journal tracking operation lifecycle, item outcomes, and recovering interrupted jobs on app startup.

### Live Filesystem Changes
- **Native OS Event Monitoring**: Native `ReadDirectoryChangesW` watcher keeps visible directories synchronized in real-time.
- **Event Coalescing**: 150 ms debounce coalescing window with 500 ms maximum cap prevents UI flooding during high-volume file mutations.
- **Graceful Overflow Recovery**: Bounded ingress channel (4,096 items) and dirty directory set (1,024 items) automatically escalate to full rescan if buffer limits are reached.

### Indexed Filename Search
- **Local SQLite FTS5 Index**: Fast full-text trigram filename index (`tokenize='trigram'`) stored in `%LOCALAPPDATA%\RustExplorer\index.sqlite3`.
- **Section 14.2 Query Syntax**: Supports terms, `"quoted phrases"`, extension filters (`ext:pdf`), and type filters (`type:folder` / `type:file`) with deterministic ranking (exact > prefix > substring).
- **Instant Query Latency**: Sub-10 ms query responses across 100,000 indexed records.

---

## 3. Performance Benchmarks

| Metric / Scenario | Budget / Target | Observed Result | Status |
|---|---|---|---|
| Startup Working Set Memory | $\le 350\text{ MiB}$ | **31.35 MiB** | PASS |
| 100k Directory Generation | -- | **43.1 ms** | PASS |
| 100k Snapshot Creation | -- | **45.8 ms** | PASS |
| 100k Natural Sort + Page 1 | $\le 150\text{ ms}$ | **103.2 ms** | PASS |
| Subsequent 50-Item Page Slice | $\le 5\text{ ms}$ | **26.0 µs** (0.026 ms) | PASS |
| Item Token Resolution | $\le 50\text{ µs}$ | **200 ns** | PASS |
| 100k SQLite FTS5 Indexing Rate | Baseline $\ge 1,000\text{/s}$ | **6,888 entries/s** | PASS |
| 100k SQLite Database Disk Footprint | $\le 350\text{ MiB}$ | **67.93 MiB** | PASS |
| 100k Trigram Query Latency | $\le 50\text{ ms}$ | **6.86 ms** | PASS |

---

## 4. Security & Privacy Audit

- **100% Local-First**: Zero external network requests, zero analytics, zero telemetry.
- **Strict Content Security Policy (CSP)**: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost`.
- **Capability Isolation**: Main window permissions restricted to `"core:default"`. Arbitrary process execution via `tauri-plugin-shell` is denied to the webview.
- **Standard User Execution**: Requires no administrative elevation (`asInvoker`).

---

## 5. Distribution Bundles

| Bundle Name | Type | Size | SHA-256 Checksum |
|---|---|---|---|
| `rust-explorer_0.1.0_x64-setup.exe` | NSIS Per-User Installer | 3.55 MiB | `5D4070B6EDD02174F88D44AB7103DFB5DBE6A6D503B4598A979C73267510F319` |
| `rust-explorer_0.1.0_x64_en-US.msi` | Windows Installer Package | 5.15 MiB | `366B69E11CF996C112E7BEF08E43C00EFD5F8FC5506F0677311EA14C61D71DAD` |
| `rust-explorer.exe` | Standalone Executable | 14.43 MiB | `F8E167C7B81AFA36E5A2E0AF5F2B81222C5223F8E96051B23846656386F159B5` |

---

## 6. Installation & Removal

- **Installation**: Run `rust-explorer_0.1.0_x64-setup.exe`. It installs into `%LOCALAPPDATA%\Programs\rust-explorer` and creates a Start Menu shortcut without requesting administrative elevation.
- **Uninstallation**: Uninstall via Windows Settings -> Installed Apps. Removes all program files and shortcuts cleanly while preserving user settings unless explicitly cleared.
