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
