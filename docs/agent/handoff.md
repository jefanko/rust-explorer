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

## Current Milestone
- **Milestone**: M5 — Live Changes
- **Active Tickets**:
  - `M5.1 Directory Watching`: `notify` backend integration for active tab folder watching and indexed roots.
  - `M5.2 Coalescing & Reconciliation`: Event batching, debounce, dirty-root tracking, and folder snapshot reconciliation.

## Next Commands
1. Review `crates/explorer-watch` structure and dependencies (integrate `notify` crate with recommended Windows ReadDirectoryChangesW backend).
2. Implement watch subscription service managing active directory watches per tab.
3. Implement coalescing and debounce logic to avoid event flooding during bulk filesystem operations.
4. Wire watch events into `FolderService` / active snapshot refresh and emit UI change signals.
5. Verify with `.\scripts\check.ps1` and test live file change reflections.

