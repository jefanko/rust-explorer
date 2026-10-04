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

## Current Status — MVP Acceptance Open (2026-10-04)

The MVP is **not accepted** against Section 26.2. The milestone notes below are historical implementation notes and do not establish that the acceptance criteria passed. See [objective-code-review-2026-10-04.md](objective-code-review-2026-10-04.md) for baseline findings and remediation status.

Five recheck blockers are closed in production: protected/tree/reparse/placeholder preflight plus source-parent identity and STA revalidation; correct HRESULT classification and native source/output outcome mapping; clipboard main-window HWND owner; per-user instance lease before DB/recovery/executor; and fallible durable startup without in-memory fallback. Plans remain Rust-owned, native paths remain UTF-16, and existing listing/index/watch remediation is preserved.

STA now has bounded dispatch, COM readiness, message pumping and orderly join; the executor retains the instance guard through drain. Second launches only try trusted owner HWND/PID focus and exit, never recover the shared journal. Cross-process fixture exclusion/relaunch passed; actual desktop focus/active-job close UX has not been accepted.

Native create, file/directory rename, copy and move passed on marked fixtures. Directory rename uses the native Shell folder ITransferSource provider with TSF_NORMAL/no overwrite because this host's IFileOperation directory rename fails 0x80070002 before callbacks; the provider result/output is journaled directly, with actual HRESULT retained. Collision preservation passed. This adapter difference from the spec's literal IFileOperation sequence is documented in the objective review.

Successful recycling remains a capability skip: SHQueryRecycleBinW returns 0x80070005 on fixture volumes, so DeleteItem is never submitted and sources remain intact. An early native attempt offered permanent deletion; it was explicitly declined. Never weaken the recycle guard or turn this skip into PASS. Clipboard invalid-owner tests pass, but round-trip was filtered to preserve the user's existing clipboard; safe restoration/Explorer/OLE completion harness is still required.

Validation: workspace run 51 passed with one clipboard round-trip filtered; targeted run 37 passed after safety changes and two added regressions. Later narrow checks passed for policy/jobs (17), planner (3), lossless path handling (3), and native executor create/rename (1). Final format/Clippy, doctor/typecheck, 8 UI unit tests and UI build pass; actual outputs are in artifacts/reviews/gap-closure-2026-10-04 and docs/agent/validation.md. No desktop app launch, installer or physical release performance acceptance was performed. Mutations used marked roots and serialized test execution; no personal files or global shell settings changed.

Next: finish window/session plan binding, progress/cancel/retry, huge-tree UX, safe clipboard interop, native focus/close/installer journey, index resume and physical performance. Full MVP acceptance remains OPEN. Keep .rust-explorer-fixture-root outside listing/index test data subdirectories; protect marked boundaries while allowing controlled child operations. Do not run competing application executors. Existing uncommitted work from previous remediation was preserved; nothing was published or committed.

## Historical Milestone Implementation Notes — Acceptance Not Established

- **Milestone M0 — Environment, Scaffolding, and Baseline Desktop Host**: Implementation recorded; acceptance open.
- **Milestone M1 — Safe Browsing**: Implementation recorded; acceptance open.
  - Native Win32 enumeration (`FindFirstFileExW` / `FindNextFileW`).
  - Windows Known Folders (`SHGetKnownFolderPath`) and Logical Drives (`GetLogicalDriveStringsW`).
  - Thread-safe `FolderService` with generation counter, snapshots, and natural sorting.
  - Interactive React UI with `@tanstack/react-virtual` table, address bar, history navigation, and live Rust IPC.
  - All tests and verification checks passing (`.\scripts\check.ps1`).
- **Milestone M2 — Tabs and Interaction**: Implementation recorded; acceptance open.
  - Independent multi-tab state (location, history, scroll, selection per tab).
  - Full keyboard navigation (`Alt+Arrows`, `Ctrl+L`, `Ctrl+F`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `ArrowUp/Down`, `Shift+Arrows`, `Home/End`, `Enter`, `Alt+Enter`, `Escape`).
  - Multi-selection and app-owned context menu for files, folders, and folder background.
  - Native Windows Shell Properties dialog (`SEE_MASK_INVOKEIDLIST`) and "Open in Windows Explorer".
  - SQLite `SettingsStore` (`kv_store` and `favorites` tables), sidebar Favorites, and Theme switcher.
  - All fast verification suite checks and standalone packaging passing.

- **Milestone M3 — Native Mutation Vertical Slice**: Implementation recorded; acceptance open.
  - Dedicated COM STA worker thread (`StaWorker`) running `IFileOperation`.
  - Job model & state machine: `CommitToken`, immutable `OperationPlan`, `JobExecutor`, and SQLite `JobJournal`.
  - Create Folder & Rename slice with DOS name validation, conflict detection, and cancelable execution.
  - Interactive Job Drawer UI with operation history, badges, and modals.
  - All tests and verification checks passing (`.\scripts\check.ps1`).

- **Milestone M4 — Everyday File Operations**: Implementation recorded; acceptance open.
  - Native Shell Copy & Move operations via `IFileOperation` on `StaWorker`.
  - Guarded Recycle Bin deletion via `ShellProgressSink` with `TSF_DELETE_RECYCLE_IF_POSSIBLE` validation (aborts if recycling is not supported; no silent fallback).
  - Windows Explorer clipboard interoperability via standard `CF_HDROP` and `Preferred DropEffect` (two-way copy/cut/paste).
  - Operation planner with cycle/nesting detection, item count bounds, and UNC recycle prevention.
  - UI toolbar buttons, context menu items, keyboard shortcuts (<kbd>Ctrl+C</kbd>, <kbd>Ctrl+X</kbd>, <kbd>Ctrl+V</kbd>, <kbd>Delete</kbd>), and guarded Recycle confirmation modal.
  - Fast verification suite passed (`.\scripts\check.ps1`, 19 tests, 0 warnings) and standalone executable compiled.

- **Milestone M5 — Live Changes**: Implementation recorded; acceptance open.
  - Native filesystem watching via `notify::RecommendedWatcher` backed by Windows `ReadDirectoryChangesW`.
  - Bounded ingress channel (4,096 capacity) and bounded dirty directories (1,024 capacity) with overflow reconciliation protection.
  - 150 ms debounce coalescing window with 500 ms maximum wait cap to prevent starvation.
  - Reference-counted directory subscriptions per tab, mode upgrades, and zero-leak cleanup on tab close (`unsubscribe_all`).
  - Webview bridge receiving `"watch-notification"` events and automatically refreshing visible directory listings.
  - Fast verification suite passed (`.\scripts\check.ps1`, 25 tests, 0 warnings) and debug desktop binary packaged.

- **Milestone M6 — Indexed Filename Search**: Implementation recorded; acceptance open.
  - Dedicated SQLite FTS5 index at `%LOCALAPPDATA%\RustExplorer\index.sqlite3` with `tokenize='trigram'` and transactional sync triggers.
  - Non-overlapping root limit (maximum 8 roots) enforced by canonical native paths.
  - Bounded BFS metadata crawler using native Win32 `FindFirstFileExW`, batch commit pacing (500 items/100 ms), reparse point non-traversal, and default exclusions (`$Recycle.Bin`, `System Volume Information`, app data).
  - Section 14.2 search query parser supporting terms, `"quoted phrases"`, `ext:`, `type:folder`/`type:file`, 3+ character minimum rule, and trigram ranking (exact > prefix > substring).
  - UI Search scope switcher (`[ Folder | Indexed ]`), 150 ms debounced query execution with request cancellation, virtualized results list, match count capping at 1,000 matches, and sidebar Indexed Roots manager with live status badges.
  - Fast verification suite passed (`.\scripts\check.ps1`, 30 tests, 0 warnings) and standalone desktop binary packaged.

- **Milestone M7 — Hardening and Performance**: Implementation recorded; acceptance open.
  - Zero-allocation ASCII natural sort comparator (reduced 100k sort time from 3.56s to 102.9 ms, 35x speedup).
  - Paged snapshot caching in `FolderSnapshot` (50-item page slice takes 26 µs via `sorted_cache`).
  - LRU snapshot pruning in `FolderService` (capped at 16 snapshots maximum).
  - 100k directory listing benchmark: 45.8 ms snapshot creation, 103.2 ms sort, 200 ns token lookup.
  - 100k SQLite FTS5 benchmark: 6,888 items/sec indexing, 67.93 MiB on disk, <10 ms queries.
  - Automated benchmark runner `xtask bench release` producing `artifacts/benchmarks/summary.md`.
  - Resource limits: bounded watch channels, refcounted watch subscriptions with `unsubscribe_all`, 1000 search match cap.
  - Security audit: strict CSP, window `"main"` capability binding, `"core:default"` webview permissions, `ShellProgressSink` guarded recycle.
  - Fast verification suite passed (`.\scripts\check.ps1`, 33 tests, 0 warnings).

- **Milestone M8 — Installable MVP**: Packaging recorded; clean-machine acceptance open.
  - Per-user NSIS installer built (`target/release/bundle/nsis/rust-explorer_0.1.0_x64-setup.exe`, 3.55 MiB) installing to `%LOCALAPPDATA%\Programs\rust-explorer` with Start Menu shortcut.
  - Windows MSI installer built (`target/release/bundle/msi/rust-explorer_0.1.0_x64_en-US.msi`, 5.15 MiB).
  - Standalone release binary built (`target/release/rust-explorer.exe`, 14.43 MiB).
  - Native E2E test runner (`npm run test:e2e:native`) verified clean desktop launch, responsive window, and 31.35 MiB working set memory.
  - Staged official release deliverables: `artifacts/release/SHA256SUMS.txt`, `artifacts/release/BUILD_METADATA.json`, `artifacts/release/RELEASE_NOTES.md`, and `artifacts/release/DEFINITION_OF_DONE_EVIDENCE.md`.
  - Full fast verification suite passed (`.\scripts\check.ps1`, 33 tests, 0 warnings).

## Project Status: MVP NOT ACCEPTED
Milestones M0–M8 have implementation recorded, but the open findings and missing release evidence in the objective review prevent a claim that the MVP acceptance criteria or performance budgets are complete.

## Future Roadmap (after MVP acceptance)
- **v0.2 — Usability**: Dual pane view, thumbnail previews, internal drag-and-drop, and batch rename.
- **v0.3 — Transfer Control**: Custom CopyFile2 backend, progress graph, and scoped undo research.
- **v0.4 — Search Expansion**: Content extraction in isolated sandbox workers and USN journal indexing.
- **v0.5 — Native Breadth**: OLE external drag/drop, Windows shell namespace browsing, and cloud provider hydration status.
