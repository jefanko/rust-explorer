# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
