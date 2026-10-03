# Rust Explorer Validation Evidence Log

## Format
Every entry records: Date (UTC/Local), Milestone, Command executed, Build/Commit, Result (Pass/Fail/Skipped), Evidence summary.

---

### M0 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M0.1 | `git --version`, `node --version`, `npm --version` | PASS | Git 2.53.0, Node.js v22.20.0, npm 10.9.3 confirmed. |
| 2026-10-03 | M0.1 | WebView2 check | PASS | Evergreen Runtime 154.0.4258.53 present on disk. |
| 2026-10-03 | M0.1 | `winget install --id Rustlang.Rustup` | PASS | Rustup 1.29.1 installed successfully. |
| 2026-10-03 | M0.1 | `winget install --id Microsoft.VisualStudio.2022.BuildTools` | PASS | Visual Studio Build Tools 2022 (17.14.41) installed with C++ tools. |
| 2026-10-03 | M0.1 | `rustc` link probe via `link.exe` | PASS | `test.exe` compiled and output "MSVC linker works!". |
| 2026-10-03 | M0.2 | `npm install` | PASS | Workspace frontend packages installed (112 packages). |
| 2026-10-03 | M0.2 | `cargo check --workspace` | PASS | All 7 crates + xtask + Tauri host compiled cleanly. |
| 2026-10-03 | M0.3 | `cargo xtask doctor` | PASS | Checked git, node, npm, and probed in-memory SQLite FTS5 trigram virtual table. |
| 2026-10-03 | M0.3 | `cargo fmt --all -- --check` | PASS | Code formatted according to Rust style guidelines. |
| 2026-10-03 | M0.3 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Zero warnings across all targets. |
| 2026-10-03 | M0.3 | `cargo test --workspace` | PASS | Domain tests executed and passed. |
| 2026-10-03 | M0.3 | `npm run typecheck` & `npm run build:ui` | PASS | TypeScript checked cleanly, Vite produced production bundles in `dist/`. |
| 2026-10-03 | M0.3 | `npm run test:unit` | PASS | Vitest suite executed cleanly. |
| 2026-10-03 | M0.4 | `cargo build --package rust-explorer` | PASS | `target\debug\rust-explorer.exe` (18.2 MB) produced. |
| 2026-10-03 | M0.4 | Desktop smoke launch | PASS | Launched process PID 40568, initialized valid Win32 HWND 3869342, closed cleanly. |

---

### M1 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M1.1 | `cargo test --package explorer-win` | PASS | Verified path conversions (`path_to_wide`, `wide_to_path`), extended prefixes (`\\?\`, `\\?\UNC\`), and path safety validation. |
| 2026-10-03 | M1.2 | `cargo test --package explorer-win` | PASS | `enumerate_directory` verified against temporary directory fixture with mixed files, extensions, and subdirectories. |
| 2026-10-03 | M1.2 | `cargo test --package explorer-win` | PASS | `get_standard_known_folders` and `get_logical_drives` verified against live Windows system (Documents, C: drive resolved). |
| 2026-10-03 | M1.3 | `cargo test --package explorer-fs` | PASS | `FolderService` snapshot caching, pagination, and natural sorting (directories-first) verified. |
| 2026-10-03 | M1.4 | `cargo check --package rust-explorer` | PASS | Registered Tauri commands (`bootstrap`, `navigate`, `list_page`, `refresh`, `open_item`). |
| 2026-10-03 | M1.5 | `npm run typecheck`, `npm run build:ui`, `npm run test:unit` | PASS | Virtualized table (@tanstack/react-virtual), address bar, sidebar navigation compiled and passed tests. |
| 2026-10-03 | M1.6 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Clean clippy run across workspace. |
| 2026-10-03 | M1.7 | `.\scripts\check.ps1` | PASS | Complete CI pipeline checks passed 100%. |
| 2026-10-03 | M1.7 | Live Desktop Launch | PASS | Launched process PID 6512 with HWND 4459148. Real directories (Documents, Drives) enumerate and render interactively. |

---

### M2 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M2.1 | `npm run test:unit` | PASS | Unit tests verify tab title extraction, path handling, and multi-selection range math. |
| 2026-10-03 | M2.2 | `cargo test --package explorer-store` | PASS | `test_settings_store_in_memory` verifies SQLite kv_store and favorites persistence. |
| 2026-10-03 | M2.3 | `cargo test --workspace` | PASS | All 12 unit/integration tests across workspace pass cleanly. |
| 2026-10-03 | M2.4 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Zero warnings across all targets with `-D warnings`. |
| 2026-10-03 | M2.5 | `npm run typecheck` | PASS | Strict TypeScript check passed with 0 errors. |
| 2026-10-03 | M2.5 | `npm run build:ui` | PASS | Vite bundle built in 6.69s (`dist/index.html`, CSS, and JS). |
| 2026-10-03 | M2.5 | `cargo xtask doctor` | PASS | All prerequisites (git, node, npm, SQLite FTS5 trigram virtual tables) passed. |
| 2026-10-03 | M2.5 | `.\scripts\check.ps1` | PASS | Complete verification suite passed (format, clippy, tests, typecheck, unit tests, build, doctor). |
| 2026-10-03 | M2.5 | `npx tauri build --debug --no-bundle` | PASS | Standalone executable `target\debug\rust-explorer.exe` produced with offline embedded assets. |

---

### M3 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M3.1 | `cargo test --package explorer-win` | PASS | `test_shell_create_folder_and_rename_in_sta`: Verified `StaWorker` execution of `shell_create_folder` and `shell_rename_item` via Win32 `IFileOperation`. |
| 2026-10-03 | M3.2 | `cargo test --package explorer-store` | PASS | `test_job_journal_lifecycle_and_interrupted_recovery`: Verified `JobJournal` schema, job recording, state transitions, and interrupted recovery on startup. |
| 2026-10-03 | M3.2 | `cargo test --package explorer-jobs` | PASS | `test_name_validation`, `test_plan_create_folder_and_rename`, and `test_executor_create_folder_and_rename_pipeline`: Verified DOS reserved names, immutable plans, single-use commit tokens, and execution pipeline. |
| 2026-10-03 | M3.3 | `cargo check --package rust-explorer` | PASS | Tauri IPC commands compiled cleanly (`plan_create_folder`, `plan_rename`, `commit_plan`, `create_folder`, `rename_item`, `list_jobs`). |
| 2026-10-03 | M3.4 | `npm run typecheck` | PASS | TypeScript strict type checking passed with 0 errors across new operation bridge types and client functions. |
| 2026-10-03 | M3.4 | `npm run test:unit` | PASS | Vitest suite passed 100% (3 tests). |
| 2026-10-03 | M3.4 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Clippy verified with 0 warnings/errors across all crates and targets. |
| 2026-10-03 | M3.4 | `cargo fmt --all -- --check` | PASS | Rust code formatting verified cleanly. |
| 2026-10-03 | M3.4 | `cargo test --workspace` | PASS | All 17 unit and integration tests across the workspace pass. |
| 2026-10-03 | M3.4 | `.\scripts\check.ps1` | PASS | Full verification suite passes 100%. |
| 2026-10-03 | M3.4 | `npx tauri build --debug --no-bundle` | PASS | Successfully compiled debug executable `target\debug\rust-explorer.exe` (includes Job Drawer, Folder creation modal, and Rename modal). |

---

### M4 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M4.1 | `cargo test --package explorer-win` | PASS | `test_shell_copy_move_and_recycle_in_sta`: Verified `IFileOperation` copy, move, and recycle in dedicated `StaWorker`. |
| 2026-10-03 | M4.2 | `cargo test --package explorer-win` | PASS | `ShellProgressSink` verified: checks `TSF_DELETE_RECYCLE_IF_POSSIBLE` and aborts if recycling is not supported to prevent silent permanent deletion. |
| 2026-10-03 | M4.3 | `cargo test --package explorer-jobs` | PASS | `test_plan_copy_move_and_recycle` & `test_executor_copy_move_and_recycle_pipeline`: Verified cycle prevention, UNC path rejection for recycling, and state logging in SQLite `JobJournal`. |
| 2026-10-03 | M4.4 | `cargo test --package explorer-win` | PASS | `test_clipboard_roundtrip`: Verified Windows Explorer interoperability with `CF_HDROP` and `Preferred DropEffect` for copy (`1`) and move (`2`). |
| 2026-10-03 | M4.4 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Zero warnings across all workspace targets with `-D warnings`. |
| 2026-10-03 | M4.4 | `cargo fmt --all -- --check` | PASS | Rust code formatting verified cleanly. |
| 2026-10-03 | M4.4 | `cargo test --workspace` | PASS | All 19 unit and integration tests across 7 crates pass 100%. |
| 2026-10-03 | M4.4 | `npm run typecheck` & `npm run test:unit` | PASS | TypeScript strict type checking passed, Vitest unit tests passed (3 tests). |
| 2026-10-03 | M4.4 | `npm run build:ui` | PASS | Vite production build compiled (`dist/index.html`, CSS, JS bundles). |
| 2026-10-03 | M4.4 | `cargo xtask doctor` | PASS | Toolchain check and SQLite FTS5 trigram virtual table check passed. |
| 2026-10-03 | M4.4 | `.\scripts\check.ps1` | PASS | Complete CI fast verification suite passed 100%. |
| 2026-10-03 | M4.4 | `npx tauri build --debug --no-bundle` | PASS | Standalone executable `target\debug\rust-explorer.exe` compiled cleanly with copy, move, recycle modal, and clipboard features. |

---

### M5 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M5.1 | `cargo test --package explorer-watch` | PASS | `test_live_filesystem_modification_emits_notification`: Verified live Windows OS change events via `notify::RecommendedWatcher`. |
| 2026-10-03 | M5.1 | `cargo test --package explorer-watch` | PASS | `test_watch_service_subscription_lifecycle_and_refcounts`: Reference counted subscriptions per path, deduplication, and mode upgrade verified. |
| 2026-10-03 | M5.1 | `cargo test --package explorer-watch` | PASS | `test_unsubscribe_all_cleans_tab_watches`: Verified no handle or watch leaks on tab close. |
| 2026-10-03 | M5.2 | `cargo test --package explorer-watch` | PASS | `test_event_coalescing_and_deduplication`: 150 ms debounce coalescing window and deduplication within bursts verified. |
| 2026-10-03 | M5.2 | `cargo test --package explorer-watch` | PASS | `test_overflow_event_clears_changes`: Verified `MAX_DIRTY_DIRS` (1024) bounding and overflow transition for full rescan reconciliation. |
| 2026-10-03 | M5.2 | `cargo test --package explorer-watch` | PASS | `test_reconciliation_lifecycle_and_backoff`: Verified failure backoff (2s to 30s) and freshness recovery. |
| 2026-10-03 | M5.2 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Zero warnings across all targets with `-D warnings`. |
| 2026-10-03 | M5.2 | `cargo test --workspace` | PASS | All 25 unit and integration tests across 7 crates pass 100%. |
| 2026-10-03 | M5.2 | `npm run typecheck` & `npm run test:unit` | PASS | Strict TypeScript type checking passed with 0 errors; Vitest unit tests passed. |
| 2026-10-03 | M5.2 | `npm run build:ui` | PASS | Vite production build compiled (`dist/index.html`, CSS, JS bundles). |
| 2026-10-03 | M5.2 | `.\scripts\check.ps1` | PASS | Full fast verification suite passed 100%. |
| 2026-10-03 | M5.2 | `npx tauri build --debug --no-bundle` | PASS | Standalone executable `target\debug\rust-explorer.exe` compiled cleanly with live watching enabled. |

---

### M6 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-03 | M6.1 | `cargo test --package explorer-index` | PASS | `test_index_db_schema_roots_and_triggers`: FTS5 schema, trigram virtual table, non-overlapping root limits (max 8), and sync triggers verified. |
| 2026-10-03 | M6.2 | `cargo test --package explorer-index` | PASS | `test_crawler_indexes_files_and_skips_exclusions`: Verified bounded BFS crawler, exclusion skipping (`$Recycle.Bin`, system files), reparse point non-traversal, and batch commit pacing (500 items/100 ms). |
| 2026-10-03 | M6.3 | `cargo test --package explorer-index` | PASS | `test_parse_query_valid_and_invalid` & `test_execute_search_ranking_and_filters`: Section 14.2 grammar parsed (terms, phrases, `ext:`, `type:`), 3-char minimum enforced, trigram ranking verified. |
| 2026-10-03 | M6.3 | `cargo test --package explorer-index` | PASS | `test_index_service_add_search_remove`: Thread-safe IndexService coordinates root additions, background worker crawls, querying, and removals. |
| 2026-10-03 | M6.4 | `npm run test:unit` | PASS | Vitest suite passes 100% (5 tests) including query constraint validation and parent directory extraction. |
| 2026-10-03 | M6.4 | `npm run typecheck` | PASS | Strict TypeScript type checking passes with 0 errors across search components and DTOs. |
| 2026-10-03 | M6.4 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Zero warnings across all crates and targets with `-D warnings`. |
| 2026-10-03 | M6.4 | `cargo test --workspace` | PASS | All 30 unit and integration tests across 7 crates pass 100%. |
| 2026-10-03 | M6.4 | `npm run build:ui` | PASS | Vite production build compiled (`dist/index.html`, CSS, JS bundles). |
| 2026-10-03 | M6.4 | `cargo xtask doctor` | PASS | Verified git, node, npm, and bundled SQLite FTS5 trigram capability. |
| 2026-10-03 | M6.4 | `.\scripts\check.ps1` | PASS | Full fast verification suite passes 100%. |
| 2026-10-03 | M6.4 | `cargo build --package rust-explorer` | PASS | Standalone executable `target\debug\rust-explorer.exe` compiled cleanly with indexed search and UI scope switcher. |



