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

## Completed Milestones
- **Milestone M0 — Environment, Scaffolding, and Baseline Desktop Host**: 100% COMPLETE.
  - All crates compiled and verified.
  - `cargo xtask doctor` passes.
  - `scripts/check.ps1` passes (fmt, clippy, test, typecheck, ui build, doctor).
  - Desktop executable `target\debug\rust-explorer.exe` built and verified launching with Win32 HWND.

## Current Milestone
- **Milestone**: M1 — Safe Browsing
- **Active Tickets**:
  - `M1.1 Lossless Path & Domain Models`: Win32 lossless UTF-16 conversions, opaque item tokens, structured errors.
  - `M1.2 Native Win32 Enumeration`: Wide-character directory enumeration in `explorer-win`.
  - `M1.3 Filesystem Service & Snapshots`: Listing snapshot, sorting, pagination in `explorer-fs`.
  - `M1.4 Tauri IPC Adapter`: Typed navigation and listing IPC (`navigate`, `list_page`, `refresh`).
  - `M1.5 Virtualized Details View`: React + `@tanstack/react-virtual` table.
  - `M1.6 Sidebar Known Folders & Drives`: `SHGetKnownFolderPath` and drive detection.
  - `M1.7 Navigation Address Bar & History`: Breadcrumbs and editable address bar.

## Next Commands
1. Implement `explorer-win/src/enumerate.rs` using `FindFirstFileW` / `FindNextFileW` with wide APIs.
2. Implement snapshot building, natural sorting, and pagination in `explorer-fs`.
3. Wire Tauri commands in `src-tauri/src/commands.rs`.
4. Run `.\scripts\check.ps1`.
