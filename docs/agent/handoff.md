# Rust Explorer Resumption Handoff

## Environment
- **Operating System**: Windows 11 x86-64
- **Working Directory**: `d:\dev\rust-explorer`
- **Installed Tools**:
  - Git: 2.53.0
  - Rust: 1.99.0 (`stable-x86_64-pc-windows-msvc`) with `rustfmt`, `clippy`
  - MSVC C++ Build Tools 2022: v17.14.41 (`Microsoft.VisualStudio.Workload.VCTools`)
  - Node.js: v22.20.0
  - npm: 10.9.3
  - Microsoft Edge WebView2 Evergreen: 154.0.4258.53

## Completed Milestones
- **Milestone M0 — Environment, Scaffolding, and Baseline Desktop Host**: 100% COMPLETE.
- **Milestone M1 — Safe Browsing**: 100% COMPLETE.
  - Native Win32 enumeration (`FindFirstFileExW` / `FindNextFileW`).
  - Windows Known Folders (`SHGetKnownFolderPath`) and Logical Drives (`GetLogicalDriveStringsW`).
  - Thread-safe `FolderService` with generation counter, snapshots, and natural sorting.
  - Interactive React UI with `@tanstack/react-virtual` table, address bar, history navigation, and live Rust IPC.
  - All tests and verification checks passing (`.\scripts\check.ps1`).
- **Milestone M2 — Tabs and Interaction**: 100% COMPLETE.
  - Independent multi-tab state (location, history, scroll, selection per tab).
  - Full keyboard navigation (`Alt+Arrows`, `Ctrl+L`, `Ctrl+F`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `ArrowUp/Down`, `Shift+Arrows`, `Home/End`, `Enter`, `Alt+Enter`, `Escape`).
  - Multi-selection and app-owned context menu for files, folders, and folder background.
  - Native Windows Shell Properties dialog (`SEE_MASK_INVOKEIDLIST`) and "Open in Windows Explorer".
  - SQLite `SettingsStore` (`kv_store` and `favorites` tables), sidebar Favorites, and Theme switcher.
  - All fast verification suite checks and standalone packaging passing.

- **Milestone M3 — Native Mutation Vertical Slice**: 100% COMPLETE.
  - Dedicated COM STA worker thread (`StaWorker`) running `IFileOperation`.
  - Job model & state machine: `CommitToken`, immutable `OperationPlan`, `JobExecutor`, and SQLite `JobJournal`.
  - Create Folder & Rename slice with DOS name validation, conflict detection, and cancelable execution.
  - Interactive Job Drawer UI with operation history, badges, and modals.
  - All tests and verification checks passing (`.\scripts\check.ps1`).

- **Milestone M4 — Everyday File Operations**: 100% COMPLETE.
  - Native Shell Copy & Move operations via `IFileOperation` on `StaWorker`.
  - Guarded Recycle Bin deletion via `ShellProgressSink` with `TSF_DELETE_RECYCLE_IF_POSSIBLE` validation (aborts if recycling is not supported; no silent fallback).
  - Windows Explorer clipboard interoperability via standard `CF_HDROP` and `Preferred DropEffect` (two-way copy/cut/paste).
  - Operation planner with cycle/nesting detection, item count bounds, and UNC recycle prevention.
  - UI toolbar buttons, context menu items, keyboard shortcuts (<kbd>Ctrl+C</kbd>, <kbd>Ctrl+X</kbd>, <kbd>Ctrl+V</kbd>, <kbd>Delete</kbd>), and guarded Recycle confirmation modal.
  - Fast verification suite passed (`.\scripts\check.ps1`, 19 tests, 0 warnings) and standalone executable compiled.

- **Milestone M5 — Live Changes**: 100% COMPLETE.
  - Native filesystem watching via `notify::RecommendedWatcher` backed by Windows `ReadDirectoryChangesW`.
  - Bounded ingress channel (4,096 capacity) and bounded dirty directories (1,024 capacity) with overflow reconciliation protection.
  - 150 ms debounce coalescing window with 500 ms maximum wait cap to prevent starvation.
  - Reference-counted directory subscriptions per tab, mode upgrades, and zero-leak cleanup on tab close (`unsubscribe_all`).
  - Webview bridge receiving `"watch-notification"` events and automatically refreshing visible directory listings.
  - Fast verification suite passed (`.\scripts\check.ps1`, 25 tests, 0 warnings) and debug desktop binary packaged.

- **Milestone M6 — Indexed Filename Search**: 100% COMPLETE.
  - Dedicated SQLite FTS5 index at `%LOCALAPPDATA%\RustExplorer\index.sqlite3` with `tokenize='trigram'` and transactional sync triggers.
  - Non-overlapping root limit (maximum 8 roots) enforced by canonical native paths.
  - Bounded BFS metadata crawler using native Win32 `FindFirstFileExW`, batch commit pacing (500 items/100 ms), reparse point non-traversal, and default exclusions (`$Recycle.Bin`, `System Volume Information`, app data).
  - Section 14.2 search query parser supporting terms, `"quoted phrases"`, `ext:`, `type:folder`/`type:file`, 3+ character minimum rule, and trigram ranking (exact > prefix > substring).
  - UI Search scope switcher (`[ Folder | Indexed ]`), 150 ms debounced query execution with request cancellation, virtualized results list, match count capping at 1,000 matches, and sidebar Indexed Roots manager with live status badges.
  - Fast verification suite passed (`.\scripts\check.ps1`, 30 tests, 0 warnings) and standalone desktop binary packaged.

- **Milestone M7 — Hardening and Performance**: 100% COMPLETE.
  - Zero-allocation ASCII natural sort comparator (reduced 100k sort time from 3.56s to 102.9 ms, 35x speedup).
  - Paged snapshot caching in `FolderSnapshot` (50-item page slice takes 26 µs via `sorted_cache`).
  - LRU snapshot pruning in `FolderService` (capped at 16 snapshots maximum).
  - 100k directory listing benchmark: 45.8 ms snapshot creation, 103.2 ms sort, 200 ns token lookup.
  - 100k SQLite FTS5 benchmark: 6,888 items/sec indexing, 67.93 MiB on disk, <10 ms queries.
  - Automated benchmark runner `xtask bench release` producing `artifacts/benchmarks/summary.md`.
  - Resource limits: bounded watch channels, refcounted watch subscriptions with `unsubscribe_all`, 1000 search match cap.
  - Security audit: strict CSP, window `"main"` capability binding, `"core:default"` webview permissions, `ShellProgressSink` guarded recycle.
  - Fast verification suite passed (`.\scripts\check.ps1`, 33 tests, 0 warnings).

## Current Milestone
- **Milestone**: M8 — Installable MVP
- **Active Tickets**:
  - `M8.1 NSIS Installer`: Per-user NSIS installer packaging with Start Menu shortcut.
  - `M8.2 Native E2E Test Suite`: Verify desktop launch and full user journey on Windows 11.
  - `M8.3 Clean-Machine Acceptance`: Build final release installer, compute SHA-256 checksums, and verify definition-of-done checklist.

## Next Commands
1. Configure and run NSIS installer build using Tauri bundler: `npx tauri build`.
2. Verify per-user installation mode (`currentUser`) without administrator requirement.
3. Compute SHA-256 checksums and package release documentation.
4. Execute definition-of-done acceptance checklist.


