# Rust Explorer

Rust Explorer is a modern, local-first Windows 11 file manager designed for everyday folder navigation, file selection, launching files, copying, moving, renaming, folder creation, recycling, and fast filename search.

## Technology Stack
- **Engine**: Rust 2024 edition, MSVC toolchain, owning all filesystem logic, identities, scheduling, indexing, and state.
- **Desktop Host**: Tauri 2 with Windows WebView2 Evergreen runtime.
- **Frontend**: React + strict TypeScript + Vite, using `@tanstack/react-virtual` for virtualized rendering.
- **Persistence & Indexing**: Bundled SQLite with FTS5 trigram tokenization.

## Prerequisites
- **OS**: Windows 11 (x86-64)
- **C++ Build Tools**: Visual Studio Build Tools 2022 with "Desktop development with C++" (`Microsoft.VisualStudio.Component.VC.Tools.x86.x64`) and Windows SDK
- **Rust**: Rustup with `stable-x86_64-pc-windows-msvc`, `rustfmt`, and `clippy`
- **Node.js**: Node.js 22 LTS with `npm`
- **WebView2**: Microsoft Edge WebView2 Evergreen Runtime

## Getting Started
```powershell
# Install frontend dependencies
npm ci

# Fetch Rust dependencies
cargo fetch --locked

# Run desktop application in development mode
npm run dev:desktop

# Build desktop application for production
npm run build:desktop
```

## Repository Scripts and Checks
```powershell
# Doctor check
cargo xtask doctor

# Formatting and linting
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
npm run typecheck

# Full CI Fast Verification Suite
.\scripts\check.ps1

# Native E2E Test Suite
npm run test:e2e:native

# 100k Stress Benchmarks
cargo run -p xtask -- bench release

# Build Production Release & NSIS Installer
.\scripts\package.ps1
```

## Packaging & Installer
The release build generates installers in `target/release/bundle/`:
- **NSIS Installer**: `target/release/bundle/nsis/rust-explorer_0.1.0_x64-setup.exe` (per-user installation without requiring administrator privileges).
- **MSI Installer**: `target/release/bundle/msi/rust-explorer_0.1.0_x64_en-US.msi`.
- **Standalone Portable Executable**: `target/release/rust-explorer.exe`.

Release checksums and metadata are staged in `artifacts/release/`.

## Architecture and Safety
Rust Explorer implements strict safety boundaries:
- **Lossless Paths**: Native paths use wide-character representations and explicit UTF-16 code units; lossless conversion preserves Unicode and long paths (`\\?\`).
- **Opaque Tokens**: Frontend operates solely on session-bound tokens (`FolderToken`, `ItemToken`, `CommitToken`), never raw path strings as mutation authorities.
- **Two-Phase Operations**: Mutations execute via `Plan → Review → Commit → Revalidate → Execute → Reconcile`.
- **Guarded Recycling**: Deletions enforce `FOFX_RECYCLEONDELETE` and COM `IFileOperationProgressSink` flag checks; permanent deletion is never a silent fallback.
- **Dedicated COM STA**: File operations execute on a dedicated single-threaded apartment worker (`StaWorker`) running a Windows message pump.
- **Zero-Allocation Natural Sort**: Ultra-fast sorting capable of sorting 100,000 files in ~102 ms, with 26 µs subsequent page slicing via snapshot cache.
- **SQLite FTS5 Trigram Index**: Fast recursive filename search with Section 14.2 grammar support, responsive in under 10 ms for 100k entries.

## Troubleshooting
- **WebView2 Not Found**: Ensure Microsoft Edge WebView2 Evergreen Runtime is installed.
- **Cargo / Rust Not Found**: Ensure `~/.cargo/bin` is in your environment `PATH`.
- **Build Tools Missing**: Install Visual Studio Build Tools 2022 with "Desktop development with C++" and the Windows 11 SDK.

## Supported Limitations (MVP Envelope)
- **Filesystem**: Local NTFS is the primary supported acceptance target. ExFAT and SMB/UNC shares operate on a best-effort basis with explicit error reporting.
- **Recycle Bin**: Remote UNC shares do not support the Windows Recycle Bin; deletion attempts on UNC shares will be safely aborted to protect user data.
- **Search**: Filename and metadata indexing only. File content scanning and thumbnails are non-goals for the MVP.
- **Shell Scope**: Operates alongside Windows File Explorer; does not replace the Windows desktop shell or global system associations.

## License
MIT

