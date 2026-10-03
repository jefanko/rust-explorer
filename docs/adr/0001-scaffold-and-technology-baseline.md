# ADR 0001: Architecture, Scaffolding, and Technology Baseline

## Status
Accepted

## Date
2026-10-03

## Context
Rust Explorer is a modern, local-first Windows 11 file manager intended as a daily alternative to Windows File Explorer for core file browsing, management, and searching. We require high data safety, native Windows fidelity (accurate lossless paths, Recycle Bin recovery, Explorer clipboard interoperability, and Shell operations), fast startup, and responsiveness without locking the UI during large directory enumerations or intensive file operations.

## Decision
1. **Engine**: Rust (edition 2024, stable `x86_64-pc-windows-msvc` toolchain) handles all filesystem policies, metadata snapshots, job scheduling, indexing, and SQLite persistence.
2. **Desktop Shell**: Tauri 2 with Windows WebView2 Evergreen runtime. The frontend communicates with Rust purely via typed IPC commands and bounded event streams.
3. **Frontend**: React + TypeScript (strict mode) with Vite, using `@tanstack/react-virtual` to virtualize large file listings without UI degradation. Plain CSS with Fluent-inspired design tokens is used.
4. **Operation Backend**: `ShellOperationBackend` wrapping `IFileOperation` on a dedicated Single-Threaded Apartment (STA) thread running a Windows message pump.
5. **Database & Search**: Embedded SQLite (`rusqlite`) with FTS5 trigram tokenization for opt-in filename search. Settings and job journal are separated from the index.
6. **Workspace Layout**:
   - `crates/explorer-domain`: Shared IDs, models, and structured errors.
   - `crates/explorer-win`: Native Win32/COM/Shell adapters.
   - `crates/explorer-fs`: Directory listing snapshots and policies.
   - `crates/explorer-jobs`: Job queue, planner, and executor.
   - `crates/explorer-index`: SQLite FTS5 search index and crawler.
   - `crates/explorer-watch`: Filesystem watching via `notify` with dirty reconciliation.
   - `crates/explorer-store`: Persistent settings and job journal.
   - `xtask`: Developer automation (`doctor`, `fixtures`, `bench`, `smoke-installer`).
   - `apps/desktop`: Tauri 2 desktop host and React UI.

## Consequences
- Complete separation between UI and filesystem mutation authority prevents unintended filesystem side effects.
- Direct use of typed `windows` crate bindings isolates unsafe FFI to `explorer-win`.
- Windows WebView2 provides native rendering without requiring heavy Chromium distribution bundles.

## Verification
- `cargo xtask doctor` verifies toolchains and SQLite FTS5 trigram availability.
- Rust and TypeScript builds, typechecks, and tests run clean in CI.
