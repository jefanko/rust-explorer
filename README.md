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
cargo clippy --workspace --all-targets --locked -- -D warnings
npm run lint
npm run typecheck

# Tests
cargo test --workspace --locked
npm run test:unit
npm run test:e2e:ui
npm run test:e2e:native

# Benchmarks
cargo xtask bench --profile mvp
```

## Architecture and Safety
Rust Explorer implements strict safety boundaries:
- **Lossless Paths**: Native paths use `PathBuf`/`OsString` and explicit UTF-16 code units; `to_string_lossy()` is strictly avoided for mutation.
- **Opaque Tokens**: Frontend operates solely on session-bound tokens, never raw path strings as mutation authorities.
- **Two-Phase Operations**: Mutations execute via `Plan → Review → Commit → Revalidate → Execute → Reconcile`.
- **Guarded Recycling**: Deletions enforce `FOFX_RECYCLEONDELETE` and verify recycling flags; permanent deletion is never a silent fallback.
- **Dedicated COM STA**: File operations execute on a dedicated STA thread running a Windows message pump.

## License
MIT
