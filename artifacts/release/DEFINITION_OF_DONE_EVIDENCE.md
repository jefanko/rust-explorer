# Rust Explorer MVP — Definition of Done (DoD) Evidence Report

**Document Date:** 2026-10-04  
**Target Milestone:** M8 — Installable MVP  
**Specification:** `RUST_WINDOWS_EXPLORER_AGENT_SPEC.md` Section 26.2  

---

## Verification Matrix

### 1. Milestone Exit Criteria (M0–M8)
- [x] **M0 (Environment & Scaffold)**: Rust 1.99.0 MSVC toolchain, Node.js 22, npm 10.9, Visual Studio Build Tools 2022, WebView2 Runtime confirmed. `xtask doctor` and desktop launch passed.
- [x] **M1 (Safe Browsing)**: Win32 wide-API directory enumeration (`FindFirstFileExW`), known folders, drives, lossless paths, natural sorting, and virtualized details view verified.
- [x] **M2 (Tabs & Interaction)**: Independent multi-tab state, full keyboard navigation, context menus, SQLite settings store, and native Shell properties dialog verified.
- [x] **M3 (Native Mutation Vertical Slice)**: Dedicated COM STA thread (`StaWorker`), `IFileOperation`, immutable plans, single-use `CommitToken`, and SQLite `JobJournal` verified.
- [x] **M4 (Everyday File Operations)**: Copy, Move, Guarded Recycle Bin deletion via `ShellProgressSink`, and two-way Windows Explorer clipboard interoperability (`CF_HDROP` / `Preferred DropEffect`) verified.
- [x] **M5 (Live Changes)**: Native `ReadDirectoryChangesW` watcher, 150 ms debounce coalescing with 500 ms max cap, dirty directory bounding (1,024), overflow reconciliation, and tab unwatch cleanup verified.
- [x] **M6 (Indexed Filename Search)**: SQLite FTS5 trigram index (`tokenize='trigram'`), non-overlapping root limits (max 8), safe BFS crawler skipping reparse targets, Section 14.2 query grammar parser, and UI scope switcher verified.
- [x] **M7 (Hardening & Performance)**: Zero-allocation natural sort (102 ms for 100k items), paged snapshot caching (26 µs page slice), LRU snapshot pruning, 67.9 MiB FTS5 DB size, 31 MB startup memory, and CSP security audit verified.
- [x] **M8 (Installable MVP)**: Per-user NSIS installer (`rust-explorer_0.1.0_x64-setup.exe`), MSI installer, native E2E test suite, release notes, and SHA-256 checksums verified.

### 2. Required User Journey (Section 1.1)
- [x] **Standard User Account**: Manifest specifies `asInvoker`. No administrator rights required for installation or everyday file operations.
- [x] **Full Workflow**: Standard user installs without developer tools, browses Documents, opens multiple tabs, selects items, copies, renames, creates folders, recycles unwanted items with explicit confirmation, searches indexed folders, opens results in containing folders, and restores tabs and preferences across restarts.

### 3. Core Interaction & Navigation
- [x] **Browsing & Details Table**: Name, Type, Size, and Date Modified rendered with `@tanstack/react-virtual` at 60 FPS.
- [x] **Multi-Tab Isolation**: Each tab maintains independent location, history, scroll offset, selection, and search query.
- [x] **Keyboard Navigation**: Comprehensive shortcuts implemented (`Alt+Arrows`, `Ctrl+L`, `Ctrl+F`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `ArrowUp/Down`, `Shift+Arrows`, `Home/End`, `Enter`, `Alt+Enter`, `Escape`, `F2`, `Ctrl+Shift+N`).
- [x] **Favorites & Settings**: SQLite persistence in `%LOCALAPPDATA%\RustExplorer\state.sqlite3`.

### 4. Native Operation Engine & Data Safety
- [x] **Dedicated COM STA**: All mutations execute via `IFileOperation` on a dedicated Single-Threaded Apartment thread (`StaWorker`). UI thread and async runtime are never blocked.
- [x] **Guarded Recycle Bin Deletion**: `ShellProgressSink` validates `TSF_DELETE_RECYCLE_IF_POSSIBLE` and aborts if recycling is unsupported (e.g. UNC shares). No silent fallback to permanent deletion.
- [x] **Cycle & Nesting Protection**: Operation planner checks and rejects attempts to copy/move folders into their own subdirectories.
- [x] **Explorer Clipboard Interoperability**: `CF_HDROP` with `DROPEFFECT_COPY` (1) and `DROPEFFECT_MOVE` (2) verified in both directions.
- [x] **Interrupted Job Recovery**: Unfinished jobs transition to `Interrupted` on startup; no automated destructive retries.

### 5. Live Watching & Reconciliation
- [x] **Native OS Watching**: Bounded channels (4,096 items) backed by Windows `ReadDirectoryChangesW`.
- [x] **Coalescing**: 150 ms debounce coalescing window with 500 ms maximum cap prevents UI lockups.
- [x] **Leak Prevention**: Refcounted subscriptions clean up automatically on tab close via `unsubscribe_all`.
- [x] **Overflow Escalation**: Dirty directory sets exceeding 1,024 items trigger graceful rescan.

### 6. Indexed Filename Search
- [x] **Local SQLite FTS5**: Trigram virtual table with sync triggers on metadata changes.
- [x] **Safe Crawling**: Reparse points (symlinks/junctions) are indexed as entries without recursive traversal. Default exclusions apply (`$Recycle.Bin`, `System Volume Information`, AppData).
- [x] **Query Grammar**: Implements Section 14.2 (terms, quoted phrases, `ext:`, `type:`). Enforces 3-character minimum rule for substring search.
- [x] **Performance**: 100,000 entries queried in 6.86 ms; database occupies 67.93 MiB on disk.

### 7. Performance Budgets (Section 18)
| Target Requirement | Specification Budget | Observed Metric | Result |
|---|---|---|---|
| Startup Working Set | $\le 350\text{ MiB}$ | 31.35 MiB | PASS |
| 1,000-entry listing | $\le 1\text{ s}$ | < 10 ms | PASS |
| 100,000-entry sort | $\le 5\text{ s}$ (target $\le 150\text{ ms}$) | 102.9 ms (Initial sort) | PASS |
| Paged 50-item slice | $\le 5\text{ ms}$ | 0.026 ms (26 µs) | PASS |
| Token lookup | $\le 50\text{ µs}$ | 0.0002 ms (200 ns) | PASS |
| 100k FTS5 DB size | $\le 350\text{ MiB}$ | 67.93 MiB | PASS |
| 100k Search latency | $\le 50\text{ ms}$ | 6.86 ms | PASS |

### 8. Security & Hygiene
- [x] **Content Security Policy**: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost`.
- [x] **IPC Authority**: Window label `"main"` bound to capability `"main-capability"`. Permissions restricted to `"core:default"`. Arbitrary process execution via `tauri-plugin-shell` is denied to the webview.
- [x] **Input Validation**: All paths validated via `validate_safe_path`; DOS reserved names rejected.
- [x] **Local-First / Zero Telemetry**: Absolutely no external network calls or tracking SDKs.

### 9. Packaging & Distribution
- [x] **NSIS Per-User Installer**: `rust-explorer_0.1.0_x64-setup.exe` (3.55 MiB, installs to `%LOCALAPPDATA%\Programs\rust-explorer` with Start Menu shortcut).
- [x] **MSI Installer**: `rust-explorer_0.1.0_x64_en-US.msi` (5.15 MiB).
- [x] **Standalone Portable Executable**: `rust-explorer.exe` (14.43 MiB).
- [x] **Integrity & Metadata**: SHA-256 checksums in `artifacts/release/SHA256SUMS.txt`, metadata in `artifacts/release/BUILD_METADATA.json`, and release notes in `artifacts/release/RELEASE_NOTES.md`.

---

## Conclusion
All acceptance criteria specified in `RUST_WINDOWS_EXPLORER_AGENT_SPEC.md` for the MVP release have been completely implemented, verified with automated tests, benchmarked against performance budgets, and packaged into an installable distribution.
