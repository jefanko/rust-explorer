# Rust Explorer

A local-first, memory-efficient Windows 11 file manager built with Rust and Tauri 2.

[![Platform](https://img.shields.io/badge/platform-Windows%2011-0078D4?logo=windows&logoColor=white)](https://microsoft.com/windows)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/host-Tauri%202.2-FFC131?logo=tauri&logoColor=black)](https://tauri.app/)
[![CI/CD](https://github.com/rust-explorer/rust-explorer/actions/workflows/ci.yml/badge.svg)](https://github.com/rust-explorer/rust-explorer/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Overview

Rust Explorer is an alternative Windows 11 file manager designed for responsiveness, strict filesystem safety, and predictable daily file management.

All filesystem operations, listing logic, change notifications, and search indexing are implemented directly in Rust. The user interface runs as a thin webview client, communicating with the backend over asynchronous Tauri IPC commands and event streams.

### Core Design Principles
- **Rust-Owned State**: The webview is strictly a view layer. Directory enumeration, sorting, indexing, and mutations are performed exclusively by the Rust core.
- **Dedicated COM STA Worker**: File operations (`IFileOperation`) run on an isolated Win32 Single-Threaded Apartment thread with a dedicated message pump. Long-running I/O never blocks the UI thread.
- **Guarded Recycling**: Deletions enforce `FOFX_RECYCLEONDELETE` and COM progress sink checks. If a filesystem location does not support the Windows Recycle Bin (such as remote UNC shares), the operation aborts immediately rather than silently falling back to permanent deletion.
- **Lossless Path Handling**: Windows paths are represented internally with full UTF-16 fidelity and canonical extended prefixes (`\\?\`), avoiding character conversion losses and standard 260-character `MAX_PATH` limitations.
- **Low Resource Usage**: Uses ~31 MiB of working set memory on startup. Zero external network calls, analytics, or telemetry.

---

## Features

### Directory Browsing
- **Zero-Allocation Natural Sort**: Custom natural sort comparator that orders 100,000 directory entries in ~102 ms without runtime heap allocations.
- **Virtualized Grid**: Powered by `@tanstack/react-virtual` for smooth 60 FPS scrolling through directories of any size, displaying Name, Type, Size, and Date modified.
- **Paged Snapshot Caching**: Folder listings are cached in memory; subsequent 50-item page requests within a 100,000-item folder take ~26 microseconds.
- **LRU Snapshot Eviction**: Capped at 16 folder snapshots maximum, automatically evicting older snapshots during extended sessions.

### Multi-Tab Navigation
- **Independent Tab State**: Each tab maintains its own navigation history (Back, Forward, Up), scroll position, selection, and active search queries.
- **Fluent Design UI**: Clean Windows 11 Fluent-inspired layout with native icons and instant switching between System, Light, and Dark themes.
- **Known Folders and Logical Drives**: Sidebar access to standard Windows Known Folders (Desktop, Downloads, Documents, Pictures, Music, Videos) and detected drive volumes (`C:\`, `D:\`).
- **Persistent Favorites**: Bookmarked folders stored locally in a lightweight SQLite database.

### File Operations
- **Two-Phase Operation Pipeline**: Mutations follow a strict `Plan → Review → Commit → Revalidate → Execute → Reconcile` lifecycle.
- **Durable SQLite Job Journal**: Tracks active and historical operations, recording per-item outcomes and supporting recovery across application restarts.
- **Windows Explorer Interoperability**: Full clipboard copy, cut, and paste compatibility (<kbd>Ctrl+C</kbd>, <kbd>Ctrl+X</kbd>, <kbd>Ctrl+V</kbd>) via standard Windows Shell formats (`CF_HDROP` and `Preferred DropEffect`).
- **Single-Instance Lease**: Enforces a single application executor per user session to avoid conflicting concurrent operations.

### Live Directory Synchronization
- **Real-Time Filesystem Watching**: Monitors active tab directories using Windows `ReadDirectoryChangesW` via `notify`.
- **Event Debouncing**: 150 ms debounce coalescing window with a 500 ms maximum cap prevents UI flooding during high-volume batch writes.
- **Buffer Overflow Protection**: Ingress buffers escalate to full directory rescans if system event queues overflow.

### Indexed and Folder Search
- **Bundled SQLite FTS5 Trigram Index**: Local full-text filename index stored at `%LOCALAPPDATA%\RustExplorer\index.sqlite3`.
- **Query Parser**: Supports plain terms, quoted phrases (`"meeting notes"`), extension filters (`ext:rs`, `ext:exe`, or `.exe`), and item type filters (`type:folder`, `type:file`).
- **Low Query Latency**: Ranked query results (Exact > Prefix > Substring) across 100,000 indexed records in under 10 ms.
- **Dual Scope**: Toggle between searching within the current directory hierarchy or querying indexed roots.

---

## Benchmarks

Measured on a release build using the automated `xtask` benchmark harness against a 100,000-entry dataset:

| Scenario | Target / Budget | Result | Status |
|---|---|---|---|
| Startup Working Set Memory | <= 350 MiB | 31.35 MiB | Passed |
| 100k Directory Generation | Baseline | 43.1 ms | Passed |
| 100k Snapshot Creation | Baseline | 45.8 ms | Passed |
| 100k Natural Sort + Page 1 | <= 150 ms | 103.2 ms | Passed |
| Subsequent 50-Item Page Slice | <= 5 ms | 26.0 µs (0.026 ms) | Passed |
| Item Token Resolution | <= 50 µs | 200 ns | Passed |
| 100k SQLite FTS5 Indexing Rate | >= 1,000 items/s | 6,888 items/s | Passed |
| 100k SQLite Index Disk Footprint | <= 350 MiB | 67.93 MiB | Passed |
| 100k Trigram Query Latency | <= 50 ms | 6.86 ms | Passed |

---

## Workspace Layout

```
rust-explorer/
├── apps/
│   └── desktop/               # Tauri 2 host and React frontend
│       ├── src-tauri/         # IPC command handlers and app lifecycle
│       ├── ui/                # React 18, TypeScript, Tailwind CSS, TanStack Virtual
│       └── tests/             # End-to-end launch and UI smoke tests
│
├── crates/
│   ├── explorer-domain/       # Shared domain types, IDs, and opaque tokens
│   ├── explorer-win/          # Win32 APIs, COM STA worker, Shell IFileOperation, paths
│   ├── explorer-fs/           # Folder service, directory listing, sorting, pagination
│   ├── explorer-index/        # SQLite FTS5 trigram search index, parser, crawler
│   ├── explorer-jobs/         # Operation planner, execution engine, conflict handlers
│   ├── explorer-store/        # Durable SQLite job journal and settings store
│   └── explorer-watch/        # ReadDirectoryChangesW file watcher with coalescing
│
├── xtask/                     # Repository automation CLI (doctor, benchmarks)
└── scripts/                   # Verification (check.ps1) and release packaging (package.ps1)
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| <kbd>Alt</kbd> + <kbd>←</kbd> | Navigate Back |
| <kbd>Alt</kbd> + <kbd>→</kbd> | Navigate Forward |
| <kbd>Alt</kbd> + <kbd>↑</kbd> | Navigate to Parent Directory (Up) |
| <kbd>Ctrl</kbd> + <kbd>L</kbd> | Focus Address Bar |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | Focus Search Bar |
| <kbd>Ctrl</kbd> + <kbd>T</kbd> | Open New Tab |
| <kbd>Ctrl</kbd> + <kbd>W</kbd> | Close Active Tab |
| <kbd>Ctrl</kbd> + <kbd>Tab</kbd> | Switch to Next Tab |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Tab</kbd> | Switch to Previous Tab |
| <kbd>Ctrl</kbd> + <kbd>C</kbd> | Copy Selected Items |
| <kbd>Ctrl</kbd> + <kbd>X</kbd> | Cut Selected Items |
| <kbd>Ctrl</kbd> + <kbd>V</kbd> | Paste Items |
| <kbd>Delete</kbd> | Move Selected Items to Recycle Bin (Guarded) |
| <kbd>F2</kbd> | Rename Selected Item |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>N</kbd> | Create New Folder |
| <kbd>F5</kbd> | Refresh Directory Listing |
| <kbd>Ctrl</kbd> + <kbd>A</kbd> | Select All Items |
| <kbd>Alt</kbd> + <kbd>Enter</kbd> | Open Windows Shell Properties Dialog |
| <kbd>Escape</kbd> | Clear Selection / Dismiss Active Context Menu or Modal |

---

## Prerequisites

Building Rust Explorer requires:

- **Operating System**: Windows 11 (x86-64)
- **Rust Toolchain**: Rust 1.99.0+ (`stable-x86_64-pc-windows-msvc`) with `rustfmt` and `clippy`
  ```powershell
  rustup default stable-x86_64-pc-windows-msvc
  rustup component add rustfmt clippy
  ```
- **C++ Build Tools**: Visual Studio Build Tools 2022 with **"Desktop development with C++"** and the **Windows 11 SDK**
- **Node.js**: Node.js v22 LTS with `npm` (v10+)
- **WebView2**: Microsoft Edge WebView2 Evergreen Runtime

---

## Development

### 1. Setup
```powershell
git clone https://github.com/rust-explorer/rust-explorer.git
cd rust-explorer

# Install frontend dependencies
npm ci

# Validate development environment
cargo xtask doctor
```

### 2. Run in Development Mode
Launches the desktop application with hot reloading for both UI and Rust backend:
```powershell
npm run dev:desktop
```

### 3. Verification Suite
All commits must pass the repository verification suite:
```powershell
# Full verification suite (Rust format, Clippy, tests, Typecheck, UI build, Doctor)
.\scripts\check.ps1
```

Individual checks can be run directly:
```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm run typecheck
npm run test:unit
```

### 4. Run Benchmarks
Runs the 100,000-item directory and SQLite FTS5 search benchmark suite:
```powershell
cargo run -p xtask -- bench release
```

---

## Packaging and Installers

To build the release binary and installation packages:
```powershell
.\scripts\package.ps1
```

Output bundles are generated in `target/release/bundle/`:
- **NSIS Setup Installer**: `target/release/bundle/nsis/rust-explorer_0.1.0_x64-setup.exe` (per-user installation, no administrative elevation required).
- **Windows MSI Package**: `target/release/bundle/msi/rust-explorer_0.1.0_x64_en-US.msi`.
- **Standalone Portable Executable**: `target/release/rust-explorer.exe`.

Release checksums and build metadata are located in `artifacts/release/`.

---

## Scope and Limitations (MVP)

- **Supported Filesystems**: Local NTFS is the primary supported filesystem. ExFAT, FAT32, and SMB/UNC shares operate on a best-effort basis with explicit error reporting.
- **Network Drives and Recycle Bin**: Network UNC shares do not support the Windows Recycle Bin; deletion attempts on UNC paths are aborted to prevent unintended permanent data loss.
- **Search Capabilities**: Filename and basic metadata indexing only. Full-text file content indexing is out of scope for the MVP.
- **Shell Scope**: Operates alongside Windows File Explorer. Does not replace the Windows desktop shell or alter global system file associations.

---

## License

This project is licensed under the [MIT License](LICENSE).
