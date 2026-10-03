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
## Milestone M3 — Native Mutation Vertical Slice [COMPLETE]

### Step 1: STA COM Worker & Shell Operation Pipeline
- Implemented `crates/explorer-win/src/com.rs`:
  - `StaWorker`: Dedicated OS thread initialized with `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` and an internal task queue. Guarantees `IFileOperation` runs safely on a single STA thread without blocking the UI thread or Tauri async worker pool.
- Implemented `crates/explorer-win/src/shell.rs`:
  - `shell_create_folder`: Creates a new folder via Win32 `IFileOperation` (`NewItem`), `IShellItem`, and checks `GetAnyOperationsAborted()`.
  - `shell_rename_item`: Renames an existing file or directory via Win32 `IFileOperation` (`RenameItem`) and checks `GetAnyOperationsAborted()`.
  - Unit test `test_shell_create_folder_and_rename_in_sta` tests end-to-end STA dispatch on a dedicated temporary test fixture directory.

### Step 2: Job Models, Planning & SQLite Journal
- Extended `crates/explorer-domain`:
  - `CommitToken`: Unique single-use token tied to each planned operation.
  - `OperationPlan`, `JobState`, `ItemStatus`, `ItemOutcome`, `JobSummary` types in `operations.rs`.
- Implemented `crates/explorer-store/src/journal.rs`:
  - `JobJournal`: Persistent SQLite table `job_journal` tracking job history, state transitions (`Validating`, `Queued`, `Running`, `Completed`, `Failed`, `Canceled`, `Interrupted`), error records, and start/finish epoch timestamps.
  - Automatic interrupted job recovery: transitions any incomplete jobs left in `running`, `validating`, or `queued` state to `interrupted` upon app startup.
- Implemented `crates/explorer-jobs`:
  - `planner.rs`: DOS reserved name validation (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`, control chars, trailing dots/spaces), path existence and conflict checks, and 5-minute expiry timestamp.
  - `executor.rs`: Enforces single-use `commit_token` idempotency, records state transitions in `JobJournal`, and dispatches operations to `StaWorker`.
  - `queue.rs`: `OperationService` coordinating planner, executor, and journal.

### Step 3: Tauri IPC & Frontend Job Drawer
- Extended `apps/desktop/src-tauri`:
  - `state.rs`: Registered `StaWorker`, `JobJournal`, and `OperationService` in `AppState`. Invokes `recover_interrupted_jobs()` on app bootstrap.
  - `commands.rs`: Exposed IPC commands `plan_create_folder`, `plan_rename`, `commit_plan`, `create_folder`, `rename_item`, and `list_jobs`.
- Enhanced `apps/desktop/ui`:
  - `bridge/client.ts` & `bridge/types.ts`: TypeScript bindings for operation planning, commits, and job history.
  - Interactive Folder Creation: "📁+ New Folder" button in navbar, context menu action, and `Ctrl+Shift+N` shortcut with modal prompt.
  - Interactive Item Rename: Context menu action and `F2` shortcut with modal prompt.
  - Job Drawer: Slide-out drawer displaying recent operations, state badges (`Completed`, `Failed`, `Canceled`, `Running`), timestamps, and error details.
  - Verification: `.\scripts\check.ps1` passed 100% (format, clippy, unit tests, TypeScript, build:ui, doctor), and standalone `rust-explorer.exe` built.

## Milestone M4 — Everyday File Operations [COMPLETE]

### Step 1: Shell Progress Sink & Guarded Recycle Bin Deletion
- Implemented `crates/explorer-win/src/sink.rs`:
  - `ShellProgressSink`: Implements COM interface `IFileOperationProgressSink` via `windows_core::implement`.
  - In `PreDeleteItem`, inspects deletion transfer flags (`TSF_DELETE_RECYCLE_IF_POSSIBLE = 0x00000080`). If the target volume/storage does not support recycling or if the flag is missing, it immediately aborts the operation (`E_ABORT`) and sets `aborted_for_safety = true`. Strictly enforces the contract rule: *"Never silently fall back from Recycle to permanent deletion."*
  - `SinkTracker`: Thread-safe tracking of per-item lifecycle (`PreCopyItem`, `PostCopyItem`, `PreMoveItem`, `PostMoveItem`, `PreDeleteItem`, `PostDeleteItem`, `ResetTimerOrAbort`), recording item outcomes (completed, failed, canceled).
- Implemented `crates/explorer-win/src/shell.rs`:
  - `shell_copy_items`: Copies multiple source paths to a destination folder using `IFileOperation::CopyItem` in `StaWorker`.
  - `shell_move_items`: Moves multiple source paths to a destination folder using `IFileOperation::MoveItem` in `StaWorker`.
  - `shell_recycle_items`: Safely deletes items to the Recycle Bin using `IFileOperation::DeleteItem` with `FOFX_RECYCLEONDELETE | FOFX_EARLYFAILURE` and `ShellProgressSink`. Preemptively rejects drive roots and UNC paths (`\\server\share`).
  - Unit test `test_shell_copy_move_and_recycle_in_sta`: Validates copy, move, and recycle pipeline inside a temporary test fixture directory.

### Step 2: Windows Explorer Clipboard Interoperability (CF_HDROP)
- Implemented `crates/explorer-win/src/clipboard.rs`:
  - `write_clipboard_hdrop`: Writes file paths using standard `CF_HDROP` (`DROPFILES` struct with wide characters) and registered clipboard format `Preferred DropEffect` (`DROPEFFECT_COPY = 1` for Copy, `DROPEFFECT_MOVE = 2` for Cut). Transfers handle ownership safely to Windows.
  - `read_clipboard_hdrop`: Reads `CF_HDROP` file list and checks `Preferred DropEffect` to detect whether items were cut or copied. Enables seamless two-way interoperability with Windows File Explorer.
  - Unit test `test_clipboard_roundtrip`: Verifies clipboard writing and reading for both copy and move modes.

### Step 3: Job Planner & Executor Pipelines
- Implemented `crates/explorer-jobs/src/planner.rs`:
  - `plan_copy`: Validates source existence, checks maximum 10,000 items, deduplicates sources, ensures destination is a directory, and checks for source-inside-destination cycles.
  - `plan_move`: Same validations as copy, plus verifies destination is not inside source (preventing moving a directory into its own child).
  - `plan_recycle`: Validates source existence, rejects drive roots, rejects UNC paths (`ErrorCode::RecycleUnsupported`), and deduplicates items.
- Extended `crates/explorer-jobs/src/executor.rs`:
  - Dispatches `Copy`, `Move`, and `Recycle` operations through `StaWorker`.
  - Records job state transitions (`Running`, `Completed`, `Failed`, `Canceled`) and item outcome counts in `JobJournal`.
  - Unit tests `test_plan_copy_move_and_recycle` and `test_executor_copy_move_and_recycle_pipeline` pass 100%.
- Extended `crates/explorer-jobs/src/queue.rs`:
  - Added `plan_copy`, `plan_move`, `plan_recycle`, `execute_copy`, `execute_move`, `execute_recycle` methods to `OperationService`.

### Step 4: Tauri IPC & Desktop UI Actions
- Implemented Tauri IPC commands in `apps/desktop/src-tauri/src/commands.rs`:
  - `plan_copy`, `plan_move`, `plan_recycle`, `execute_copy`, `execute_move`, `execute_recycle`, `clipboard_write`, `clipboard_read`.
- Extended `apps/desktop/ui`:
  - `bridge/client.ts` & `bridge/types.ts`: TypeScript wrappers and DTOs for clipboard and file operations.
  - Toolbar Action Buttons: Copy, Cut, Paste, Delete buttons with dynamic enable/disable states based on current selection and clipboard content.
  - Context Menu Actions: Copy, Cut, Delete on selected items; Paste on folder background.
  - Keyboard Shortcuts: <kbd>Ctrl+C</kbd> (copy), <kbd>Ctrl+X</kbd> (cut), <kbd>Ctrl+V</kbd> (paste), <kbd>Delete</kbd> (recycle modal).
  - Guarded Recycle Modal: Prompts user before deletion, showing item count, item names, and explicit notice that items are sent to the Recycle Bin.
  - Automatic View Refresh: Automatically refreshes active folder snapshot after operation commits to immediately display file changes.

### Step 5: Verification & Packaging
- Fast verification suite `.\scripts\check.ps1` passed 100% (format, clippy with `-D warnings`, 19 Rust workspace tests, TypeScript strict typecheck, Vitest unit tests, UI build, and doctor).
- Built debug desktop binary `target\debug\rust-explorer.exe` via `npx tauri build --debug --no-bundle`.

**Next Milestone**: Milestone M5 — Live Changes (`notify` backend integration for visible folders and indexed roots, event batching, debounce, dirty-root tracking, and directory snapshot reconciliation).

