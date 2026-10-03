# Environment and Toolchain Resolution

## Baseline Environment (Recorded 2026-10-03)
- **Operating System**: Windows 11 Home / Pro (x86-64)
- **Node.js**: v22.20.0
- **npm**: 10.9.3
- **Git**: 2.53.0
- **Rust Toolchain**: 1.99.0 (`stable-x86_64-pc-windows-msvc`)
  - Target: `x86_64-pc-windows-msvc`
  - Components: `rustfmt`, `clippy`
- **WebView2 Evergreen Runtime**: 154.0.4258.53 (installed at `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\154.0.4258.53`)
- **Visual Studio Build Tools**: 2022 (v17.14.41) with `Microsoft.VisualStudio.Workload.VCTools`

## Core Pinned Dependencies

### Rust Crates
- `windows`: ~0.58 (or matching compatible release with Win32 storage, COM, Shell, OLE features)
- `tauri`: ^2.0
- `tauri-build`: ^2.0
- `tokio`: 1.x (features: `rt-multi-thread`, `sync`, `time`, `macros`)
- `rusqlite`: with bundled SQLite and FTS5 enabled
- `notify`: 7.x
- `serde`, `serde_json`: 1.x
- `thiserror`: 2.x
- `tracing`, `tracing-subscriber`: 0.1 / 0.3
- `uuid`: 1.x (v4)
- `tempfile`, `proptest`: latest stable

### Frontend Packages
- `react`, `react-dom`: ^18.3 or ^19.0
- `@tanstack/react-virtual`: ^3.10
- `@tauri-apps/api`: ^2.0
- `@tauri-apps/plugin-shell`: ^2.0
- `vite`: ^6.0
- `typescript`: ^5.6
- `vitest`, `@testing-library/react`: latest stable
