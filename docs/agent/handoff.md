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

## Current Milestone
- **Milestone**: M6 — Indexed Filename Search
- **Active Tickets**:
  - `M6.1 SQLite FTS5 Setup`: Authoritative metadata table and FTS5 trigram table with sync triggers.
  - `M6.2 Metadata Crawler`: Safe non-recursive reparse point crawling, background throttling.
  - `M6.3 Search Query Engine`: Query parser (terms, phrases, `ext:`, `type:`), ranking, paged results.
  - `M6.4 Search UI`: Current folder filter and indexed search scope switcher.

## Next Commands
1. Review `crates/explorer-index` crate structure and dependencies.
2. Implement SQLite schema with `roots`, `entries`, and `filename_fts` virtual table using the trigram tokenizer.
3. Add sync triggers ensuring FTS5 table stays consistent with entries metadata on insert, update, and delete.
4. Implement metadata crawler respecting exclusion paths, reparse point skipping, and batch commit pacing (500 entries or 100 ms).
5. Implement search query parser supporting terms, phrases (`"..."`), `ext:`, `type:`, and trigram ranking.
6. Connect search IPC commands to Tauri and add search scope switcher in UI.

