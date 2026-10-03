# Rust Explorer Backlog

## Milestone M0 — Environment, Scaffolding, and Baseline Desktop Host [COMPLETE]
- [x] **M0.1 Environment Audit & Toolchain Setup**: Verify/install Rust (`x86_64-pc-windows-msvc`), MSVC C++ Build Tools & Windows SDK, Node.js LTS, WebView2 runtime.
- [x] **M0.2 Workspace Scaffolding**: Setup root `Cargo.toml`, `package.json`, `.cargo/config.toml`, `rust-toolchain.toml`, `crates/` layout, and `apps/desktop` Tauri 2 app.
- [x] **M0.3 Xtask & Script Infrastructure**: Implement `xtask doctor` (toolchain, SQLite FTS5 trigram verification) and core build/lint/test scripts.
- [x] **M0.4 Blank Window Launch & CI Baseline**: Verify compilation and launch of desktop Tauri 2 window on Windows 11. Record versions in `docs/environment.md`.

## Milestone M1 — Safe Browsing [COMPLETE]
- [x] **M1.1 Lossless Path & Domain Models**: Implement `explorer-domain` types, lossless UTF-16 path conversions, opaque item tokens, structured errors.
- [x] **M1.2 Native Win32 Enumeration**: Implement `explorer-win` directory enumeration using wide APIs (`FindFirstFileExW`/`FindNextFileW`) and owned handles.
- [x] **M1.3 Filesystem Service & Snapshots**: Implement `explorer-fs` listing snapshot, natural sorting (directories first), cancelable pagination, and bounded workers.
- [x] **M1.4 Tauri IPC Adapter**: Implement typed commands/events for navigation and listing (`bootstrap`, `navigate`, `list_page`, `refresh`, `open_item`).
- [x] **M1.5 Virtualized Details View**: React + `@tanstack/react-virtual` table rendering name, type, size, modified date with loading and error states.
- [x] **M1.6 Sidebar Known Folders & Drives**: Integrate `SHGetKnownFolderPath` (Documents, Downloads, Desktop, etc.) and `GetLogicalDriveStringsW` / `GetDiskFreeSpaceExW`.
- [x] **M1.7 Navigation Address Bar & History**: Implement editable address bar, back/forward/up navigation with history stack, and instant folder filtering.

## Milestone M2 — Tabs and Interaction [COMPLETE]
- [x] **M2.1 Multi-Tab State**: Independent location, history, scroll, and selection per tab.
- [x] **M2.2 Keyboard Navigation**: Full keyboard shortcuts (Enter, Arrows, Alt+Left/Right/Up, Ctrl+L, Ctrl+T, Ctrl+W, F5).
- [x] **M2.3 Selection & Context Menu**: Multi-selection, stable row keys, app-owned context menu.
- [x] **M2.4 Native Open & Properties**: `ShellExecuteExW` file association launch and native properties dialog.
- [x] **M2.5 Persistent Preferences**: Settings store for theme, favorites, tab restoration, column widths.

## Milestone M3 — Native Mutation Vertical Slice [COMPLETE]
- [x] **M3.1 Dedicated COM STA Worker**: File operation STA thread with message pump for `IFileOperation`.
- [x] **M3.2 Job Model & State Machine**: Planning, tokens, immutable plans, single-use commit, journal transitions.
- [x] **M3.3 Create Folder & Rename Slice**: Implement create-folder and rename via Shell backend with pre/post callbacks.
- [x] **M3.4 Job Drawer UI**: Drawer showing progress, outcomes, and failure states.

## Milestone M4 — Everyday File Operations [COMPLETE]
- [x] **M4.1 Copy & Move via Shell**: Execute copy/move jobs with conflict resolution via native dialogs.
- [x] **M4.2 Guarded Recycle**: Recycle bin deletion with `FOFX_RECYCLEONDELETE` and `PreDeleteItem` checks (no silent fallback).
- [x] **M4.3 Cancellation & Partial Outcomes**: Cooperative cancellation, item-level outcome recording, retry planning.
- [x] **M4.4 Explorer Clipboard Interoperability**: `CF_HDROP` copy/cut/paste interoperable with Windows Explorer.

## Milestone M5 — Live Changes
- [ ] **M5.1 Directory Watching**: `notify` backend integration for visible folders and indexed roots.
- [ ] **M5.2 Coalescing & Reconciliation**: Event batching, debounce, dirty-root tracking, and overflow recovery.

## Milestone M6 — Indexed Filename Search
- [ ] **M6.1 SQLite FTS5 Setup**: Authoritative metadata table and FTS5 trigram table with sync triggers.
- [ ] **M6.2 Metadata Crawler**: Safe non-recursive reparse point crawling, background throttling.
- [ ] **M6.3 Search Query Engine**: Query parser (terms, phrases, `ext:`, `type:`), ranking, paged results.
- [ ] **M6.4 Search UI**: Current folder filter and indexed search scope switcher.

## Milestone M7 — Hardening and Performance
- [ ] **M7.1 100k-Entry Stress Verification**: Virtualization and memory stability test on 100k items.
- [ ] **M7.2 Resource Limits & Leak Prevention**: Audit handle, memory, and channel bounds.
- [ ] **M7.3 Security Audit**: CSP review, IPC authority checks, input sanitization.

## Milestone M8 — Installable MVP
- [ ] **M8.1 NSIS Installer**: Per-user installer packaging with Start Menu shortcut.
- [ ] **M8.2 Native E2E Test Suite**: Automated WebDriver tests driving actual Windows app.
- [ ] **M8.3 Clean-Machine Acceptance**: Full execution of Section 1.1 user journey and Section 26.2 MVP completion gate.
