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

## Current Milestone
- **Milestone**: M3 — Native Mutation Vertical Slice
- **Active Tickets**:
  - `M3.1 Dedicated COM STA Worker`: File operation STA thread with message pump for `IFileOperation`.
  - `M3.2 Job Model & State Machine`: Planning, tokens, immutable plans, single-use commit, journal transitions.
  - `M3.3 Create Folder & Rename Slice`: Implement create-folder and rename via Shell backend with pre/post callbacks.
  - `M3.4 Job Drawer UI`: Drawer showing progress, outcomes, and failure states.

## Next Commands
1. Implement dedicated COM STA worker thread in `explorer-win` / `explorer-jobs`.
2. Implement immutable mutation planner and journal in `explorer-jobs` and `explorer-store`.
3. Implement `create_folder` and `rename_item` Tauri commands with progress reporting.
4. Add Job Drawer UI in frontend to display operation status and outcomes.
5. Run `.\scripts\check.ps1`.
