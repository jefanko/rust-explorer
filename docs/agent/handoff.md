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

## Current Milestone
- **Milestone**: M4 — Everyday File Operations
- **Active Tickets**:
  - `M4.1 Copy & Move via Shell`: Execute copy/move jobs with conflict resolution via native Shell backend.
  - `M4.2 Guarded Recycle`: Recycle Bin deletion with `FOFX_RECYCLEONDELETE` and `PreDeleteItem` checks (no silent fallback).
  - `M4.3 Cancellation & Partial Outcomes`: Cooperative cancellation, item-level outcome recording, retry planning.
  - `M4.4 Explorer Clipboard Interoperability`: `CF_HDROP` copy/cut/paste interoperable with Windows Explorer.

## Next Commands
1. Implement Shell copy and move operations in `crates/explorer-win/src/shell.rs` via `IFileOperation::CopyItem` and `IFileOperation::MoveItem`.
2. Implement guarded Recycle Bin deletion via `IFileOperation::DeleteItem` with `FOFX_RECYCLEONDELETE` and capability checks.
3. Extend planner and executor in `crates/explorer-jobs` for copy, move, and recycle.
4. Implement Windows clipboard format reading/writing (`CF_HDROP` / `Preferred DropEffect`) for seamless copy/cut/paste with Windows Explorer.
5. Add copy/cut/paste/delete actions to UI context menus, keyboard shortcuts (<kbd>Ctrl+C</kbd>, <kbd>Ctrl+X</kbd>, <kbd>Ctrl+V</kbd>, <kbd>Delete</kbd>), and verify with `.\scripts\check.ps1`.

