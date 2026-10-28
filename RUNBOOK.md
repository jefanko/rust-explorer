# Rust Explorer Runbook

Tanggal: 10 Oktober 2026
Target: Native Win32 Desktop Application

## 1. Quick Start

### Build and Run
To launch the native Win32 application directly:
```powershell
cargo run
```

### Build Optimized Release
To build the standalone release binary:
```powershell
cargo build --release
# Output: target\release\rust-explorer.exe
```

## 2. Test Verification

### Run Full Test Suite
To run all tests across all workspace crates (unit tests, integration tests, and stress tests):
```powershell
cargo test --workspace
```

### Run Native UI Tests Only
To run tests specific to the native host (presenter, history navigation, bounded cache, filters, dispatcher):
```powershell
cargo test -p rust-explorer
```

### Run Health & Environment Diagnostics
```powershell
cargo xtask doctor
```

## 3. Diagnostics & Crash Analysis

The native host is a GUI-subsystem binary: panics and access violations never reach a
console. Everything is written to:

```
%LOCALAPPDATA%\RustExplorer\logs\rust-explorer.log
```

- **Startup banner**: `=== rust-explorer start pid=... ===`, one per run.
- **`[panic]`**: Rust panic with location and a symbolised stack. The window procedure
  catches panics instead of aborting, so the process normally survives.
- **`[crash]`**: unhandled exception (for example `0xc0000005` inside `comctl32.dll`)
  with the exception address and a DbgHelp-resolved stack. Debug builds resolve
  `rust-explorer.exe` frames through the neighbouring PDB.
- **Message breadcrumbs**: set `RUST_EXPLORER_TRACE=1` to append every dispatched
  window message (including `WM_NOTIFY` codes) for reproducing UI-order bugs.

```powershell
$env:RUST_EXPLORER_TRACE = "1"
cargo run
Get-Content "$env:LOCALAPPDATA\RustExplorer\logs\rust-explorer.log" -Tail 80
```

Windows Error Reporting still receives a report after `[crash]` is written, so
`Get-WinEvent -LogName Application | Where-Object Message -like '*rust-explorer*'`
returns the module and offset for comparison.

State lives in the same directory (`state.sqlite3`, `index.sqlite3`) and is guarded by
`instance.lock`; deleting the whole `RustExplorer` folder resets settings, favorites
and the search index.

## 4. Performance & Memory Measurement
To measure the real-time working set and private bytes of the application:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\measure-memory.ps1
```

**Target Thresholds:**
- Idle Working Set: `<= 60 MB` (Current baseline: `~26.5 MB`)
- Private Memory: `<= 20 MB` (Current baseline: `~10.8 MB`)
- 100k Directory Listing: `<= 3.0s` (Current baseline: `~2.76s`)

## 5. Release Packaging

To build and package the production artifacts:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

**Outputs generated in `target\release-artifacts\`:**
- `rust-explorer.exe` (Standalone portable native binary)
- `SHA256SUMS.txt` (UTF-8 clean SHA-256 checksums)
- `BUILD_METADATA.json` (Commit SHA, toolchain versions, build timestamps)

## 6. Working with Legacy Tauri Host

The old Tauri 2 + React codebase is preserved under `legacy/desktop/`.
If you need to run or test the legacy frontend:
```powershell
cd legacy/desktop
npm install
npm run dev:desktop
```
