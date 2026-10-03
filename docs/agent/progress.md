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

## Milestone M2 — Tabs and Interaction [COMPLETE]

### Step 1: SQLite Settings Store & Win32 Shell Extensions
- Implemented `crates/explorer-store/src/settings.rs`:
  - `SettingsStore` with SQLite `kv_store` (schema versioning, JSON app preferences) and `favorites` table (path, display name, timestamp).
  - Open in-memory support for testing and on-disk SQLite at `%LOCALAPPDATA%\RustExplorer\state.sqlite3`.
- Extended `crates/explorer-win/src/shell.rs`:
  - `show_file_properties`: Invokes native Windows Shell properties dialog using `ShellExecuteExW` with `SEE_MASK_INVOKEIDLIST` and `properties` verb.
  - `open_in_windows_explorer`: Spawns `explorer.exe /select,"<path>"` using argument vectors rather than shell interpolation.
- Added and registered Tauri IPC commands in `apps/desktop/src-tauri/src/commands.rs`:
  - `show_properties`, `open_in_explorer`, `load_settings`, `save_settings`, `add_favorite`, `remove_favorite`.

### Step 2: Multi-Tab State, Keyboard Navigation & Context Menus
- Refactored `apps/desktop/ui/src/app/App.tsx`:
  - Independent tab state: each tab preserves its own folder path, history stack, scroll position, entries snapshot, sort parameters, selection, and filter query.
  - Tab management: create tab (`+` or `Ctrl+T`), close tab (`×` or `Ctrl+W`), cycle tabs (`Ctrl+Tab`, `Ctrl+Shift+Tab`), and tab restoration via `SettingsStore`.
  - Comprehensive keyboard navigation:
    - `Alt+Left` / `Alt+Right` / `Alt+Up`: Back / Forward / Parent directory.
    - `Ctrl+L`: Focus address bar with automatic text selection.
    - `Ctrl+F`: Focus search/filter input.
    - `F5`: Refresh current folder.
    - `Ctrl+A`: Select all items in current listing (excluded while typing in inputs).
    - `ArrowUp` / `ArrowDown`: Visible focus row navigation with automatic scroll into view.
    - `Shift+ArrowUp` / `Shift+ArrowDown`: Multi-selection range expansion.
    - `Home` / `End`: Jump to first or last row.
    - `Enter`: Navigate into folder or launch file with associated application.
    - `Alt+Enter`: Open native Windows Shell properties dialog.
    - `Escape`: Clear selection, blur active inputs, or dismiss context menu.
  - Context menu:
    - Custom app-owned context menu on right click for files, folders, and empty folder background.
    - Menu actions: Open, Open in Windows Explorer, Add to Favorites, Refresh, Properties.
  - Favorites & Themes:
    - Sidebar Favorites section with pinned paths and quick remove buttons.
    - System / Light / Dark theme switcher with persistence in SQLite store.
- Added unit tests in `apps/desktop/ui/src/test/App.test.tsx` verifying formatting, tab title extraction, and selection range computation.
- Verified with `.\scripts\check.ps1` (0 warnings, 0 errors, all Rust + UI tests passing).
- Packaged offline standalone binary `rust-explorer.exe` with embedded assets via `npx tauri build --debug --no-bundle`.

**Next Milestone**: Milestone M3 — Native Mutation Vertical Slice (Dedicated COM STA worker for `IFileOperation`, immutable operation plans, single-use commit tokens, create-folder and rename, job drawer UI).
