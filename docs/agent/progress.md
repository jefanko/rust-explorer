# Rust Explorer Progress Log

## Milestone M0 — Environment, Scaffolding, and Baseline Desktop Host [COMPLETE]
- Scaffolding, toolchain installation, and baseline desktop window setup complete.

## Milestone M1 — Safe Browsing [COMPLETE]

### Step 1: Lossless Paths & Native Win32 Enumeration
- Enhanced `crates/explorer-domain`: added `DirectoryPage`, `KnownFolderItem`, `DriveItem`, `BootstrapData`, `NavigationResponse`, `SortColumn`, and `SortDirection`.
- Optimized `ExplorerError` to wrap `Box<ExplorerErrorData>` with `#[serde(transparent)]`, keeping `Result<T, ExplorerError>` down to 8 bytes.
- Implemented `explorer-win/src/path.rs`: lossless UTF-16 conversions, extended length prefix (`\\?\` / `\\?\UNC\`), and strict path security validation.
- Implemented `explorer-win/src/enumerate.rs`: native `FindFirstFileExW` / `FindNextFileW` with `FIND_FIRST_EX_LARGE_FETCH` and `FindExInfoBasic`.
- Implemented `explorer-win/src/known_folders.rs`: `SHGetKnownFolderPath` (Desktop, Documents, Downloads, Pictures, Music, Videos) and `GetLogicalDriveStringsW` / `GetDiskFreeSpaceExW`.
- Implemented `explorer-win/src/shell.rs`: `open_file_with_association` using `ShellExecuteExW`.

### Step 2: Filesystem Snapshot Service & Natural Sorting
- Implemented `explorer-fs/src/listing.rs`: natural sorting comparator (e.g. `file2` < `file10`) with folders placed first.
- Implemented `explorer-fs/src/snapshots.rs`: `FolderSnapshot` caching raw enumerated entries, generation numbers, and token-to-path mappings.
- Implemented `explorer-fs/src/lib.rs`: `FolderService` providing thread-safe `navigate`, `list_page`, and `resolve_item`.

### Step 3: Tauri IPC & Frontend Virtualized UI
- Implemented `apps/desktop/src-tauri/src/commands.rs`: `bootstrap`, `navigate`, `list_page`, `refresh`, `open_item`.
- Created `apps/desktop/ui/src/bridge/`: typed TypeScript DTOs and `client.ts` invoke wrapper.
- Implemented `apps/desktop/ui/src/app/App.tsx`:
  - Connected to live Rust backend via `client.bootstrap()`.
  - Sidebar showing live Windows Known Folders and Logical Drives.
  - Virtualized table using `@tanstack/react-virtual` for fast rendering of large directory listings.
  - Column sorting (Name, Type, Size, Date modified) with direction toggles.
  - History stack (Back, Forward, Up to parent).
  - Editable address bar with direct path submission.
  - Instant in-folder filtering.
  - Double-click folder navigation and file launching via native Windows association.
- Verified with `.\scripts\check.ps1` (0 warnings, 0 errors, all tests passing).
- Rebuilt desktop binary `rust-explorer.exe` with embedded assets and launched on desktop.

**Next Milestone**: Milestone M2 — Tabs and Interaction (multi-tab state, keyboard shortcuts, favorites, persistent settings).
