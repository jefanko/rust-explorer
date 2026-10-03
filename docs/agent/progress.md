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

## Milestone M5 — Live Changes [COMPLETE]

### Step 1: Notify Adapter & Ingress Bounding
- Implemented `crates/explorer-watch/src/adapter.rs`:
  - `NotifyAdapter`: Integrates `notify::RecommendedWatcher` with Windows `ReadDirectoryChangesW` backend.
  - Ingress channel capacity bounded at 4,096 raw events (`tokio::sync::mpsc::channel(4096)`).
  - Handles backend errors and buffer saturation by raising `is_overflow` flags and emitting `WatchEventKind::Overflow` notifications.
  - Supports non-recursive (visible folder tabs) and recursive (indexed roots) watching modes.

### Step 2: Event Coalescer & Dirty-Root Bounding
- Implemented `crates/explorer-watch/src/coalesce.rs`:
  - `EventCoalescer`: Implements a 150 ms debounce coalescing window (conforming to the 100–200 ms requirement) with a maximum debounce wait cap of 500 ms to prevent starvation during continuous file system mutations.
  - Limits dirty directory records to at most 1,024 unique directories (`MAX_DIRTY_DIRS = 1024`).
  - Upon exceeding 1,024 dirty directories or upon receiving overflow events, transitions into overflow reconciliation and drops granular item records to prevent unbounded memory growth.
  - Unit tests verify deduplication, debounce timing, and overflow clearing.

### Step 3: Degradation Recovery & Tab Subscription Service
- Implemented `crates/explorer-watch/src/reconcile.rs`:
  - `ReconciliationManager`: Tracks per-path freshness and degradation state.
  - Implements exponential backoff on repeated reconciliation failures (2s default interval up to 30s maximum backoff).
- Implemented `crates/explorer-watch/src/service.rs`:
  - `WatchService`: Thread-safe coordinator for watches and tab subscriptions.
  - Reference-counted subscriptions per canonical path (`subscriber_count`).
  - Automatic watch mode upgrades (e.g. from non-recursive to recursive if required).
  - `unsubscribe_all`: Cleans up all watches for a closed tab, preventing resource and handle leaks (satisfies tab lifecycle leak gate).
  - Unit tests verify multi-tab subscription refcounts, tab closure cleanup, and live Windows filesystem modification events.

### Step 4: Desktop Host & Webview Event Bridge
- Updated `apps/desktop/src-tauri`:
  - Registered `WatchService` in `AppState`.
  - Added setup hook in `lib.rs` that subscribes to coalesced notifications and forwards them to the webview via Tauri event `"watch-notification"`.
  - Exposed IPC commands: `watch_folder`, `unwatch_folder`, `unwatch_all`, `get_watch_status`.
- Updated `apps/desktop/ui`:
  - Added typed bindings in `bridge/types.ts` and `bridge/client.ts`.
  - In `App.tsx`, automatically subscribes to active folder paths when tabs navigate, unwatching previous paths.
  - In `closeTab`, cleans up all watches associated with the closed tab.
  - Listens to `"watch-notification"` and automatically triggers directory refresh/reconciliation when watched folders change on disk.

### Step 5: Verification & Packaging
- Fast verification suite `.\scripts\check.ps1` passed 100% (format, clippy with `-D warnings`, 25 Rust workspace tests, TypeScript strict typecheck, Vitest unit tests, UI build, and doctor).
- Built debug desktop binary `target\debug\rust-explorer.exe` via `npx tauri build --debug --no-bundle`.

## Milestone M6 — Indexed Filename Search [COMPLETE]

### Step 1: SQLite FTS5 Schema, Triggers & Root Management
- Implemented `crates/explorer-index/src/db.rs`:
  - `IndexDb`: Dedicated SQLite database at `%LOCALAPPDATA%\RustExplorer\index.sqlite3` with WAL mode and `NORMAL` synchronous settings.
  - Tables: `roots` (max 8 non-overlapping roots), `entries` (authoritative UTF-16LE paths, display paths, kinds, sizes, timestamps, epochs), and `filename_fts` virtual table using `tokenize='trigram'`.
  - Automatic synchronization triggers: `entries_ai`, `entries_ad`, and `entries_au` ensure every insert, delete, or update in `entries` transactionally synchronizes normalized lowercase filenames in `filename_fts`.
  - Enforces non-overlapping root hierarchies: rejects duplicate paths and parent/child subpath overlaps using canonical path checks.
  - Unit test `test_index_db_schema_roots_and_triggers` verifies schema creation, root constraints, and trigger-maintained FTS rows.

### Step 2: Safe Bounded Metadata Crawler
- Implemented `crates/explorer-index/src/crawl.rs`:
  - `MetadataCrawler`: Breadth-first metadata-only directory crawler using native Win32 `FindFirstFileExW`. Never reads file contents or hydrates placeholders.
  - Reparse point safety: Section 14.4 requirement fulfilled—reparse points (symlinks, junctions) have their entry metadata indexed without traversing into their targets.
  - Default exclusions: automatically skips `$Recycle.Bin`, `System Volume Information`, and `%LOCALAPPDATA%\RustExplorer`.
  - Paced transactional batching: flushes entries every 500 items or 100 ms to avoid locking the database.
  - Epoch-based tracking: marks seen entries per scan epoch; removes missing children only after successful directory completion; supports cooperative cancellation.
  - Unit test `test_crawler_indexes_files_and_skips_exclusions` verifies file discovery, exclusion skipping, and reparse point handling.

### Step 3: Section 14.2 Search Query Engine & Trigram Ranking
- Implemented `crates/explorer-index/src/query.rs`:
  - `parse_query`: Implements Section 14.2 grammar: handles plain terms, `"quoted phrases"`, `ext:<ext>` filters, and `type:folder` / `type:file` filters.
  - Enforces minimum 3-character rule for substring search unless metadata-only query (`ext:` / `type:`).
  - Trigram ranking: ranks exact normalized filename matches first (`rank = 1`), prefix matches second (`rank = 2`), and substring matches third (`rank = 3`). Tie-breaks deterministically by normalized name, path bytes, and row ID.
  - Keyset / offset pagination: enforces 1,000 matches cap with `is_capped` indicator and 100 results per page.
  - Unit tests `test_parse_query_valid_and_invalid` and `test_execute_search_ranking_and_filters` verify ranking, extensions, and phrase matching.

### Step 4: Index Service & Tauri Host Integration
- Implemented `crates/explorer-index/src/service.rs`:
  - `IndexService`: Thread-safe service coordinating roots, background crawler threads, and query engine.
  - Background crawler worker threads: spawns dedicated OS threads with their own runtime, preventing lockups.
- Updated `apps/desktop/src-tauri`:
  - `state.rs`: Registered `IndexService` in `AppState`.
  - `commands.rs`: Added IPC commands `list_indexed_roots`, `add_indexed_root`, `remove_indexed_root`, `recrawl_indexed_root`, `search_indexed`, and `open_path`.
  - `lib.rs`: Registered all 6 commands in Tauri `generate_handler![]`.

### Step 5: Frontend Search UI & Root Management
- Updated `apps/desktop/ui`:
  - `bridge/client.ts` & `bridge/types.ts`: Typed TypeScript bindings for indexed search and root management.
  - Search Scope Switcher: `[ Folder | Indexed ]` toggle button group next to search bar in the toolbar.
  - Debounced Search: 150 ms debounce with cancellation token on superseded queries; displays query constraint messages ("Use at least 3 characters for indexed search") and loading spinner.
  - Virtualized Results View: Displays search results table with columns (Name, Location, Size, Date modified), file icons, and match count with cap warning ("First 1,000 matches; refine search").
  - Double-Click & Context Menu Actions: Double-click navigates into folders or opens files; right-click context menu offers "Open", "Open containing folder", and "Copy path".
  - Keyboard Navigation: Arrow keys navigate search results, Enter opens selected result, Escape clears query.
  - Sidebar Indexed Roots Section: Displays up to 8 roots with live status badges (`ready`, `scanning`, `degraded`, etc.), "+ Index current folder" button, recrawl button (🔄), and remove button (×).

### Step 6: Verification & Packaging
- Fast verification suite `.\scripts\check.ps1` passed 100% (format, clippy with `-D warnings`, 30 Rust workspace tests, TypeScript strict typecheck, Vitest unit tests, UI build, and doctor).
- Built debug desktop binary `target\debug\rust-explorer.exe` via `cargo build --package rust-explorer`.

## Milestone M7 — Hardening and Performance [COMPLETE]

### Step 1: 100k-Entry Stress Verification & Algorithm Optimizations
- Zero-Allocation Natural Sort Comparator (`crates/explorer-fs/src/listing.rs`):
  - Replaced heap-allocating `to_lowercase().to_string()` during sorting of directory entries with zero-allocation ASCII comparison fast-path.
  - Sorting 100,000 file entries dropped from 3.56s to **102.9 ms** (release profile), well below the 150 ms budget.
- Paged Snapshot Caching (`crates/explorer-fs/src/snapshots.rs` & `crates/explorer-fs/src/lib.rs`):
  - Added thread-safe `sorted_cache: RwLock<Option<(SortColumn, SortDirection, Vec<FileEntry>)>>` to `FolderSnapshot`.
  - Paginating 50-item slices across a 100k-entry directory listing takes **26 microseconds** (0.026 ms) instead of re-sorting.
  - Implemented LRU snapshot retention in `FolderService`: automatically bounds cached snapshots to 16 maximum (`MAX_CACHED_SNAPSHOTS = 16`), pruning oldest snapshots to eliminate memory leaks during long browsing sessions.
- Comprehensive 100k Stress Benchmark Suites:
  - `crates/explorer-fs/tests/stress_tests.rs`:
    - 100,000 entries generated in 43.1 ms
    - 100,000 entry snapshot created in 45.8 ms
    - Initial 100k natural sort + page 1 (50 items): 103.2 ms (budget < 150 ms)
    - Subsequent page 2 (50 items): 76.4 µs (budget < 5 ms)
    - Far page at offset 50,000: 26.0 µs (budget < 5 ms)
    - Reverse sort: 107.5 ms (budget < 150 ms)
    - Item token resolution: 200 ns per item (budget < 50 µs)
  - `crates/explorer-index/tests/stress_tests.rs`:
    - Inserted 100,000 entries into SQLite FTS5 in 14.5s (6,888 items/sec)
    - Database size on disk: **67.93 MiB** (budget <= 350 MiB)
    - Trigram query `"invoice"` (1,000 matches): 6.86 ms (budget < 50 ms)
    - Multi-term query `"invoice september"` (1,000 matches): 7.24 ms (budget < 50 ms)
    - Extension filter `"report ext:txt"` (500 matches): 5.50 ms (budget < 50 ms)
    - Exact phrase match: 799.7 µs (budget < 50 ms)
  - Benchmarks integrated into `xtask bench release` producing `artifacts/benchmarks/summary.md`.

### Step 2: Resource Limits & Leak Prevention Audit
- Watcher Lifecycle: Reference counted subscriptions per path; `unsubscribe_all` on tab close verified to clean up watches with zero handle leaks.
- Ingress & Overflow Bounds: Bounded channel of 4,096 items; bounded dirty directory tracking of 1,024 items with graceful overflow reconciliation.
- Crawler & Index Limits: BFS crawler commits batches every 500 items/100 ms; enforces maximum 8 non-overlapping roots; Section 14.2 search caps results at 1,000 matches.
- Memory & Lock Discipline: `RwLock` and `Mutex` guards are tightly scoped; no locks held across `.await` points; LRU snapshot pruning prevents unbounded memory growth.

### Step 3: Security & Capability Audit
- Tauri Window Capability: Explicitly bound `"label": "main"` in `tauri.conf.json` matching `"windows": ["main"]` in `capabilities/main.json`.
- Strict Content Security Policy: Configured `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost`. No `unsafe-eval`; remote asset and iframe execution blocked.
- Frontend Plugin Boundary: Only `"core:default"` permission granted to webview; `tauri-plugin-shell` commands are unexposed to the frontend, preventing arbitrary process execution.
- Path & Mutation Sanitization: Native Win32 mutations enforce `validate_safe_path`, reject DOS reserved names and UNC recycle operations, and require validated `OperationPlan` with single-use `CommitToken`.
- Guarded Recycle Bin: `ShellProgressSink` validates `TSF_DELETE_RECYCLE_IF_POSSIBLE` and aborts if recycling is unsupported, guaranteeing no silent fallback to permanent deletion.

**Next Milestone**: Milestone M8 — Installable MVP (NSIS installer packaging, release documentation, clean-machine acceptance, and checksums).

## Milestone M8 — Installable MVP [COMPLETE]

### Step 1: NSIS Per-User Installer Packaging
- Configured Tauri Windows bundler in `apps/desktop/src-tauri/tauri.conf.json` with `"windows": { "nsis": { "installMode": "currentUser" } }`.
- Created automated packaging pipeline in `scripts/package.ps1` running `npx --workspace=@rust-explorer/desktop tauri build`.
- Produced release artifacts in `target/release/bundle/`:
  - `rust-explorer_0.1.0_x64-setup.exe` (3.55 MiB NSIS installer, standard user install to `%LOCALAPPDATA%\Programs\rust-explorer` with Start Menu shortcut).
  - `rust-explorer_0.1.0_x64_en-US.msi` (5.15 MiB MSI package).
  - `rust-explorer.exe` (14.43 MiB optimized standalone portable binary).

### Step 2: Native E2E Test Suite & Journey Verification
- Implemented `apps/desktop/tests/native-e2e/runner.js`:
  - Spawns the release binary in an isolated temporary environment (`LOCALAPPDATA` redirected to fixture AppData).
  - Verifies process initialization, window display, and `Responding: true`.
  - Measures working set memory: **31.35 MiB** (well within the 350 MiB budget).
  - Verifies presence and integrity of release installer bundles.
  - Cleans up isolated test state safely.
- Expanded `apps/desktop/ui/src/test/App.test.tsx` (8 unit tests):
  - Added test coverage for Windows path breadcrumbs splitting.
  - Added test coverage for case-insensitive in-folder filtering.
  - Added test coverage for guarded Recycle Bin modal confirmation messages.
  - Verified Section 14.2 search query constraints.

### Step 3: Clean-Machine Acceptance & Release Deliverables
- Staged all required release artifacts under `artifacts/release/`:
  - `SHA256SUMS.txt`: Computed SHA-256 hashes for all 3 release binaries.
  - `BUILD_METADATA.json`: Machine-readable metadata (compiler, versions, targets, hashes, sizes).
  - `RELEASE_NOTES.md`: Comprehensive v0.1.0 release notes covering architecture, safety, benchmarks, and installation.
  - `DEFINITION_OF_DONE_EVIDENCE.md`: Line-by-line verification evidence of all criteria in Section 26.2 "MVP completion gate".
- Updated `README.md` with complete installation, build, testing, benchmarking, troubleshooting, and supported limitation sections.
- Fast verification suite `.\scripts\check.ps1` passed 100% (format, clippy with `-D warnings`, 33 tests, TypeScript strict, 8 unit tests, UI build, doctor).

**Project Status**: All milestones (M0 through M8) are 100% COMPLETE! The Rust Explorer installable MVP is fully implemented, hardened, verified, and packaged for release.


