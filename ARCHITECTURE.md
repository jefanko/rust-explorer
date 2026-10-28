# Rust Explorer Architecture

Tanggal: 10 Oktober 2026
Status: Native Win32 (windows crate 0.58)

## 1. High-Level Architecture Overview

Rust Explorer is structured as a modular workspace separating domain logic, filesystem interactions, durable storage, indexing, jobs execution, and the native Win32 UI presentation layer.

```
+------------------------------------------------------------------------+
|                      apps/native-win32 (rust-explorer)                 |
|  - Win32 Message Loop (GetMessageW, TranslateAcceleratorW)             |
|  - Window Procedures & Control Subclasses                              |
|  - Virtual Owner-Data ListView (LVS_OWNERDATA)                         |
|  - TabPresenter with BoundedRowCache & Filter Engine                   |
|  - Event Dispatcher (WM_APP_WAKEUP coalesced channel)                  |
+------------------------------------------------------------------------+
                                    |
                                    v
+------------------------------------------------------------------------+
|                             explorer-app                               |
|  - Host-independent Application Controller (AppState)                  |
|  - Async operations & command facades (navigate, list_page, plan_...)  |
|  - RequestIdentity & CompletionEnvelope dispatch protocol              |
|  - Lifecycle orchestration (init, recovery, root restore)               |
+------------------------------------------------------------------------+
      |               |              |              |             |
      v               v              v              v             v
+-----------+   +-----------+  +-----------+  +-----------+  +-----------+
|explorer-fs|   |explorer-win| |explorer-job| |explorer-idx| |explorer-wat|
|Directory  |   |Win32 COM  |  |Two-phase  |  |SQLite FTS5|  |ReadDirect |
|snapshots, |   |STA worker,|  |planner,   |  |trigram,   |  |ChangesW   |
|natural    |   |IFileOper, |  |commit,    |  |crawler,   |  |coalescing |
|sorting    |   |Instance   |  |journal    |  |query svc  |  |debounce   |
+-----------+   +-----------+  +-----------+  +-----------+  +-----------+
      |               |              |              |             |
      +---------------+--------------+--------------+-------------+
                                     |
                                     v
                        +-------------------------+
                        |     explorer-domain     |
                        | Pure data models & IDs  |
                        +-------------------------+
```

## 2. Component Layers

### 2.1 Native Presentation Layer (`apps/native-win32`)
- **Direct Win32 Message Pump**: Standard Win32 message loop handling `-1`, `0`, and `1` from `GetMessageW`, keyboard accelerator translation (`HACCEL`), and subclassed edit notifications.
- **Controls Hierarchy**:
  - `ToolbarWindow32`: Back, Forward, Up, Refresh, Dark Mode, Preview Toggle buttons.
  - `Edit` (Address & Search): In-folder filtering on `EN_CHANGE` with escape restore, Enter navigation.
  - `SysTabControl32`: Native multi-tab container with independent history stacks.
  - `SysTreeView32`: Quick Access Known Folders, Drives with free/total storage, and Favorites.
  - `SysListView32` (`LVS_REPORT | LVS_OWNERDATA`): Virtual listview handling millions of items with `LVN_GETDISPINFOW` and `LVN_ODCACHEHINT` notifications.
  - `PreviewPane`: Text reader (up to 64 KiB), image renderer (WIC / GDI), and folder child scanner.
  - `msctls_statusbar32`: Live directory path, filtered item count, and multi-selection statistics.
- **Thread Dispatcher (`dispatcher.rs`)**:
  - Background Tokio tasks send completions through a thread-safe bounded queue (`VecDeque<UiEvent>`).
  - Coalesces UI wakeups using `WM_APP_WAKEUP` to eliminate UI stuttering.
  - Drains events in bounded batches to avoid starving window paint or mouse events.
  - The window is created hidden and bound to the dispatcher before the first navigation, so no completion can be enqueued against a null `HWND`.
- **Crash Diagnostics (`diagnostics.rs`)**:
  - Panic hook plus `SetUnhandledExceptionFilter` write `[panic]` / `[crash]` records with a DbgHelp-symbolised stack to `%LOCALAPPDATA%\RustExplorer\logs\rust-explorer.log`.
  - The window procedure is wrapped in `catch_unwind`, so a panic degrades to a logged error instead of aborting the process.
  - `RUST_EXPLORER_TRACE=1` records per-message breadcrumbs for UI-order bugs.
- **Shell Icons (`icons.rs`)**: caches system small-image-list indices per extension so the virtual list and sidebar render real Windows icons without per-row disk access.
- **Modal Safety (`dialogs.rs`)**: message boxes, prompt dialogs and popup menus mark a nested modal scope; re-entrant `WM_COMMAND`/`WM_CONTEXTMENU`/`WM_CLOSE` handling is suppressed and deferred errors surface in the status bar instead of stacking dialogs.

### 2.2 Application Controller (`crates/explorer-app`)
- Completely decoupled from UI frameworks (reusable by headless tools, CLI, or test runners).
- Owns `AppState`: contains `FsService`, `OperationService`, `IndexService`, `WatchService`, `SettingsStore`, and `JobJournal`.
- Exposes typed asynchronous operations:
  - `navigate`, `refresh`, `list_page`
  - `plan_create_folder`, `plan_rename`, `plan_recycle`, `plan_copy_items`, `plan_move_items`
  - `search_query`, `load_preview`
  - `add_favorite`, `remove_favorite`

### 2.3 Win32 System Integration (`crates/explorer-win`)
- **Dedicated COM STA Worker**: Wraps `IFileOperation` inside an isolated Single-Threaded Apartment thread with `CoInitializeEx(COINIT_APARTMENTTHREADED)`.
- **Fail-Closed Recycling**: Prohibits permanent deletion fallbacks (`FOFX_RECYCLEONDELETE`) and validates progress sink status.
- **Single-Instance Lease (`InstanceLease`)**: File lock-based mutual exclusion guaranteeing single process ownership.

### 2.4 Filesystem Engine (`crates/explorer-fs`)
- Enumerates directory entries using native Windows `FindFirstFileW` / `FindNextFileW`.
- Implements zero-allocation natural sorting (`compare_entries`).
- Optimized `FolderSnapshot`: Uses token indexing (`token_map: HashMap<ItemToken, u32>`) and pre-sorted index vectors to achieve ~26 MB idle memory and sub-millisecond pagination.

## 3. Legacy Tauri Archive

The original Tauri 2 + React + TypeScript implementation has been migrated to `legacy/desktop/`.
All core features achieved 100% parity (P01-P26) in the pure native host while achieving an ~87% reduction in RAM consumption.
