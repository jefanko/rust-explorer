# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Startup produced an empty window**: the first navigation completed against a dispatcher that still had a null `HWND`, so every completion was silently dropped. The window is now created hidden, bound to the dispatcher, and only then navigated and shown. Address bar, tab title, status bar and first page populate immediately.
- **List view repaint → page-request feedback loop**: `LVN_ODCACHEHINT` re-requested already resident pages, and every page completion re-set the item count and invalidated the whole view. This burned a CPU core and crashed inside `comctl32` (`0xC0000005` in the ListView notification path) within minutes of idle use. Page requests are now deduplicated against the row cache, and item counts are only pushed when they actually change.
- **Process could not close while indexing**: the runtime was dropped on exit while crawl tasks were still running. The window now releases its native watches and shuts the runtime down in the background, so closing is immediate.
- **Modal dialogs re-entered the window state machine**: message boxes, prompt dialogs and popup menus pump messages on the UI thread; re-entrant `WM_APP_WAKEUP`/`WM_NOTIFY`/`WM_COMMAND` handling mutated `WindowState` through a second mutable view. Re-entrant stateful handling is now suppressed while a modal loop is active.
- **Panics aborted the process** across the `extern "system"` boundary. The window procedure is wrapped in `catch_unwind` and records the failure instead of terminating.
- **Hidden/system files were ignored by the toggle**: filtering happened in the view, which desynchronised row indices from page offsets. Hidden entries are now excluded (or included) at enumeration time, and the toggle re-enumerates.
- **Duplicate tab ids** after closing a tab caused request identities to collide; ids now come from a monotonic counter.
- **Failed navigation left the tab claiming the new path**; the view now rolls back to the last successfully enumerated folder.
- **Startup re-crawled every indexed root** (a ~1M-entry, ~1 GB index pegged a core for minutes on every launch). Interrupted/never-completed crawls are still finished, but stale-complete roots are refreshed lazily one at a time.
- **Duplicate indexed roots** are skipped during restore instead of being watched twice.

### Added
- **Crash and panic diagnostics**: `%LOCALAPPDATA%\RustExplorer\logs\rust-explorer.log` records panics, unhandled exceptions with a symbolised stack (PDB-aware via DbgHelp), and optional per-message breadcrumbs (`RUST_EXPLORER_TRACE=1`).
- **Shell icons** in the file list and sidebar (system small image list, cached per extension).
- **Working dark mode**: immersive dark title bar plus native `DarkMode_Explorer` / `DarkMode_ItemsView` control themes for edits, list view, header, tree and tab strip.
- **Live folder updates**: the active folder is watched and refreshes are debounced by 250 ms.
- **Explorer-style behaviours**: address bar opens files and expands `%VARS%`, files launch on double-click/Enter, the context menu "Open" entry works, sort arrows are drawn on the active column, `Delete`/`F2`/`Ctrl+N` flows, empty-folder text, per-monitor DPI resize, minimum window size, and a status bar that reports transient operation results instead of modal dialogs.
- **Column widths fill the view** and the date column shows local time.

### Changed
- Hidden entries are filtered during enumeration (`FolderService::navigate_with`), keeping snapshot indices and page offsets aligned.
- Column sorting re-sorts the cached snapshot instead of re-enumerating the directory.
- Toolbar layout no longer clips the Refresh button; navigation buttons use proper arrow glyphs.

### Known gaps
- Internal drag & drop between list rows is not implemented in the native host yet (the planner API exists; the UI does not wire it).
- Global indexed search reports match counts in the status bar; there is no dedicated results view.
- The column header and status bar remain light while the client area is dark.


### Added
- **Dynamic Release Metadata**: `scripts/package.ps1` dynamically generates `BUILD_METADATA.json` with commit SHA, target triple, compiler/node versions, and exact SHA-256 hashes and sizes calculated directly from staged release binaries.
- **Automated Manifest Version Consistency**: `cargo xtask doctor` now enforces version alignment across `Cargo.toml`, `package.json`, `apps/desktop/package.json`, and `tauri.conf.json`.
- **In-App Drag and Drop**: Supported drag and drop file operations with `plan_transfer` backend planning and validation (guards against dropping folders into themselves or parents).
- **File Preview Pane**: Dedicated Windows 11 Fluent preview pane supporting text files (syntax/lines/encoding, 64 KiB ceiling), images (base64 data rendering for PNG, JPEG, GIF, WEBP, BMP, ICO, SVG), and folder entry counts.
- **Code Signing Pipeline**: Optional and conditional Authenticode signing in `scripts/package.ps1` and CI workflow via `WINDOWS_CERT_PFX_BASE64` and `WINDOWS_CERT_PASSWORD`.
- **Automated Security Audits & Dependabot**: Added `npm audit --omit=dev --audit-level=high` and `cargo audit` checks to CI and configured weekly Dependabot updates for cargo, npm, and GitHub Actions.

### Changed
- **CI/CD Pipeline Hardening**:
  - Restored rolling `latest` tag movement so pre-release builds always update.
  - Tagged releases enforce strict match against manifest versions (`v<version>`).
  - Added least-privilege token permissions (`contents: read` by default, `contents: write` restricted to release job).
- **Modularized UI Architecture**: Split monolithic `App.tsx` into clean, testable components (`TabBar`, `NavToolbar`, `CommandBar`, `Sidebar`, `StatusBar`, `ContextMenu`, `ModalDialog`, `JobsDrawer`, `PreviewPane`) and pure domain utility modules (`format`, `filter`, `paths`, `selection`, `searchQuery`, `recycle`).
- **Comprehensive Unit Tests**: Updated UI test suite to directly test production modules with zero dummy mock functions.

### Removed
- Removed unused `tauri-plugin-shell` dependency and startup registration to preserve capability isolation.
- Removed broken `lint` and `test:e2e:ui` scripts lacking configuration.
- Removed stale committed `artifacts/release/SHA256SUMS.txt` and `BUILD_METADATA.json` in favor of dynamic generation.

---

## [0.1.0] - 2026-10-04

### Added
- Initial MVP release of Rust Explorer.
- High-performance directory listing with zero-allocation natural sort comparator.
- Virtualized file table supporting 100k-entry directories at 60 FPS.
- Native Win32 `IFileOperation` COM STA engine with guarded Recycle Bin deletion.
- Bundled SQLite FTS5 trigram search index.
- Multi-tab navigation with persistent settings and state.
- Live filesystem monitoring with `ReadDirectoryChangesW` and debounce coalescing.
- NSIS and MSI installers for Windows 11 x64.
