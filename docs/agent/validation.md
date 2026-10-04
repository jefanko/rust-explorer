# Rust Explorer Validation Evidence Log

> Milestone entries below are historical command records, not current acceptance evidence. Current status and objective-specific limitations are tracked in [objective-code-review-2026-10-04.md](objective-code-review-2026-10-04.md).

## 2026-10-04 — Five-blocker closure (current)

| Check | Result | Evidence / limits |
|---|---|---|
| cargo test --workspace --locked --offline -- --test-threads=1 --skip test_clipboard_roundtrip --nocapture | PASS, 51 tests | artifacts/reviews/gap-closure-2026-10-04/rust-tests.txt. One clipboard round-trip filtered; serial native fixture operations; synthetic stress tests are not physical performance acceptance. |
| cargo test -p explorer-fs -p explorer-jobs -p explorer-store -p explorer-win -p rust-explorer --lib --locked --offline -- --test-threads=1 --skip test_clipboard_roundtrip --nocapture | PASS, 37 tests | targeted-final-tests.txt; final policy/rename changes, native error-vs-cancel regression and corrupt-state host initialization regression included. |
| Follow-up policy/jobs, planner, path and native create/rename checks | PASS | After the 37-test run: 17 fs/jobs tests, 3 planner tests (identical-name rejection and case-only rename), 3 path tests (unpaired UTF-16 surrogate preservation), and 1 native executor create/rename fixture test passed. The first exact-name filter selected zero tests; rerunning with the correct filter executed the native test successfully. Final format and all-target Clippy passed. |
| Rust format / Clippy all targets -D warnings | PASS | Locked/offline final check; all tests compile, including filtered clipboard round-trip. |
| cargo xtask doctor | PASS | Toolchain checks and bundled FTS5/trigram test. |
| npm run typecheck / test:unit / build:ui | PASS | TypeScript; 8 existing UI helper tests; Vite production bundle. These tests do not prove app interaction/accessibility acceptance. |
| Native create / file-folder rename / copy / move / collision | PASS | Marked fixtures. Folder rename selects native Shell transfer provider with TSF_NORMAL; existing collision target and source preserved. IFileOperation folder rename initially failed 0x80070002; no unchecked filesystem fallback used. |
| Native safe recycle refusal | PASS | Query support returns 0x80070005 before queuing DeleteItem; source intact, native error retained. Earlier permanent-delete offer declined. PreDelete no-recycle flag veto tested with injected callback. |
| Successful recycling | CAPABILITY SKIP | Host volume support not verifiable; native success/recycle-only guarantee on supported filesystem remains OPEN. |
| Clipboard owner | PARTIAL | Null owner rejected before clipboard open; valid main HWND routed/validated in code. Clipboard round-trip NOT RUN because complete format restoration harness is absent; user's clipboard untouched. |
| Instance lease / disk journal | PASS | Separate probe processes denied while owner active, relaunch accepted after release, journal sentinel unchanged; disk journal reopen/recovery, unavailable path and corrupt state fail-closed cases passed. Actual UI focus and shutdown UX NOT RUN. |
| Desktop/installer/real cloud/physical release benchmark | NOT RUN | No app host instance launched, no personal-file mutation or global shell-setting change. Cloud attribute cases tested without hydration; provider journey requires capability-specific fixtures. |

Earlier failures are real and repaired: ancestor identity checks walked ACL-protected unrelated parents; extended Shell parsing and short-path aliases failed; positive DONT_PROCESS_CHILDREN was initially over-conservative; IFileOperation folder rename failed before callbacks. Final policy narrows canonical depth/native identity ancestry; Shell paths strip prefixes without UTF-16 loss; returned output gates that HRESULT's success; directory transfer provider succeeds and collision safety is tested. Early test harness/dev-dependency wiring errors were repaired. Failed native dialogs were declined/canceled, never counted as successful mutations.

Full MVP/release acceptance remains OPEN; the five implementation blockers are closed without upgrading unverified gates to PASS.

## Format
Every entry records: Date (UTC/Local), Milestone, Command executed, Build/Commit, Result (Pass/Fail/Skipped), Evidence summary.

---

### 2026-10-04 Objective Recheck — After Remediation

HEAD: `fa934f0309a0e6930d163ade21d800b1fa2d5a4d` plus uncommitted remediation working tree. Review did not change production source.

| Command | Result | Evidence / limit |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | Workspace formatting. |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS | Rust compile/lint; all-targets does not execute tests. |
| `npm run typecheck` | PASS | `tsc --noEmit`. |
| `cargo test -p explorer-domain -p explorer-store --locked --offline` | PASS, 4 tests | IDs/errors and in-memory settings/journal lifecycle; no native mutations. |
| `cargo test -p explorer-win path::tests --lib --locked --offline` | PASS, 3 tests | Native path helper cases only. |
| `cargo test -p explorer-jobs test_name_validation --lib --locked --offline` | PASS, 1 test | Existing name validation cases; other planner/mutation tests excluded. |
| `cargo test -p explorer-fs listing::tests --lib --locked --offline` | PASS, 2 tests | Synthetic natural sorting/folders first. |
| `cargo test -p explorer-index query::tests --lib --locked --offline` | PASS, 2 tests | In-memory query parsing/ranking/filtering. |
| `./artifacts/reviews/objective-recheck-2026-10-04/run-probe.ps1` | EXECUTED, 2 defects observed | Production sink maps USER_IGNORED to Succeeded and USER_CANCELLED to Failed. Surrogate prefix preserved, trailing-space name rejected, committed outcomes recovered, offline index retained. Probe uses in-memory data, no Shell transfer or clipboard. |
| `git diff --check` | PASS | No whitespace errors. |
| Native mutation/clipboard/app/installer/physical performance acceptance | NOT RUN | No competing executor launched; no personal files or clipboard touched; unmarked mutation harnesses excluded. Release gates remain open. |

Probe source/output/runner: `artifacts/reviews/objective-recheck-2026-10-04/`. Initial standalone compile attempts failed due missing top-level domain rlib and mismatched feature graph; repaired by a unified dependency build. Final probe compiled and ran successfully; this is not a product build failure. Official Win32 documentation was checked for clipboard owner and Shell result-code contracts; source links are in the objective review.

---

### 2026-10-04 Objective Remediation — Earlier Validation

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-04 | Remediation | `cargo fmt --all -- --check` | PASS | Rust workspace formatting verified after the final changes. |
| 2026-10-04 | Remediation | `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS | All workspace targets lint clean; this compiles targets but does not execute tests. |
| 2026-10-04 | Remediation | `cargo check --workspace --locked --offline` | PASS | Workspace Rust code compiled, including callback journaling and interrupted-job outcome recovery. |
| 2026-10-04 | Remediation | `npm run typecheck` | PASS | Desktop TypeScript compiled with `tsc --noEmit`. |
| 2026-10-04 | Remediation | `git diff --check` | PASS | No whitespace errors; Git printed configured LF-to-CRLF normalization notices. |
| 2026-10-04 | Remediation | Runtime tests, mutation harness, clipboard interaction, desktop launch, installer journey, physical 100k benchmark | NOT RUN | No mutation fixtures were executed; existing mutation harnesses do not yet meet the marked-root acceptance contract. No native/clean-machine or performance result is claimed. |

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

---

### M7 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-04 | M7.1 | `cargo test --package explorer-fs --test stress_tests --release` | PASS | 100k entries generated in 43.1 ms; snapshot created in 45.8 ms; initial natural sort + page 1: 103.2 ms (<150 ms budget); subsequent page 2: 76.4 µs (<5 ms budget); token lookup: 200 ns (<50 µs budget). |
| 2026-10-04 | M7.1 | `cargo test --package explorer-index --test stress_tests --release` | PASS | 100k entries indexed into SQLite FTS5 in 14.5s (6,888 items/sec); disk size: 67.93 MiB (budget <= 350 MiB); substring search: 6.86 ms (<50 ms budget); phrase search: 799.7 µs (<50 ms budget). |
| 2026-10-04 | M7.1 | `cargo run -p xtask -- bench release` | PASS | Full automated benchmark runner executed; summary report written to `artifacts/benchmarks/summary.md`. |
| 2026-10-04 | M7.2 | `cargo test --workspace` | PASS | 33 tests across workspace pass; watcher unwatch_all, bounded ingress/dirty directories, and LRU snapshot pruning verified. |
| 2026-10-04 | M7.3 | Security Audit & CSP check | PASS | Content Security Policy reviewed; window label `"main"` bound to capability `"main-capability"`; webview permissions restricted to `"core:default"` with no shell exec plugins exposed. |
| 2026-10-04 | M7.3 | `.\scripts\check.ps1` | PASS | Fast verification suite passes 100% (format, clippy with `-D warnings`, 33 tests, TypeScript strict, Vitest, UI build, doctor). |

---

### M8 Validation Log

| Date | Step | Command | Result | Evidence / Output |
|---|---|---|---|---|
| 2026-10-04 | M8.1 | `.\scripts\package.ps1` | PASS | Built release bundles: NSIS setup (`rust-explorer_0.1.0_x64-setup.exe`, 3.55 MiB), MSI bundle (`rust-explorer_0.1.0_x64_en-US.msi`, 5.15 MiB), and release executable (`rust-explorer.exe`, 14.43 MiB). |
| 2026-10-04 | M8.2 | `npm run test:e2e:native` | PASS | Native E2E test runner verified: clean process launch (PID 40480), `Responding: true`, working set memory 31.35 MiB (<350 MiB budget), isolated AppData state initialization, and distribution bundle existence. |
| 2026-10-04 | M8.2 | `npm run test:unit` | PASS | 8 unit tests in `App.test.tsx` passed, validating path breadcrumbs, case-insensitive filtering, recycle modal prompt formatting, and query constraints. |
| 2026-10-04 | M8.3 | Release Staging & Verification | PASS | Computed SHA-256 hashes (`artifacts/release/SHA256SUMS.txt`), authored release metadata (`artifacts/release/BUILD_METADATA.json`), official release notes (`artifacts/release/RELEASE_NOTES.md`), and full Definition of Done evidence report (`artifacts/release/DEFINITION_OF_DONE_EVIDENCE.md`). |
| 2026-10-04 | M8.3 | `.\scripts\check.ps1` | PASS | Fast verification suite passed 100% (format, clippy with `-D warnings`, 33 tests, TypeScript strict, 8 unit tests, UI build, doctor). |
