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
- **Indexed search always returned zero rows**: the UI requested page 100 with page size 0, so the ranked result set was offset past the 1,000-row cap. It now requests page 1 with the full result limit, ignores stale `SearchReady` completions, and reports parse errors in the status bar.

### Added
- **Crash and panic diagnostics**: `%LOCALAPPDATA%\RustExplorer\logs\rust-explorer.log` records panics, unhandled exceptions with a symbolised stack (PDB-aware via DbgHelp), and optional per-message breadcrumbs (`RUST_EXPLORER_TRACE=1`).
- **Shell icons** in the file list and sidebar (system small image list, cached per extension).
- **Full dark mode**: persisted System/Light/Dark preference, OS theme following and high-contrast fallback. The title bar, toolbar buttons, edits, list/header, tree, tab strip, status bar, preview pane, context menus and dialogs all render dark. The tab strip and status bar are owner-drawn because Windows 10 has no native dark theme for them.
- **Theme persistence**: the Theme button cycles System / Light / Dark and saves the choice in `settings.sqlite3`; Light forces a light process theme even when the OS is dark.
- **Live folder updates**: the active folder is watched and refreshes are debounced by 250 ms.
- **Deep indexed search**: the search box queries every indexed root with the bundled FTS5 filenames index and renders hits in the main table with Name / Location / Size / Date modified. Double-click or Enter opens a hit; the context menu offers Open containing folder and Copy path. Results are capped at 1,000 with a refine-search hint.
- **Automatic indexing and live results**: if no indexed roots exist, the active folder is registered and crawled automatically, and the search re-runs while roots are scanning so matches appear as they are indexed.
- **Explorer-style behaviours**: address bar opens files and expands `%VARS%`, files launch on double-click/Enter, the context menu "Open" entry works, sort arrows are drawn on the active column, `Delete`/`F2`/`Ctrl+N` flows, empty-folder text, per-monitor DPI resize, minimum window size, and a status bar that reports transient operation results instead of modal dialogs.
- **Column widths fill the view** and the date column shows local time.

### Changed
- The search box now performs deep indexed search across all roots on every text change (150 ms debounce). The previous in-folder live filter is replaced; clearing the box or pressing Escape returns to the browsed folder.
- Hidden entries are filtered during enumeration (`FolderService::navigate_with`), keeping snapshot indices and page offsets aligned.
- Column sorting re-sorts the cached snapshot instead of re-enumerating the directory.
- Toolbar layout no longer clips the Refresh button; navigation buttons use proper arrow glyphs.

### Known gaps
- Internal drag & drop between list rows is not implemented in the native host yet (the planner API exists; the UI does not wire it).
- Deep search indexes filenames only, not file contents.
- Search results are capped at the first 1,000 matches; there is no paging beyond that.
- Preview is disabled while search results are shown.
- Scrollbars keep the system theme; Windows draws them outside the control's dark theme.


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
