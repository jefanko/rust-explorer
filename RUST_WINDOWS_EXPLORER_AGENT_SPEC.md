# Rust Explorer — Agent-Ready Windows File Manager Specification

**Specification version:** 1.0  
**Prepared:** 2026-10-03  
**Working product name:** Rust Explorer  
**Repository name:** `rust-explorer`  
**Primary platform:** Windows 11, x86-64, standard user account  
**Implementation:** Rust engine + Tauri 2 desktop host + React/TypeScript interface  
**Target outcome:** an installable, usable MVP; implementation and validation continue until the acceptance gates in this document pass.

**Quick navigation:** [Bootstrap](#4-prerequisites-and-bootstrap-procedure) · [Architecture](#6-architecture-and-dependency-boundaries) · [Copy engine](#13-copy-engine-mvp-native-backend-and-later-custom-backend) · [Search](#14-indexing-and-search) · [Tests](#19-tests-and-acceptance-fixtures) · [Milestones](#24-milestones-and-phased-roadmap) · [Agent loop](#25-autonomous-agent-execution-loop) · [Definition of done](#26-definition-of-done)

## 1. Mission and execution contract

Build a modern, local-first Windows file manager that users can use for everyday folder navigation, file selection, opening files, copying, moving, renaming, creating folders, recycling files, and finding files by name. It launches alongside Windows File Explorer. “Replacement” means an alternative daily file-management application; it does not mean replacing the Windows desktop shell or achieving all Explorer features in the MVP.

The autonomous coding agent must implement working vertical slices, run checks, repair failures, and maintain evidence of progress. This document specifies the project; it does not assert that the application, benchmarks, or installer already exist.

Interpret requirements as follows:

- **MUST:** required for MVP acceptance unless explicitly labeled a later phase.
- **SHOULD:** implement unless a documented constraint makes it unreasonable.
- **LATER:** record in the roadmap; do not make MVP completion depend on it.
- If a library API differs from examples here, consult its official documentation, adapt the implementation, and record the exact resolved version. Preserve the intended behavior.
- Prioritize filesystem correctness and preservation of user data, then responsiveness, then feature breadth and visual polish.
- Do not substitute mocks, TODOs, disabled tests, or a browser-only demo for a working Windows application.
- Do not ask the project owner to make routine implementation decisions already resolved here. Ask only for required credentials, unavailable privileges, genuinely ambiguous product requirements, or an irreversible external action beyond the implementation scope.

### 1.1 Required MVP user journey

A standard user installs the application without installing Rust or Node.js, opens it, browses Documents, opens a second tab, selects several files, copies them to another folder, handles an existing-name conflict through Windows, renames a file, creates a folder, moves a file, recycles an unwanted file, searches an explicitly indexed folder, opens a result in its parent folder, and closes/reopens the application with tabs and preferences restored. External file changes appear without restarting. Failed or canceled operations report what happened and never pretend to have completed.

### 1.2 Supported MVP envelope

| Area | MVP contract |
|---|---|
| OS | Windows 11 on supported x86-64 installations; record tested build numbers |
| Permissions | Standard user; application manifest requests `asInvoker` |
| Filesystem | Local NTFS is the required acceptance target |
| Locations | Drive roots, ordinary filesystem folders, known user folders, removable drive discovery |
| UNC/SMB and exFAT | Best-effort navigation and operations with explicit errors; release notes state test coverage |
| Search | Local filename and metadata index for user-selected roots; no content extraction |
| Links | Detect and display reparse points; avoid automatic recursive traversal; conservative mutation policy |
| Cloud placeholders | Display attributes/status where available; do not download content during enumeration/indexing |
| Special Shell folders | Opening external Windows UI is allowed; internal virtual-namespace browsing is later |
| Large directories | Virtualized interface and streamed/paged Rust listing; 100,000-entry stress fixture |

## 2. Product scope and non-goals

### 2.1 Required features

1. Single main window, multiple folder tabs, Back/Forward/Up navigation, breadcrumbs, editable address bar.
2. Sidebar with known folders, local drives, and persistent favorites.
3. Details view with name, type, size, and modified time; selectable columns may follow later.
4. Stable sorting, folders first, hidden-file toggle, multi-selection, keyboard support.
5. Open a file with its Windows association, open its parent folder, and show native properties.
6. Create folder, rename, copy, move, cut/copy/paste, and recycle through the native operation backend.
7. Job queue with progress, cancellation request, per-item outcomes, and clear partial-failure states.
8. Folder watching, explicit refresh, and reconciliation after missed events.
9. Current-folder filename filter plus indexed recursive filename search in chosen roots.
10. Light/dark/system theme, readable focus indicators, screen-reader labels, high-DPI behavior.
11. Persist tabs, favorites, theme, column widths, and indexing choices; recover safely after a crash.
12. Per-user installer, reproducible build instructions, automated checks, benchmark reports, and a clean-machine smoke-test report.

### 2.2 Explicit MVP non-goals

- Replacing `explorer.exe`, taskbar, desktop, Windows logon shell, or global folder associations.
- Installing a service, kernel driver, filesystem filter, or privileged always-running indexer.
- Full Shell namespace support: Control Panel, MTP phones, Libraries, Recycle Bin browsing, ZIP folders, network discovery, and third-party namespace extensions.
- Hosting arbitrary third-party context-menu extensions in the application.
- Tabs with terminals, file preview parsers, PDF/Office content search, media transcoding, archive editing, or remote transfer protocols.
- Cross-platform releases, ARM64 release, dual-pane mode, plugin marketplace, cloud synchronization, AI features, telemetry, or automatic update downloads.
- A custom byte-copy implementation, guaranteed atomic multi-file transactions, app-managed undo, copy pause/resume, or automatic crash resumption.
- Full preservation of every possible filesystem feature across unlike volumes.
- Guaranteed sub-millisecond whole-disk search or any unmeasured performance claim.

Do not quietly reintroduce these features while implementing the MVP. Create backlog issues instead.

## 3. Technology decisions

These choices are fixed for the MVP. Revisit only if a blocking incompatibility is demonstrated by a small reproducible experiment and an architecture decision record (ADR).

| Layer | Choice | Purpose and constraint |
|---|---|---|
| Engine language | Rust, edition 2024, stable MSVC toolchain | Own filesystem logic, identities, scheduling, search, persistence, and errors |
| Native bindings | Microsoft `windows` crate | Typed Win32/COM APIs, isolated behind safe Rust adapters |
| Desktop host | Tauri 2 | Window lifecycle, IPC, packaged local assets, Windows WebView2 |
| Frontend | React + strict TypeScript + Vite | UI/state presentation only; no direct filesystem access |
| Large lists | `@tanstack/react-virtual` | Render visible rows with bounded overscan |
| UI styling | CSS variables and plain CSS | Fluent-inspired spacing and colors; avoid a large widget dependency initially |
| Runtime | Tokio with explicitly bounded blocking workers | Orchestration and cancellation; never run COM operations in arbitrary tasks |
| Persistence/index | `rusqlite` with bundled SQLite | One writer actor; a small read pool; SQLite FTS5 trigram filename index |
| Watching | `notify` behind a project-owned adapter | Windows notifications plus explicit reconciliation and polling fallback |
| Serialization | `serde`, `serde_json` | Versioned, typed command/event DTOs |
| Errors/logging | `thiserror`, `tracing`, rolling local log sink | Structured errors; paths redacted by default |
| IDs | UUID v4 or equivalent cryptographically random identifiers | Jobs, plans, sessions; not filesystem identities |
| Rust tests | Standard test runner + `tempfile` + `proptest` | Behavior, fault injection, path invariants |
| Rust microbenchmarks | Criterion | Algorithm/query comparisons; not desktop startup measurement |
| UI tests | Vitest + React Testing Library | Interaction and rendering with a deterministic bridge |
| Browser UI E2E | Playwright | Frontend behavior only; explicitly separate from native validation |
| Native E2E | Tauri driver + WebdriverIO + matching Microsoft Edge Driver | Drive the actual Windows app and its IPC |
| Packaging | Tauri NSIS, per-user install | One installer format for MVP; MSI/MSIX later |
| License | MIT for project code | Check dependency and asset licenses before release |

The interface is rendered by WebView2; it is not a WinUI control tree and this is not an all-Rust UI. “Primarily Rust” means all filesystem, indexing, native integration, job execution, and persistence live in Rust. Do not move these into JavaScript to avoid implementation work. Tauri uses WebView2 on Windows; development requires the MSVC build environment. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

Microsoft maintains Rust bindings for Windows APIs. Use their generated signatures for the pinned crate version; do not copy raw C layouts from memory. [windows-rs](https://github.com/microsoft/windows-rs)

### 3.1 Version resolution and reproducibility

At M0, resolve current compatible stable releases from official registries, record them in `docs/environment.md`, and commit:

- An exact Rust release in `rust-toolchain.toml`, with `rustfmt`, `clippy`, and target `x86_64-pc-windows-msvc`.
- `Cargo.lock` for the full workspace.
- Exact direct frontend versions and root `package-lock.json`; a pinned `packageManager` entry and recorded Node LTS major/version.
- Exact Tauri 2 CLI/API versions compatible with the Rust host. All Tauri packages/plugins must be v2-compatible.
- An MSRV in workspace package metadata, verified against the resolved dependencies.
- Pinned tool versions for `tauri-driver`, `cargo-audit`, `cargo-deny`, and benchmark tools when scripts depend on them.

Use stable Rust, not nightly. Updates occur in a dedicated change with lockfile review and checks. Never claim a version is current solely because it appears in this specification.

### 3.2 Native dependency feature plan

Enable only the `windows` crate features needed by implemented adapters. Initial candidates are `Win32_Foundation`, `Win32_Storage_FileSystem`, `Win32_System_Com`, `Win32_System_Ole`, `Win32_System_DataExchange`, `Win32_System_Memory`, `Win32_System_Threading`, `Win32_UI_Shell`, and `Win32_UI_WindowsAndMessaging`; add security/GDI/structured-storage features only when required by exact generated signatures. Verify names in the pinned crate. Do not enable every Windows feature or import native types into domain DTOs.

Enable SQLite bundling and prove FTS5 availability; avoid a dependency on a separately installed SQLite DLL. Tokio features should match actual needs (`rt-multi-thread`, synchronization, time, macros as used); most filesystem work uses controlled blocking/native adapters rather than unbounded async filesystem calls.

## 4. Prerequisites and bootstrap procedure

### 4.1 Inspect before installation

Work inside the repository. Preserve unrelated files and uncommitted changes. Read any applicable `AGENTS.md`. Inspect existing toolchains first:

```powershell
Get-Command git, rustup, cargo, node, npm, winget -ErrorAction SilentlyContinue
git --version
rustup show
cargo --version
node --version
npm --version
```

The agent may install missing development prerequisites under the environment's approval policy. Do not reinstall working software or silently alter global settings. If system installation is blocked, record the exact missing dependency and continue portable/frontend work; Windows native acceptance remains unverified until a suitable machine is available.

Required tools:

1. Git.
2. Current supported Visual Studio Build Tools with **Desktop development with C++**, x64/x86 MSVC tools, and a compatible Windows SDK.
3. Rustup and a stable `x86_64-pc-windows-msvc` Rust toolchain.
4. Node.js LTS with npm.
5. Microsoft Edge WebView2 Evergreen Runtime.
6. Microsoft Edge Driver matching the installed browser/runtime setup used by native tests, plus `tauri-driver`.

Use official installers or verified WinGet package IDs. Example candidates for tools are `Git.Git`, `Rustlang.Rustup`, `OpenJS.NodeJS.LTS`, and `Microsoft.EdgeWebView2Runtime`; verify availability before installation:

```powershell
winget show --id Rustlang.Rustup --exact
winget install --id Rustlang.Rustup --exact --source winget
winget show --id OpenJS.NodeJS.LTS --exact
winget install --id OpenJS.NodeJS.LTS --exact --source winget
rustup toolchain install stable-x86_64-pc-windows-msvc --component rustfmt --component clippy
rustup target add x86_64-pc-windows-msvc --toolchain stable-x86_64-pc-windows-msvc
```

Only run installation commands for missing tools. Resolve the Build Tools package/version currently supported on the target machine; verify its installer options and select the C++ workload. Do not assume the base Build Tools package includes C++ by default. Restart the terminal if PATH changes. Use `vswhere` to detect the C++ toolchain and SDK, then compile a tiny Rust Windows program to prove linking works. Do not pipe downloaded scripts directly into a shell.

### 4.2 Scaffold in a deterministic layout

Initialize the repository if needed. Create the structure in section 5. Generate Tauri boilerplate using the current official create-Tauri tool in an empty staging directory, selecting React, TypeScript, npm, Tauri 2, and Windows desktop. Inspect `--help` for that pinned tool before using noninteractive flags. Move only the needed boilerplate into the prescribed layout, or create equivalent files manually. Do not leave a nested Git repository or duplicate lockfiles.

Use a root Rust workspace and npm workspace. Root Rust members are `crates/*`, `apps/desktop/src-tauri`, and `xtask`; use resolver `3`. Root npm workspaces contain `apps/desktop`, whose package name is `@rust-explorer/desktop`.

Desktop conventions:

- `apps/desktop/package.json` contains Vite, tests, and the Tauri CLI.
- Vite root is `apps/desktop/ui`; output is `apps/desktop/dist`, with port 1420 and `strictPort: true`.
- Run the Tauri CLI from `apps/desktop`, where it discovers `src-tauri`.
- Tauri `devUrl` is `http://localhost:1420`; `frontendDist` is `../dist` relative to `src-tauri`.
- `beforeDevCommand` invokes desktop `npm run dev`; `beforeBuildCommand` invokes desktop `npm run build`.
- `tauri.conf.json` uses a stable development identifier such as `io.rustexplorer.desktop`, standard native window decorations, and explicit capabilities.
- Produce executable `rust-explorer.exe`; `xtask` locates its actual Cargo target output rather than hardcoding a nested target directory.
- Commit a real icon with project-owned artwork, not a placeholder referencing a missing file.

### 4.3 Commands the agent must make available

The following are the required repository command contract. Implement missing scripts at M0; later milestones extend their checks. Commands must return nonzero on failure and propagate child process exit codes.

```powershell
# Dependency installation after lockfiles exist
npm ci
cargo fetch --locked

# Fast verification
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run typecheck
npm run lint
npm run test:unit
npm run build:ui

# Launch and build the real desktop application
npm run dev:desktop
npm run build:desktop

# Native tests, generated fixtures, and benchmarks
cargo xtask doctor
cargo xtask fixtures --profile small
npm run test:e2e:ui
npm run test:e2e:native
cargo xtask bench --profile mvp
cargo xtask smoke-installer --installer <absolute-installer-path>

# Dependency checks after the pinned tools are installed
cargo audit
cargo deny check
npm audit --audit-level=high
```

Define `.cargo/config.toml` alias `xtask = "run --package xtask --"`. Root npm scripts delegate to the desktop workspace. `test:unit` must use a non-watch runner. `build:desktop` runs Tauri build with NSIS; it does not recursively call itself from `beforeBuildCommand`. The smoke-installer command supports an interactive mode for native dialogs and never installs over an unrelated product.

M0 may implement empty test scaffolding only long enough to establish the toolchain. Subsequent milestone gates require the meaningful tests specified below.

## 5. Repository structure

```text
rust-explorer/
├── AGENTS.md
├── README.md
├── LICENSE
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── package.json
├── package-lock.json
├── .cargo/config.toml
├── .gitignore
├── .github/workflows/
│   ├── ci.yml
│   ├── native-e2e.yml
│   └── release.yml
├── apps/desktop/
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── eslint.config.js
│   ├── ui/
│   │   ├── index.html
│   │   └── src/
│   │       ├── app/
│   │       ├── components/
│   │       ├── features/{navigation,listing,selection,jobs,search,settings}/
│   │       ├── bridge/{client.ts,types.ts,events.ts}
│   │       ├── styles/
│   │       └── test/
│   └── src-tauri/
│       ├── Cargo.toml
│       ├── build.rs
│       ├── tauri.conf.json
│       ├── capabilities/main.json
│       ├── permissions/
│       ├── icons/
│       └── src/{main.rs,lib.rs,commands.rs,events.rs,state.rs}
├── crates/
│   ├── explorer-domain/src/{lib.rs,ids.rs,models.rs,errors.rs,operations.rs}
│   ├── explorer-win/src/{lib.rs,path.rs,handles.rs,enumerate.rs,identity.rs,
│   │                     com.rs,shell.rs,clipboard.rs,known_folders.rs}
│   ├── explorer-fs/src/{lib.rs,listing.rs,snapshots.rs,metadata.rs,policy.rs}
│   ├── explorer-jobs/src/{lib.rs,planner.rs,queue.rs,executor.rs,
│   │                      shell_backend.rs,progress.rs,recovery.rs}
│   ├── explorer-index/src/{lib.rs,db.rs,crawl.rs,query.rs,reconcile.rs}
│   ├── explorer-watch/src/{lib.rs,adapter.rs,coalesce.rs,reconcile.rs}
│   └── explorer-store/src/{lib.rs,settings.rs,journal.rs,migrations.rs}
├── xtask/src/{main.rs,doctor.rs,fixtures.rs,bench.rs,smoke.rs}
├── tests/
│   ├── integration/
│   ├── fixtures/README.md
│   ├── ui-e2e/
│   └── native-e2e/
├── benches/
│   └── README.md
├── scripts/
│   ├── bootstrap.ps1
│   ├── check.ps1
│   └── native-test.ps1
└── docs/
    ├── architecture.md
    ├── environment.md
    ├── security.md
    ├── testing.md
    ├── benchmarks.md
    ├── release.md
    ├── known-limitations.md
    ├── adr/
    └── agent/{backlog.md,progress.md,validation.md,handoff.md}
```

Keep actual integration tests in their owning crate's `tests/` directory or register an explicit test target. A root `tests/integration/` directory in a virtual workspace does not automatically run. Criterion benches likewise require `[[bench]]` targets in the owning crate. Keep generated fixtures/reports under ignored `work/` and `artifacts/`; do not commit massive datasets.

## 6. Architecture and dependency boundaries

```text
React UI
  │ typed commands / bounded event streams
  ▼
Tauri adapter: validation, window authority, DTO conversion
  ├── Folder service ────── explorer-fs ─────── explorer-win
  ├── Operation service ─── explorer-jobs ───── Shell STA worker
  ├── Search service ────── explorer-index ──── SQLite writer/readers
  ├── Watch service ─────── explorer-watch ──── notify / polling
  └── Settings service ──── explorer-store
                    All use explorer-domain
```

### 6.1 Module responsibilities

| Module | Owns | Must not own |
|---|---|---|
| `explorer-domain` | IDs, DTO-independent entities, operation state machine, error codes, trait contracts | Tauri, React, Win32 pointers, database connections |
| `explorer-win` | Windows paths, owned handles, COM lifetime, enumeration primitives, identities, native Shell/clipboard calls | Product UI state, index ranking, job scheduling |
| `explorer-fs` | Folder snapshots, sort/page/filter, metadata budgets, path policy | SQLite indexing or UI rendering |
| `explorer-jobs` | Plan validation, queue, executor interface, journal transitions, outcome aggregation | Direct frontend invocation, general directory UI |
| `explorer-index` | Crawl lifecycle, schema, query parser, FTS maintenance, scoped reconciliation | Copy execution or arbitrary filesystem writes |
| `explorer-watch` | Watch subscriptions, coalescing, dirty flags, reconciliation requests | Deciding a destructive user action |
| `explorer-store` | Versioned preferences, journal persistence, migrations and recovery | Native API calls except storage filesystem access through adapters |
| Tauri host | Compose services, IPC access checks, per-window sessions, lifecycle | Large business-logic implementations |
| Frontend | Render state, selection, user intent, keyboard/focus handling | Path identity decisions or direct filesystem mutation |

Avoid circular crate dependencies. Store serialization interfaces must not require the jobs crate; both can use domain types. Traits at boundaries permit deterministic fakes for tests and fault injection. Do not create a generic framework beyond the requirements.

### 6.2 Threading and lifetime

- Main/UI thread: window operations and event dispatch only. Never enumerate a folder, walk a tree, query a database, or wait for `PerformOperations` here.
- Tokio: schedule services, bounded channels, cancellation tokens, command completion.
- Filesystem workers: at most 4 local blocking tasks initially; a separate budget of 2 for slow/untrusted locations. Configuration can lower limits.
- SQLite: one writer thread owns its connection; at most 2 reader connections on dedicated blocking workers. No shared mutable connection behind an async mutex.
- File operation STA: one dedicated, long-lived OS thread initialized with `CoInitializeEx(COINIT_APARTMENTTHREADED)` and an appropriate Windows message pump. Own all operation COM objects on it and uninitialize COM on that same thread.
- Shell metadata/clipboard STA: a separate actor for known folders, properties, associations, and clipboard so a long copy does not monopolize these requests. Marshal or recreate COM interfaces correctly; never send raw interfaces across threads under an invented `Send` implementation.
- Serialize mutations in MVP. Queue limit: 32 jobs. Operations in different tabs share the same executor.
- Use RAII for handles, advised sinks, allocations, and subscriptions. All channels have defined capacities, overflow policies, and shutdown behavior.

`IFileOperation` requires STA use. Create the object and its Shell items on the dedicated STA rather than on a Tokio worker. [Microsoft IFileOperation documentation](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation)

On shutdown, stop accepting jobs, cancel background crawls, drain persistence, and release watchers. If a mutation is active, show “Wait” and “Request cancellation”; do not silently terminate or roll back completed items. If Windows terminates the process, recovery must handle an interrupted journal.

Use one application instance per user/application identity for MVP, through a reviewed Tauri single-instance plugin or a correctly scoped native mutex/forwarding mechanism. A second launch focuses the existing window and forwards only validated navigation intent. It must not create another job executor against the same state directory. Test this behavior and clean shutdown/relaunch; do not acquire a machine-global privileged mutex.

## 7. Windows paths, identities, and filesystem policy

This section is a correctness requirement, not optional polish.

### 7.1 Native representation

- Internal paths use `PathBuf`/`OsString` and explicit UTF-16 conversion for `W` APIs.
- Keep native paths lossless. Never use `to_string_lossy()` to reconstruct a path for opening, copying, deleting, or uniqueness checks.
- Persist native UTF-16 code units as little-endian BLOBs with a documented encoding version. A lossy display string may be stored separately for search/presentation.
- UI rows carry opaque item tokens. Frontend display paths and normalized search strings are not mutation authorities.
- Manually typed navigation accepts normal Unicode Windows paths. Existing names with unpaired UTF-16 units remain operable through native tokens even if the UI shows an escaped/lossy label.
- Reject embedded NULs, device namespaces such as `\\.\`, raw `\Device\` paths, drive-relative input such as `C:folder`, and ADS syntax in address/name inputs. Browsing a file manager is not a raw-device interface.
- Accept absolute drive and UNC paths. Relative address input resolves only against the current tab directory; never against the process working directory. Do not expand `%VAR%` or execute expressions in address input in MVP.

### 7.2 Identity and race policy

Represent a filesystem identity with volume identity and file ID when available, plus native path and metadata fallback. A hard-linked file can have multiple directory entries; never collapse those entries merely because file IDs match. Tokens identify entries in a session and snapshot, not globally unique file objects.

Before mutation, re-query the source entry and target parent. If identity changed, a plan expired, or the source disappeared, report a stale-item error and require replanning. Compare source and destination through native identity/ancestry where possible; a lowercase string prefix is not a descendant check. Support case-sensitive directories; preserve native spelling and do not treat search case folding as filesystem equality.

Handle-based validation narrows races but path-based Shell operations cannot guarantee immunity against a malicious concurrent local process swapping entries. Document this limitation. If revalidation detects a reparse/identity change, abort before queuing the affected item. Do not claim the application is a filesystem sandbox or atomically executes a whole plan.

### 7.3 Long paths and names

Embed `longPathAware` in the application manifest. Use extended-length paths only in adapters that support them, with correct `\\?\C:\...` and `\\?\UNC\server\share\...` forms. Keep a separate Shell-compatible representation; do not blindly pass every extended path to every Shell interface. A native enumeration may succeed where opening a Shell item fails; surface a precise unsupported-operation error. Never truncate a path.

Windows long-path support depends on both API behavior and machine/application configuration. The agent must inspect and test it; do not change the machine's global registry policy merely to conceal a failing case. [Microsoft path-length guidance](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation)

New names reject separators, `.`/`..`, reserved DOS device names including extension variants, forbidden characters, trailing dots/spaces, NULs, ADS separators, and names exceeding the destination volume's component limit. Empty names fail. Do not normalize away significant characters. Existing exceptional names remain displayable; unsupported Shell mutation must fail clearly.

### 7.4 Links, placeholders, and protected paths

- Index and automatic scans do not follow junctions, symlinks, mount points, or unknown reparse tags. Record the entry and stop recursion.
- Explicit navigation into a link is allowed after resolution; display the resolved location or a link indicator and support Back.
- In MVP, refuse recursive copy/move of a tree containing reparse points or unavailable cloud placeholders until the backend's semantics are verified. A canceled preflight means “not validated,” never “safe.” Explain the skipped/blocked entries.
- Recycling or renaming a link itself must be proven to affect the link rather than its target. Otherwise disable that action for the entry and report the limitation.
- No background content reads, hashing, thumbnail extraction, or forced cloud hydration.
- Reject mutation of drive/share roots, the running application directory, its state directory, and active fixture harness boundaries. Explain the specific protected location.
- Do not take ownership, weaken ACLs, enable backup privileges, request administrator mode, or disable antivirus.

## 8. IPC contract and authoritative state

Create typed request/response models with `schema_version: 1`. Centralize frontend IPC in `bridge/client.ts`; components must not invoke arbitrary command names. Generate TypeScript DTOs from Rust using a pinned maintained tool, or maintain paired schemas with a serialization compatibility test. Do not introduce a generation dependency that prevents a minimal build.

### 8.1 Required command surface

| Command | Input | Output / behavior |
|---|---|---|
| `bootstrap` | none | Session ID, capabilities, settings, roots, restored tab state |
| `navigate` | tab ID, address or item token | Folder token and snapshot request ID |
| `list_page` | folder token, snapshot ID, cursor, sort/filter, page size | Entries, next cursor, completion/partial status |
| `refresh` | tab ID | New snapshot; cancel superseded listing |
| `open_item` | item token | Launch result; file association errors typed |
| `show_properties` | selected item tokens | Native dialog request outcome |
| `plan_operation` | kind, source tokens, destination folder token/name | Expiring plan, warnings, count/size estimate or unknown |
| `commit_operation` | plan ID, user confirmation, client request ID | Job ID; idempotent within the session |
| `cancel_job` | job ID | Cancellation-request state; eventual outcome through events |
| `get_job` / `list_jobs` | ID / pagination | Authoritative current state and item outcomes |
| `clipboard_write` | tokens, copy/cut intent | Windows clipboard result |
| `clipboard_read` | none | Validated import summary, transient source tokens, intent |
| `search` | query, scope, cursor, request ID | Paged results plus freshness status |
| `index_add_root` / `index_remove_root` | validated folder token/root ID | Explicit indexing consent / removal status |
| `index_status` / `index_rebuild` | root ID | Progress or scheduled rebuild |
| `settings_update` | validated patch | Persisted settings version |
| `favorites_update` | validated change | Updated favorites |

Use strings for IDs and 64-bit byte counts in JSON; TypeScript must not round large `u64` values. Timestamps use UTC ISO-8601 or documented integer-string units, then format locally in the UI. Unknown file size is `null`, not zero. Folder size is unknown unless explicitly calculated later.

### 8.2 Snapshot/event semantics

- Directory pages contain up to 256 entries and a snapshot generation. Snapshot row IDs remain stable across pages.
- Listing can stream initial unsorted chunks; clearly mark “Loading.” Stable sorted pagination becomes available once enumeration/sort finishes. Do not pretend first-page order is final while inserting rows invisibly.
- Events include `schema_version`, session, request/job/tab ID as appropriate, generation, monotonically increasing sequence, and event kind.
- Frontend drops stale generations and never applies a previous tab's reply to the active tab.
- Progress events are throttled to at most 10 per second per job; listing/watch updates are batched.
- Terminal states and errors are recoverable through authoritative commands. If event delivery drops, sequence gaps trigger `get_job`/snapshot refresh rather than a permanent stale UI.
- A bounded progress queue can replace an older progress update. A terminal event must also be persisted and queryable before emission.
- Close tab unsubscribes its watches and cancels only its listing/search requests, not shared file jobs.
- Tokens bind to session/window and expire; plans expire after 5 minutes, require single-use commit, and revalidate immediately before execution.
- A repeated `client_request_id` returns the same job. Persist enough deduplication state to avoid duplicate submission during reconnect; after restart, show interrupted state and require an explicit new plan.

## 9. Directory enumeration and navigation

Use Windows directory enumeration behind `explorer-win` with wide-character APIs, owned search handles, and metadata already available from enumeration. Avoid a separate `stat` on every file where the enumeration result suffices.

Each `FileEntry` includes: entry token, parent folder token, display name, optional escaped-name hint, extension/type label, file/directory/reparse kind, size, modified timestamp, attributes, optional native file identity, and capability flags. ACL checks and Shell metadata are lazy; enumeration does not promise a later operation will be permitted.

Implementation sequence:

1. Validate/resolve navigation input on a filesystem worker.
2. Register watch coverage and assign a generation; tolerate unsupported watch setup.
3. Enumerate into bounded chunks, coalescing any concurrent dirty signal.
4. Deliver a first visible chunk quickly; avoid content reads and icons per entry.
5. Build a compact snapshot, sort on a worker, publish its final order, and expose pages.
6. If the folder changed during enumeration, reconcile or schedule a new snapshot.
7. Keep a small byte-budgeted LRU of recent snapshots; release unused tokens/handles.

Sort by folders first, then chosen column, then deterministic native-name/tie-breaker ordering. Specify unknown values last. Use native ordinal comparison for navigation identities and a tested display ordering for names; natural numeric ordering is SHOULD, with `file2` before `file10`. Selection follows stable row identity, not a current row index.

Read failures must return partial entries and a warning when useful, or a full error when the folder itself cannot be opened. A removed drive returns “Location unavailable” with Retry and Back. F5 forces a fresh enumeration even when watcher state says clean.

## 10. UI strategy and interaction requirements

### 10.1 Main window

Use standard Windows window decorations, minimize/maximize/close behavior, and DPI handling from the desktop host. Default size approximately 1200×800; minimum 800×600. Use a Fluent-inspired interface with restrained spacing, clear typography, and no expensive visual effects required for usability.

```text
┌ Tabs: Documents | Downloads | + ────────────────────────┐
│ Back Forward Up   [breadcrumbs / editable address]       │
│ New folder  Copy  Cut  Paste  Rename  Recycle   [Search]  │
├───────────────┬──────────────────────────────────────────┤
│ Favorites     │ Name          Type    Size    Modified   │
│ Known folders │ Virtualized selectable file rows         │
│ Drives        │ Loading/empty/error state when applicable│
├───────────────┴──────────────────────────────────────────┤
│ Selection summary              Index state  Jobs button │
└──────────────────────────────────────────────────────────┘
```

Jobs open in a drawer with source/destination summaries, state, item results, progress estimate, cancel action, and Retry failed items. Native Windows dialogs handle copy/name conflicts in MVP; an application dialog must not independently decide the same conflict.

Tabs preserve location, history, sort, selection, and scroll independently. On reopen, unavailable tabs remain visible with a recoverable error. Do not recreate jobs from restored tabs.

### 10.2 Required keyboard behavior

| Key | Action |
|---|---|
| Enter / double click | Navigate directory or open selected file |
| Alt+Left / Alt+Right / Alt+Up | Back / Forward / Parent |
| Ctrl+L | Focus address bar |
| Ctrl+T / Ctrl+W | New tab / close tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Cycle tabs |
| Ctrl+A | Select all in current results; excluded while editing text |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / cut / paste |
| F2 | Rename one selected entry |
| Ctrl+Shift+N | New folder |
| Delete | Plan recycling, show confirmation, submit on user acceptance |
| F5 | Refresh |
| Ctrl+F | Focus search/filter |
| Escape | Cancel inline editing/selection mode; dismiss noncritical popup |
| Arrow/Shift+Arrow/Home/End | Navigate/extend selection with visible focus |

Shift+Delete is disabled for MVP with a clear message. Do not silently map it to permanent deletion. Keyboard handlers respect input fields, composition events, and native dialogs.

### 10.3 Accessibility and large-list behavior

- Expose roles, names, selection state, row count/index, and a roving focus model compatible with virtualization.
- Ensure the focused row remains mounted; announce selection/job state changes without announcing every progress tick.
- Minimum 4.5:1 contrast for ordinary text, visible focus, 200% zoom support, and respect reduced motion.
- Test 100%, 150%, and 200% DPI; verify long names, RTL characters, emoji, and narrow windows.
- Never render filenames as HTML. Text highlighting uses React text nodes/spans, not HTML generated by FTS.
- Start with generic file-type icons. Native icon cache is SHOULD only after enumeration performance passes; thumbnails are later.
- Drag/drop within the UI is SHOULD after clipboard operations work. External native OLE drag/drop is later and must not block MVP.

## 11. Windows native integration

### 11.1 Required adapters

| Feature | Native approach | Requirements |
|---|---|---|
| Known folders | `SHGetKnownFolderPath` | Documents/Desktop/Downloads/etc.; do not assume `C:\Users` layouts |
| Drives | Logical-drive APIs and volume metadata | Avoid blocking UI on unavailable/network drives |
| Open association | `ShellExecuteExW`, normal open verb | Explicit user action only; no shell command concatenation |
| Properties | Supported Shell properties verb/API | Native selection or conservative single-item support |
| File operations | `IFileOperation` + `IShellItem` + progress sink | Dedicated STA and owner HWND |
| Clipboard | `CF_HDROP`, preferred drop effect, Shell/OLE data object | Correct ownership and Unicode handling; Explorer interoperability |
| Attributes/identity | `CreateFileW` and supported information APIs | RAII handles; fail safely when identity cannot be retrieved |
| Watcher | `notify` Windows backend | Own overflow/reconciliation policy |
| Icons, later | Shell image/icon APIs | Small byte-budgeted cache; release icon/GDI resources |

Show an app-owned context menu with Open, Open parent, Copy, Cut, Paste, Rename, Recycle, and Properties. Full native third-party context menus require additional Shell extension/process isolation work and are later. “Open in Windows Explorer” may use a supported Shell method or correctly quoted argument-vector launch; never interpolate filenames into `cmd.exe` or PowerShell.

### 11.2 Clipboard correctness

Use Shell/OLE clipboard semantics so copying in this app can paste into Explorer and copying/cutting in Explorer can paste here. `CF_HDROP` provides filesystem paths; cut intent uses the registered preferred-drop-effect format. Validate imported paths as untrusted input, check buffer sizes and double-NUL termination, and do not free memory whose ownership was transferred to Windows. Bound imports to 10,000 entries and a documented byte limit; reject oversized data clearly.

For in-app cuts, mark only successfully moved items as completed. Never delete a cut source simply because paste was requested. On partial move, failed entries remain selectable/retryable. Track clipboard ownership/sequence before clearing or updating it; never overwrite a newer clipboard from another application. Implement the Shell completion formats needed by the chosen OLE data object, and test partial results with Explorer. If external cut completion semantics are unreliable, retain clipboard data and explain the limitation rather than deleting or recopying files.

Clipboard reads/writes require explicit user actions; never poll clipboard contents in the background. [Microsoft Shell clipboard formats](https://learn.microsoft.com/en-us/windows/win32/shell/clipboard)

## 12. File operations and the job model

### 12.1 Planning

Every mutation follows **plan → review/confirmation where needed → commit → revalidate → execute → reconcile**.

Planning must:

1. Resolve sources/destination from session-bound tokens.
2. Deduplicate identical entries and remove a child selected together with its selected ancestor.
3. Reject invalid names, protected roots, source=destination, moving a folder inside itself, and unsupported link/placeholder semantics.
4. Preflight tree policy without opening file contents; allow cancellation. For huge trees show progress and unknown totals until available.
5. Determine known file counts/bytes and likely cross-volume behavior; treat free-space checks as advisory because files and space can change.
6. Store native source/destination identities, exact user intent, warnings, and expiry in an immutable plan.
7. For Recycle, show selected count and source location; do not offer a permanent fallback.

Limit one plan to 10,000 explicitly selected entries. A selected directory can contain more descendants; traverse it with a bounded queue and cancelable preflight. Oversized selection receives a clear split-the-operation message rather than a truncated job. Record which operation kinds allow an empty source list (for example create-folder).

Copy/move/rename name collisions are decided by native Windows dialogs in MVP. The plan can warn that a conflict exists but cannot promise it will still exist at execution. No default replace-all policy, automatic “keep newer,” or silent merge.

### 12.2 States and outcomes

```text
Planned → Queued → Validating → Running → Succeeded
                                │        PartialFailure
                                │        Failed
                                └─ CancelRequested → Canceled or another terminal outcome
Restart with an unfinished journal → Interrupted
```

Cancellation is a request; it may arrive after success. The authoritative terminal outcome wins. `Canceled` may contain successfully completed items. `PartialFailure` means some items succeeded and others failed/skipped; never imply rollback. A terminal state includes per-item outcomes, native error codes, and whether output paths need inspection.

Queued cancellation removes that job before any native work. Running cancellation is checked in supported progress/pre-item callbacks and the native dialog's Cancel action. A stalled network/API call can delay cancellation; show “Cancellation requested,” keep UI responsive, and never kill a thread to manufacture a canceled state.

### 12.3 Journal and retry

Persist the job before execution and persist observable item outcomes as callbacks arrive. Crash windows exist between filesystem changes and journal writes. On restart, unfinished jobs become `Interrupted`; inspect known paths only on user request and never automatically resume, delete a source, or delete an output based on the journal alone.

Retry creates a fresh plan from confirmed failed/skipped items. Revalidate destinations, conflicts, and sources. A retry must not recopy already successful items automatically. App-managed undo is later; do not label Windows' session undo support as a guaranteed in-app undo feature.

## 13. Copy engine: MVP native backend and later custom backend

### 13.1 Backend interface

Define a Rust operation backend trait accepting a validated native plan, cancellation signal, and bounded progress/outcome sink. It returns a structured execution report. Keep the trait independent of Tauri and COM types. Implement **one** production backend for MVP: `ShellOperationBackend`.

For MVP, “copy engine” means the Rust planner, queue, telemetry, and result handling around Windows' native engine, not a reimplementation of byte transfer. Native copy provides useful Windows semantics while Rust owns the app's workflow.

### 13.2 Shell operation implementation

On the file-operation STA:

1. Create `IFileOperation` and attach owner HWND from the validated originating window.
2. Set explicit flags per operation; do not rely on changing defaults.
3. Create native Shell items from lossless paths, preserving errors for unsupported paths.
4. Attach one global `IFileOperationProgressSink` using `Advise`; do not also attach the same sink to each item and duplicate notifications.
5. Queue the intended `CopyItem`, `MoveItem`, `RenameItem`, `NewItem`, or `DeleteItem` calls.
6. Execute `PerformOperations` on the STA, allowing native conflict/error/progress UI.
7. Capture pre/post item callbacks, native result codes, returned destination items, and progress estimates. Native conflict decisions may rename outputs; use the returned item rather than guessing its path.
8. Check `GetAnyOperationsAborted` in addition to the overall HRESULT and callbacks.
9. Unadvise through RAII, aggregate outcomes, persist terminal state, and reconcile affected folders/index roots.

Use conservative flags. Do not set `FOF_NOCONFIRMATION`, `FOFX_KEEPNEWERFILE`, `FOFX_REQUIREELEVATION`, or flags that traverse junctions. Do not suppress all error UI in the interactive MVP. Native elevation prompts, if encountered, remain explicit OS choices; never auto-approve them or relaunch the whole app as administrator.

For recycling, request `FOFX_RECYCLEONDELETE` and `FOF_WANTNUKEWARNING`; preserve supported Windows undo bookkeeping with `FOFX_ADDUNDORECORD` where appropriate. These are native flags, not a substitute for verifying the outcome. [Microsoft operation flags](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags)

In `PreDeleteItem`, examine the documented transfer flags, including `TSF_DELETE_RECYCLE_IF_POSSIBLE`, and cancel the operation if the Shell is not offering recycling for a requested recycle-only job. The flag itself means “if possible,” not an unconditional guarantee. Reject known unsupported locations such as UNC recycling before submission. Prove through tests that non-recyclable items remain intact, including changed conditions after preflight. Do not assume any combination of flags guarantees recycle-only behavior on every filesystem. If this guarantee cannot be established on a target, disable recycling there and expose the error. [PreDeleteItem contract](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperationprogresssink-predeleteitem), [Transfer flags](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_transfer_source_flags)

Progress from `UpdateProgress` is an estimate of work, not necessarily bytes. DTOs declare `progress_kind: items | work_units | bytes | indeterminate`; never convert work units into MB/s. The UI may show native progress and a simplified job card. Pause/resume is not an MVP feature. Post-item callbacks report individual HRESULTs; account for canceled/skipped items even if the top-level call succeeded. [Progress sink documentation](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperationprogresssink)

Do not pre-delete a destination, delete a move source manually, or remove an unknown partial output after cancellation. Native outcomes may leave partial filesystem changes. Report and reconcile them; cleanup requires an explicit fresh operation.

### 13.3 Later custom engine experiment

Only after MVP acceptance, prototype a separate backend using `CopyFile2` for file transfers and project-owned traversal. Begin with ordinary local files. Compare behavior and speed against the Shell backend before enabling it. `CopyFile2` exposes transfer callbacks and preserves several Windows file attributes/streams; its security-attribute behavior requires separate policy. [Microsoft CopyFile2 documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-copyfile2)

The later backend must define:

- Per-file temporary output in the destination directory with a job-owned unique name and no-clobber creation.
- Explicit policy for replacement commit, failures, and recoverable temporary outputs; atomicity claims limited to supported per-file same-volume operations.
- Source identity/size/mtime checks before and after transfer; verification catches detected changes, not every possible concurrent rewrite.
- ACL inheritance, timestamps, alternate streams, sparse/compressed/encrypted files, link handling, cloud placeholders, and metadata errors.
- Cross-volume move: copy, verify, commit, then remove source only after verified success under a documented race policy.
- Optional checksum verification, separately measured and opt-in for ordinary copies.
- Bounded concurrency, initially one transfer per storage pair; no arbitrary 32-thread disk saturation.
- Tested cancellation and partial cleanup; pause/resume only after API semantics and retained state are proven.
- Failures such as disk full, destination disconnect, locked files, source mutation, and crash between commit and source removal.

Keep this experimental and opt-in until its correctness matrix passes. It must not delay or replace the MVP native backend.

## 14. Indexing and search

### 14.1 Two distinct search modes

1. **Current folder filter:** immediate case-insensitive literal substring filtering of the current snapshot; accepts 1–256 Unicode characters; no recursive crawl or database dependency.
2. **Indexed search:** filename-only search across one selected root or all explicitly indexed roots; needs at least one text term containing 3 Unicode characters, or a supported metadata-only filter. UI visibly identifies scope and freshness.

Indexed roots start empty. Offer “Index this folder,” show its path and exclusions, and request explicit user selection. Do not automatically scan all drives, the full user profile, credential stores, or network shares. MVP permits at most 8 non-overlapping local roots; reject duplicates/overlap using resolved native locations, not only string prefixes. Removing a root stops its watcher/crawl and transactionally removes its index records and FTS rows.

Search stays available for completed portions of an in-progress index, with “Indexing” and last-reconciled time shown. An offline root's results are clearly marked unavailable/stale. Before opening or mutating a result, validate the live entry and issue a normal session token. Indexed metadata is a hint, never authority for a file operation.

### 14.2 MVP query language

| Query | Behavior |
|---|---|
| `invoice` | Filename contains `invoice`, ignoring search case |
| `invoice september` | Both literal terms appear somewhere in the filename |
| `"annual report"` | Literal contiguous phrase appears in the filename |
| `invoice ext:pdf` | Literal filename match plus extension filter |
| `type:folder` / `type:file` | Kind filter in indexed scope |
| `ext:png` | Metadata-only extension query, subject to result cap |

No raw FTS syntax, SQL, regular expressions, glob expansion, shell expansion, OR, or arbitrary field grammar in MVP. Unknown filters and unmatched quotes return a helpful parse error. Search terms are normalized by a documented Unicode lowercase function; accents remain significant. Do not normalize native paths or collapse distinct entries for search.

At least one term of 3+ characters supplies an FTS trigram candidate set. Shorter terms refine it using bound `instr` predicates. Recursive searches with only 1–2-character text terms return “Use at least 3 characters for indexed search”; current-folder filtering still works. Limit query length to 256 Unicode characters and 16 terms. Escape quotes inside FTS literals and bind every SQL parameter. Tests must cover literal `%`, `_`, quotes, punctuation, and FTS operator words.

### 14.3 Storage model

Use a normal SQLite table as the authoritative metadata index and a regular, content-bearing FTS5 table for normalized filenames. Prefer simplicity over external-content optimization initially. Illustrative schema, to be finalized with migrations:

```sql
CREATE TABLE roots (
  id TEXT PRIMARY KEY,
  path_utf16le BLOB NOT NULL,
  display_path TEXT NOT NULL,
  volume_key BLOB,
  state TEXT NOT NULL,
  completed_epoch INTEGER NOT NULL DEFAULT 0,
  reconciled_at TEXT
);

CREATE TABLE entries (
  id INTEGER PRIMARY KEY,
  root_id TEXT NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  parent_id INTEGER REFERENCES entries(id),
  path_utf16le BLOB NOT NULL,
  name_display TEXT NOT NULL,
  name_norm TEXT NOT NULL,
  extension_norm TEXT NOT NULL,
  kind INTEGER NOT NULL,
  size_bytes INTEGER,
  modified_filetime INTEGER,
  attributes INTEGER NOT NULL,
  file_identity BLOB,
  seen_epoch INTEGER NOT NULL,
  stale INTEGER NOT NULL DEFAULT 0,
  UNIQUE(root_id, path_utf16le)
);
CREATE INDEX entries_parent ON entries(root_id, parent_id);
CREATE INDEX entries_extension ON entries(root_id, extension_norm, kind);

CREATE VIRTUAL TABLE filename_fts USING fts5(
  name_norm,
  tokenize='trigram'
);
```

`filename_fts.rowid` equals `entries.id`. Add insert/update/delete triggers or equivalent writer transaction logic that keeps both tables in sync, including root deletion. Test rollback and rebuild explicitly. Path bytes are never an FTS column. The unique path key refers to the exact enumerated native entry spelling; normalize alternate user navigation spellings before root registration while preserving case-sensitive entries.

Finalize parent foreign-key deletion rules explicitly: remove descendants in a verified order or use a tested cascade policy. Do not ship a schema that prevents removing an indexed subtree or leaves orphan FTS rows. Use a dedicated root directory entry with a null parent so immediate children have a stable parent identity.

Use WAL, foreign keys, a bounded busy timeout, small batched writes, and scheduled checkpointing. Start with at most 500 entries per transaction or 100 ms of accumulated work, whichever comes first. Choose synchronous durability appropriate to each database: index data is reconstructable; job journaling requires stronger durability. Prefer separate `index.sqlite3` and `state.sqlite3` so a rebuild cannot erase settings or job history. Check that bundled SQLite supports FTS5/trigram during M0 with a real create/insert/query test.

SQLite's trigram tokenizer supports substring search, with limitations for short tokens. Use quoted, bound MATCH expressions and validate behavior against the actual bundled SQLite version. [SQLite FTS5 reference](https://www.sqlite.org/fts5.html)

### 14.4 Crawl and reconciliation

- Enumerate metadata only, using a bounded work queue and no file-content reads.
- Register root watch coverage before baseline crawl; collect/coalesce dirty directories while scanning. If coverage fails, mark the root degraded and reconcile on a bounded polling schedule.
- Exclude the app state directory, `$Recycle.Bin`, `System Volume Information`, and configurable user exclusions. List skipped locations in root status.
- Do not follow reparse points. Do not hydrate placeholders. Persist their entry metadata when available.
- Pause/cancel indexing promptly between enumeration batches; browsing/copying has priority over background crawl.
- Mark an entry seen in a scan epoch. Remove missing children only after their specific parent directory was successfully enumerated to completion. A permission-denied directory, canceled crawl, missing volume, or partial enumeration must never trigger a destructive index-wide sweep.
- If a parent disappeared under a successful parent enumeration, remove its indexed subtree transactionally. If a directory renamed, reconcile old/new parents and re-enumerate the renamed subtree, or rewrite its native paths in a verified transaction; never leave children at the old path indefinitely.
- Never treat file IDs alone as unique entry keys; retain hard-linked entries.
- Persist checkpoints by completed directory/root generation, not an in-memory iterator. Restart incomplete scans and reconcile dirty roots; no claim of an exact resume position.
- Bound pending dirty directories. On overflow, mark the root `needs_reconcile`, drop redundant detailed events, and schedule a complete safe scan.
- Expose root states: `not_indexed`, `scanning`, `ready`, `degraded`, `offline`, `needs_reconcile`, `error`.

### 14.5 Result ordering and cancellation

Debounce input by approximately 150 ms; cancel superseded requests. Query the FTS candidate set, apply bound metadata/short-term predicates, and rank exact normalized filename first, full-text-prefix next, then other substring matches. Tie-break by normalized name, native entry identity/path bytes, and entry ID so repeated queries are deterministic.

Return 100 results per page from a request-scoped snapshot of at most 1,000 matches. If capped, show “First 1,000 matches; refine search.” Do not call a truncated set the total match count. Search snapshots have a bounded TTL/cache budget and cursors bind to query/scope/generation. Offset or keyset pagination may be used internally if it preserves a request's snapshot; do not allow results to jump between pages during concurrent updates.

Use SQLite interrupt/progress facilities on the query's own reader connection to cancel long work. Do not interrupt another tab's request on a shared connection. When an index fails, current-folder navigation/filtering remains usable. Offer Rebuild; keep the old failed database quarantined until recovery diagnostics are captured.

## 15. Filesystem watching and freshness

Use `notify` as the MVP backend behind `WatchBackend`. The project owns state recovery, coalescing, and rescan policy; the crate's existence does not make events a complete filesystem history. [notify project](https://github.com/notify-rs/notify)

### 15.1 Subscriptions

- Watch visible folder snapshots non-recursively and indexed roots recursively where supported.
- Deduplicate overlapping coverage; reference-count subscriptions so closing one tab does not remove another tab's watch.
- Bind events to native locations and generation IDs. Close handles/subscriptions when no longer needed.
- Start with a 4,096-event ingress capacity, 100–200 ms coalescing window, and at most 1,024 unique dirty-directory records; tune with evidence.
- Duplicate/out-of-order/unpaired rename events trigger reconciliation rather than guessed final state.
- File writes can still be in progress; refresh metadata with bounded backoff and do not open content to check completion.
- Do not suppress events generated by the app's own operations. They are deduplicated and reconciled with explicit completion refreshes.

### 15.2 Overflow, unsupported locations, and lifecycle

Any watcher overflow, backend rescan indication, dropped queue item, root disconnection, or unknown rename marks the relevant coverage dirty. Publish a degraded/freshness indicator and run enumeration-based reconciliation. The UI's Refresh action always works independently of notifications.

Windows change notifications can lose detail on buffer overflow; the native API documents re-enumeration as recovery. Network watches have additional buffer constraints. Do not make correctness depend on receiving every event. [ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw)

If notification coverage is unavailable, poll only actively viewed folders at a default 2-second interval while focused, with exponential backoff on failures. Indexed-root fallback uses incremental directory reconciliation with a bounded background budget; default full reconciliation every 15 minutes when idle. Do not rescan a whole large root every 2 seconds. Declare index freshness approximate in this mode.

After sleep/resume, regained window focus, or drive reattachment, refresh visible folders and mark affected index roots for reconciliation. Changed root volume identity requires explicit re-registration/rebuild; do not assume a drive letter always refers to the same volume. Never delete index records simply because a root is temporarily offline.

## 16. Persistence, error handling, and diagnostics

### 16.1 State locations and migrations

Resolve app-local storage with the platform known-folder/Tauri path API. Use a per-user directory such as `%LOCALAPPDATA%\RustExplorer`, never the installation directory, current folder, or repository. Store settings, job history, logs, and index separately. Treat caches/indexes as reconstructable and settings as user data.

Schema migrations use monotonically increasing versions, transactions, and tested previous-version fixtures. Before an incompatible state migration, create a bounded backup. A corrupt index is rebuildable; corrupt preferences fall back to defaults with an explanatory message and preserved backup. Do not reset preferences on every database error.

Settings defaults: system theme, hidden files off, details view, no indexed roots, restore tabs enabled, no telemetry. Validate restored paths asynchronously. Save changes after a short debounce and on orderly shutdown. A stale favorite remains editable/removable instead of causing startup failure.

### 16.2 Error model

Return structured errors with:

```text
code, user_message, operation, retryable, item_token(optional),
native_code(optional), correlation_id, safe_details(optional)
```

Required codes include `NotFound`, `AccessDenied`, `SharingViolation`, `AlreadyExists`, `InvalidName`, `PathTooLong`, `UnsupportedPath`, `UnsupportedReparsePoint`, `PlaceholderUnavailable`, `DiskFull`, `LocationOffline`, `RecycleUnsupported`, `StaleItem`, `Canceled`, `IndexUnavailable`, `WatcherDegraded`, `QueueFull`, and `Internal`.

- Preserve Win32/HRESULT codes; cancellation must not be flattened into a generic failure.
- Map known errors to plain language with an actionable next step. Keep unknown errors visible with a correlation ID.
- Use `Result` in runtime paths. No `unwrap`/`expect` on user input, network status, database operations, clipboard data, or filesystem outcomes.
- Do not retry destructive operations automatically. Metadata reads may retry transient sharing violations up to 3 times with bounded jitter/backoff; jobs require an explicit fresh retry plan.
- Distinguish unsupported behavior from missing privileges and from an internal defect.
- Never use “Success” based only on submitting a job or receiving a top-level COM success value.

### 16.3 Observability

Use structured spans for listing, query, crawl, job, native call category, and reconciliation. Record elapsed time, counts, queue depth, and safe IDs. Default logs omit full paths, filenames, clipboard content, and search text. Debug path logging is an explicit local opt-in that expires or is easy to disable.

Rotate logs with a default total budget of 20 MB and 7-day retention. Allow users to preview a diagnostic export before saving it. Panic reporting is local; no automatic upload or remote crash SDK. Display a recoverable failure window/message when possible and preserve the journal. Do not catch an FFI panic and continue with potentially corrupted native state; prevent Rust unwinding across foreign callback boundaries.

## 17. Security and data-safety requirements

### 17.1 Threat model

Treat filenames, clipboard buffers, directory events, cached index records, and frontend IPC parameters as untrusted. The application has the standard user's filesystem rights and can mutate their files by design. Protect against accidental destructive actions, malformed inputs, webview injection, confused IPC requests, stale identities, and unsafe native lifetimes.

A compromised authorized main webview can issue permitted commands, and path-based operations remain subject to local filesystem races. Opaque tokens and preflight checks reduce misuse but are not a sandbox or a defense against every malicious local process. Record these boundaries accurately.

### 17.2 Required controls

- Production WebView loads only bundled application assets. Deny remote navigation, popups, embedded remote frames, and untrusted page execution.
- Explicit CSP: self-only scripts/assets; no `unsafe-eval`; images limited to approved bundled/data/custom sources; production `connect-src` limited to required local IPC endpoints. Development HMR exceptions stay in development config only. Verify the exact CSP needed by pinned Tauri/WebView2, rather than shipping a restrictive-looking policy that breaks IPC.
- Explicit main-window capabilities; no wildcard windows or remote URL grants. Enable only required core features and app commands. Do not expose broad filesystem, shell-exec, or process plugins to the frontend.
- Register custom command names in Tauri's application manifest/permission configuration and test denied origins/windows. Custom registered app commands can otherwise be broadly available by default; built-in plugin scopes do not validate custom Rust path arguments automatically. Implement authority and argument checks in each command. [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- Mutation plans bind to the authorized session/window, immutable intent, expiry, and a single-use confirmation. Confirmation required for Recycle and for any later overwrite/permanent-delete mode.
- Limit IPC payload sizes, page sizes, job submissions, source selection count, query length, clipboard buffers, and path component lengths.
- Never parse or execute file content for enumeration/search. Open executables/documents only after explicit user action, using the Windows association and OS security behavior.
- Do not concatenate file paths into shell commands. Use native APIs or argument-vector process launch only.
- Preserve Windows security behavior and Mark-of-the-Web streams through the native backend; test supported same-volume/cross-volume cases and document deviations.
- Do not change global file associations, shell registry settings, long-path registry policy, autorun entries, UAC, Defender, or ACLs.
- Index and state remain local. No outbound app network requests except an installer obtaining WebView2 after clear installer disclosure. WebView2's own platform behavior is not an app privacy guarantee.
- Test builds that support fixture-only operation restrictions must be separately labeled and excluded from release artifacts. Production must not include debug commands that bypass confirmation or authority checks.

Run dependency vulnerability/license checks; investigate relevant findings. A justified advisory exception needs owner, reason, expiry, and evidence. Do not silence every advisory or exclude vulnerable dependencies just to make CI green.

## 18. Performance goals and measurement protocol

These are engineering targets, not promises about all Windows machines. Required baseline: Windows 11 x64, 4+ physical/logical modern CPU cores, 16 GB RAM, local SSD/NTFS, standard user, release build, default Defender active. Record actual hardware, power mode, OS build, WebView2 version, compiler/dependency versions, filesystem, and fixture seed.

| Scenario | MVP target | Measurement |
|---|---|---|
| Process launch to usable window | Cold ≤2.5 s; warm p95 ≤1.5 s | 10 cold-ish launches labeled accurately; 30 warm launches |
| 1,000-entry folder | First visible rows p95 ≤300 ms; final sorted snapshot ≤1 s | 30 navigations, mixed file/dir names |
| 100,000-entry folder | First rows p95 ≤500 ms; final sorted snapshot ≤5 s | 10 runs; clearly loading while sorting |
| Indexed search, 100k entries | Rust query p95 ≤100 ms; input-to-results p95 ≤300 ms including debounce | ≥100 representative queries; warm index |
| Broad/short filtered queries | Finish or show refinement/cancellation within 1 s | Include cap-hit and metadata-only queries |
| Initial 100k metadata crawl | ≤60 s baseline target | Report entries/s and exclusions; no content reads |
| External change to visible row | p95 ≤500 ms under normal watch coverage | ≥100 create/rename/delete events |
| Small-file copy | ≥80% of matched native Shell control throughput | Same source/destination and settings; ≥5 alternating runs |
| Large-file copy | Within 10% of matched native Shell control time | ≥5 runs of 1–4 GiB; distinguish warm/cold cache |
| Scrolling/selection | No UI task >100 ms in common interaction; aim for frame budget | Runtime trace plus real WebView2 check |
| Idle CPU | Median <1% for app process tree over 60 s when quiescent | Watch/index idle; no test fixture generation |
| Idle memory | Process-tree private bytes ≤350 MiB | Include WebView2 child processes; report Rust host separately |
| Memory during 100k crawl/list | Process-tree private bytes ≤500 MiB | Report peak and retained memory after cleanup |
| Index disk usage | ≤350 MiB for 100k metadata entries | Report DB/WAL/SHM separately, after checkpoint |
| Cancellation | Queued immediate; active request reflected ≤100 ms | Native completion may be delayed by blocked APIs |

Hard correctness gates do not become negotiable when performance fails. If a target misses, profile, implement a measured improvement, rerun the affected benchmark, and document the residual issue. Any unachievable target needs an explicit owner-approved waiver before declaring the full MVP done; do not silently lower it.

### 18.1 Implementation budgets

- Generic icons first; lazy Shell metadata, no thumbnails or content scanning in listing.
- Bounded workers, byte-budgeted snapshot/search caches, bounded pending jobs/watch events.
- Virtualized rows with memoized narrow state subscriptions; selection updates must not re-render all entries.
- Rust sorting/search preparation off the UI thread; chunk/page IPC instead of one huge JSON response.
- SQLite prepared statements, transactional batches, FTS candidate filtering; inspect query plans before adding another index/search engine.
- Foreground navigation has priority over crawl. Reduce/pause background indexing during sustained mutation/load.
- Measure resource ownership: repeated tab navigation must not continuously grow handles, GDI objects, subscriptions, or memory.

## 19. Tests and acceptance fixtures

Tests must check externally meaningful behavior and failure outcomes. Do not write tests that merely repeat a function's implementation. All mutation tests operate only in generated fixture roots.

### 19.1 Fixture safety and generation

`cargo xtask fixtures` creates a uniquely named directory under the repo's ignored `work/fixtures/` or a controlled temporary directory. Write a marker containing a random run ID and manifest of created files. Cleanup resolves absolute targets, verifies the marker/run ID and containment, and removes only that fixture. Never recursively delete a user-supplied path, drive root, parent workspace, or arbitrary environment directory.

Fixture profiles:

- `small`: 100 entries, mixed directories and small files, deterministic contents/hashes.
- `medium`: 1,000 entries, spaces, emoji, multilingual names, hidden/read-only files, empty files/folders.
- `large-list`: 100,000 entries in one folder, deterministic name distribution; created only on request.
- `index-100k`: 100,000 entries across a tree, duplicate names/extensions, known query answers.
- `copy-mixed`: many small files plus 1–4 GiB ordinary files; not sparse stand-ins for throughput benchmarks.
- `edge-cases`: long paths, hard links, reparse links where privileges permit, case-sensitive-directory fixture where supported, locked handles, and cloud placeholder test plan.

Index benchmarks can use synthetic metadata DBs separately; physical crawl benchmarks must use real directories. Large fixture creation time is excluded from operation benchmarks and reported separately. Hash-based verification may read contents in tests; the production indexer must not.

### 19.2 Rust unit/property tests

- Native path encoding round-trips including unpaired UTF-16 units.
- New-name validation, reserved names, ADS/device path rejection, component limits.
- No string-prefix descendant false positives (`C:\a` versus `C:\ab`).
- Token expiry/window isolation, plan immutability, duplicate commit idempotency.
- State transitions, canceled-with-successful-items and partial failure aggregation.
- Search parser/escaping, Unicode normalization semantics, metadata filters, deterministic ranking.
- Pagination generation handling, stale cursor rejection, stable selection IDs.
- Watch duplicate/rename/overflow coalescing and dirty-root escalation.
- FTS insert/update/delete transaction consistency, migrations, corruption recovery.
- Journal recovery to `Interrupted`; no automatic resumption.
- Property tests/fuzz inputs for path/name/query/clipboard parsing, bounded allocations, and no panics.

### 19.3 Windows integration tests

| Scenario | Required assertion |
|---|---|
| Enumerate/create/rename | Correct native names/metadata; independent enumeration confirms result |
| Case-only rename | Correct native spelling change on ordinary NTFS; distinct-case entries handled conservatively in case-sensitive directories |
| Copy nested ordinary tree | Destination contents match; source unchanged |
| Same-volume move | Correct destination; source removed only for successful entries |
| Cross-volume move | Verify on two volumes in release matrix; record unavailable environments |
| Collision | Native conflict choices honored; declined replacement preserves destination |
| Locked source/destination | Typed failure or explicit native choice; no false success |
| Disk full | Fault-injection unit/integration path and constrained-volume release test; source preserved |
| Cancellation | Terminal state reflects completed/remaining items; no arbitrary output cleanup |
| Access denied | Standard-user behavior; no ACL modification/elevation workaround |
| Recycle local file | Source absent and item recoverable through Windows Recycle Bin |
| Recycle unsupported item | Source intact; no permanent fallback |
| Reparse point | No recursive target traversal; unsupported mutation explicitly rejected |
| External create/rename/delete | Visible listing and indexed scope converge to independent enumeration |
| Directory subtree rename | Search children resolve at new paths; stale old results disappear |
| Watch overflow | Inject missed-events signal; reconciliation restores exact known fixture state |
| Offline/reattached drive | Recoverable UI state; no index-wide accidental deletion |
| Long/non-UTF-8 native names | Lossless identity/selection; operations either correct or explicit unsupported error |
| Crash during job | Journal becomes Interrupted; no automatic source deletion/retry |
| Cloud placeholder | Enumeration/index do not hydrate; blocked/unsupported transfer disclosed |
| Clipboard with Explorer | Copy and cut/paste both directions, multi-item and partial failure |
| ADS/Mark-of-the-Web | Supported native copies preserve expected streams or document observed restriction |
| Lifecycle | Repeated tab open/close does not leak watchers/handles without bound |

Use real backend tests for simple, noninteractive operation cases. Native conflict, properties, OS-open, clipboard cut completion, and Recycle Bin recovery require an interactive Windows test session or documented human/manual evidence. Never mislabel mocked native dialogs as integration coverage.

Tests requiring a second volume, symbolic-link privilege, cloud account, case-sensitive directory, or native GUI have explicit capability checks. Report skipped reasons. Release acceptance requires the specified real cases to be executed on a suitable test machine; skipping in CI is not proof of support.

### 19.4 Frontend and native E2E

Vitest covers keyboard behavior, selection persistence, stale replies, error states, accessibility labels, and job/result UI. Playwright covers browser rendering with a deterministic bridge, including 100k virtualized data. Label those reports **UI with mocked backend**.

Native E2E launches the built executable through `tauri-driver` and matching Edge Driver and exercises actual navigation, create/rename/copy, job outcomes, indexing, refresh, and persistence. Native dialogs are verified through backend assertions plus the separate interactive checklist. Follow Tauri's current Windows WebDriver setup and CI guidance. [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/), [Tauri WebDriver CI](https://v2.tauri.app/develop/tests/webdriver/ci/)

The E2E harness sets an isolated application state directory and fixture root through a separately built test configuration. Test hooks must not exist in production. The suite must assert actual fixture filesystem changes, not just a toast message. Preserve screenshots, driver logs, and fixture manifest on failure.

### 19.5 Manual release checklist

- Install/run/uninstall as standard user on a clean Windows machine or VM.
- Test WebView2 present and absent installation paths; disclose download requirement.
- Navigate common folders; verify associated open and native Properties.
- Exercise collision dialogs, Recycle recovery, locked files, cancellation, and clipboard interoperability.
- Verify high DPI, keyboard-only navigation, Narrator basics, reduced motion, and high contrast.
- Test missing favorite/drive, sleep/resume, corrupt index, interrupted job recovery, and reinstall/upgrade preserving settings.
- Confirm no shell replacement, global association changes, unexpected network requests, or debug mutation commands.
- Record exact builds, outcomes, screenshots where useful, and remaining limitations.

## 20. Benchmarks and regression tracking

Keep Criterion benches inside relevant crates with explicit `harness = false`. Bench path conversion, sorting, pagination/filtering, query parsing, SQLite prepared queries, and watch coalescing. Use generated fixtures with known distributions, not one favorable filename.

`cargo xtask bench --profile mvp` must also run end-to-end measurements for listing, crawling, native copy, watcher latency, and process resources. Save machine-readable JSON/CSV plus a human summary under `artifacts/benchmarks/<timestamp>/`. Each record includes commit, executable hash/build mode, environment, seed, iterations, warm/cold label, p50/p95/max, and failures.

Copy comparison uses a minimal separate native Shell control harness with equivalent flags/locations and without app scheduling/IPC. Alternate order, verify output contents, and report variance. Do not use Explorer stopwatch guesses or cached transfers as definitive throughput claims.

On a stable dedicated runner, compare to a reviewed baseline and flag >15% p95 regressions where variance is smaller than the change. Shared CI runners report trends; they do not enforce absolute performance thresholds. Investigate flags with profiling (Windows Performance Recorder/Analyzer or another documented profiler) before accepting baseline updates. Updating a baseline is not a fix.

## 21. Continuous integration

Use GitHub Actions if the repository is hosted there; otherwise provide equivalent locally runnable scripts. Creating workflow files is authorized implementation work. Uploading/publishing releases or obtaining signing credentials is a separate external action requiring the owner's authorization.

### 21.1 Pull-request checks

On a supported Windows x64 runner:

1. Checkout; use Node LTS and the exact Rust toolchain from the repository.
2. Cache Rust/npm by toolchain and lockfile hashes; no secrets in caches.
3. `npm ci` and locked Cargo dependencies.
4. Format, Clippy, Rust tests, frontend typecheck/lint/unit tests.
5. Frontend production build and Windows desktop compilation.
6. Small/medium fixture integration suite; no mutations outside harness roots.
7. Dependency/license checks, with explicit reviewed exceptions.
8. Upload test reports and safe diagnostics on failure.

Pin third-party actions to reviewed full commit SHAs when creating final workflows. Use minimal `permissions: contents: read` by default. Do not give untrusted pull requests release/signing secrets. Set timeouts and cancel superseded runs. Ensure release features compile, not only the default debug/test feature set.

### 21.2 Native and scheduled checks

- Native E2E job with version-matched driver and real executable; if hosted runners cannot provide the necessary desktop session, use an isolated interactive/self-hosted test runner and make the remaining validation explicit.
- Nightly/scheduled 100k stress fixture, watcher flood, interrupted-process recovery, migration tests, and leak checks.
- Dedicated benchmark runner for comparable performance; attach raw metrics.
- Smoke NSIS build on Windows; unsigned CI artifacts are labeled as development builds.

CI must distinguish passed, failed, skipped due to capability, and unexecuted tests. A green browser suite does not substitute for native app acceptance.

### 21.3 Release workflow

Run manually or on an explicitly authorized version tag. Re-run checks, build release x64, create NSIS installer, generate SHA-256 checksums and dependency/license inventory, and stage artifacts for review. Sign only when credentials are provided and authorized; protect keys and never log them. Publish only after approval. Do not invent a certificate, publish an unsigned build as trusted, or enable auto-update without its separate design and signing policy.

## 22. Packaging, distribution, and runtime readiness

Package a per-user NSIS installer using Tauri's Windows bundler. NSIS is the MVP choice; MSI may need additional Windows tooling/features and is later. [Tauri Windows installers](https://v2.tauri.app/distribute/windows-installer/)

Required package behavior:

- Install to the current user's appropriate local applications directory and create a Start Menu shortcut.
- No administrator requirement for the app's normal installation or operation; explain any WebView2 setup constraint.
- Include an embedded WebView2 Evergreen bootstrapper or another documented, tested Tauri runtime mode. Disclose when setup needs internet; provide an offline alternative later if required.
- Set product name/version/publisher consistently, include owned icon assets and license notices, and use stable application identity.
- Preserve state on upgrade. Uninstall removes program files/shortcuts and keeps user settings/index unless the user explicitly chooses data removal.
- No Explorer hooks, default-folder reassociation, startup autorun, services, shell replacement, or global registry policy changes.
- Test application launch on a machine without Rust, Node, or developer SDKs.
- Verify all runtime DLL requirements on the clean machine; do not infer portability from a successful developer-machine launch.
- Mark unsigned MVP installers honestly; Windows trust warnings may occur. Production signing is a later distribution requirement if credentials are unavailable.

Release output directory contains installer, checksums, version/build metadata, license inventory, release notes, benchmark summary, validation report, and known limitations. Do not include local paths, fixture contents, cached private metadata, signing secrets, or debug state.

## 23. Coding standards and repository hygiene

### 23.1 Rust

- Small modules with clear ownership; use newtypes for session/job/plan/root/item IDs.
- No broad `unsafe` outside `explorer-win` and narrowly reviewed FFI implementations. Prefer `#![deny(unsafe_op_in_unsafe_fn)]` workspace-wide and `#![forbid(unsafe_code)]` in pure domain/index/store logic where feasible.
- Every unsafe block documents pointer validity, allocation/buffer lifetime, thread/apartment requirements, and ownership/release obligations.
- Never invent `Send`/`Sync` for COM objects or raw handles to bypass compiler errors.
- FFI callbacks must not panic or unwind; validate callback inputs and use nonblocking bounded sinks.
- Use `Result` and contextual errors; do not flatten native failures into strings prematurely.
- Async work is cancelable and bounded. Do not hold locks across `.await`; no blocking Win32 work in async command handlers.
- Use `std::process::Command` argument vectors for tooling; propagate exit status and sanitize logs.
- Format and Clippy pass without blanket `allow` attributes. Narrow exceptions need a comment and reason.

### 23.2 TypeScript/UI

- `strict` enabled; prohibit implicit `any`, centralize shared DTOs, and validate unknown IPC data at the boundary.
- Components present state; service bridge handles transport; Rust owns native/business rules.
- No direct filesystem/process APIs, embedded shell snippets, or unescaped HTML filenames.
- Respect cancellation/generation on every async UI request.
- Use semantic controls and accessible labels. Stable selection keys must survive sorting/filtering.
- Avoid global rerenders for frequent progress updates and large selection sets.
- Maintain deterministic scripts and committed lockfiles; no dependency churn during unrelated fixes.

### 23.3 Change discipline

Each change has a concrete behavior, meaningful validation, and updated docs where applicable. Do not bundle cleanup with a native correctness fix. Do not commit large generated artifacts, fixture files, logs, private paths, credentials, or installed runtimes. Record decisions in ADRs: context, chosen behavior, alternatives, consequences, and verification.

## 24. Milestones and phased roadmap

Milestones are dependency-ordered acceptance gates, not time estimates. Build one usable vertical slice at a time. Keep the app runnable after every milestone.

| Milestone | Deliverable | Required exit evidence |
|---|---|---|
| **M0 — Environment and scaffold** | Toolchain, workspace, blank real Windows window, command scripts, ADRs | Locked build succeeds; `doctor` passes; SQLite trigram probe; frontend/Rust checks and desktop launch |
| **M1 — Safe browsing** | Known folders/drives, address input, enumeration, details list, navigation, errors | Real-folder navigation; Unicode/long-path fixtures; no UI blocking; 1k listing target; native smoke |
| **M2 — Tabs and interaction** | History, favorites, selection, keyboard, theme, settings restore, open/properties | UI interaction suite; real associated open/properties checklist; stale-response tests; DPI check |
| **M3 — Native mutation vertical slice** | Create folder and rename, planner/tokens, journal/job queue, native STA | Fixture filesystem outcomes; invalid/stale name tests; apartment/lifetime tests; interrupted journal |
| **M4 — Everyday file operations** | Copy/move/recycle, native conflicts, cancellation, per-item results, clipboard | Correctness matrix for ordinary NTFS; Explorer clipboard both directions; collision/recycle recovery; source preservation |
| **M5 — Live changes** | Watch subscriptions, coalescing, overflow/resume recovery, explicit refresh | External-event convergence; forced overflow reconciliation; tab lifecycle leak check; latency report |
| **M6 — Indexed filename search** | Root opt-in, SQLite crawler/FTS, query scope, results, rebuild | Known-answer 100k query set; subtree rename/partial scan tests; index freshness; search/crawl benchmarks |
| **M7 — Hardening and performance** | Large-directory virtualization, bounded resources, security audit, failure UX | 100k stress report; profiling/fixes for misses; denied IPC tests; dependency checks; manual edge-case matrix |
| **M8 — Installable MVP** | NSIS installer, release docs, native E2E, clean-machine validation | Standard-user install/use/upgrade/uninstall; complete DoD checklist; staged deliverables with checksum |

### 24.1 First implementation tickets

The agent should turn these into checkbox items in `docs/agent/backlog.md` before expanding the roadmap:

1. Inspect environment/repo and create pinned workspace + scripts.
2. Compile and launch a real Tauri 2 Windows window; establish build/CI baseline.
3. Implement native path wrappers and errors with encoding/name tests.
4. Implement one-folder enumeration service with cancelable paging and typed IPC.
5. Render virtualized details rows; navigate Documents through the native known-folder API.
6. Add address navigation/history and stale-generation protection.
7. Add selection/keyboard/tabs and persisted preferences.
8. Implement dedicated COM STA and create-folder/rename job slice.
9. Add copy/move, item-result aggregation, native conflict handling, and cancellation.
10. Add guarded recycle-only behavior and prove recovery/no permanent fallback.
11. Add interoperable clipboard copy/cut/paste.
12. Add watches/reconciliation, then opt-in filename indexing/search.
13. Measure performance, fix demonstrated bottlenecks, and package/test on a clean machine.

### 24.2 After MVP

| Phase | Scope | Entry/exit rule |
|---|---|---|
| **v0.2 — Usability** | Dual pane, richer view options, optional icons, internal drag/drop, batch rename, better accessibility | MVP data-safety guarantees remain; new interactions tested |
| **v0.3 — Transfer control** | Experimental CopyFile2 backend, optional verification, richer progress/conflict UI, scoped undo research | Full filesystem semantic comparison and crash matrix before default enablement |
| **v0.4 — Search expansion** | Content extraction in isolated workers, format plugins, optional fuzzy ranking, USN journal research | Explicit privacy/resource policy; parser sandboxing; no admin requirement by default |
| **v0.5 — Native breadth** | OLE external drag/drop, native menus, Shell namespace/MTP/archive browsing, cloud-provider integration | Extension isolation/security plan and compatibility evidence |
| **v1.0 — Distribution** | Signed installers, secure optional updater, ARM64 and supported-OS matrix, sustained benchmark/regression process | Owner-controlled credentials; verified updates; release support commitments |

USN-based indexing is later: evaluate volume support, privilege requirements, journal rollover, and reconciliation before adopting it. No “Everything-like whole-disk index” claim in MVP.

## 25. Autonomous agent execution loop

### 25.1 Work state files

Maintain these as concise, factual records:

- `backlog.md`: milestone, ticket, dependencies, acceptance checks, status, blockers.
- `progress.md`: behavior implemented, files changed, decisions, next useful step.
- `validation.md`: exact commands, build/commit, date, outcome, evidence path, skipped capability/reason.
- `handoff.md`: environment, current milestone, unresolved failures, next commands, known risks; enough to resume after context loss.

Do not mark a milestone complete merely because source files exist. Every checked acceptance gate has evidence. Avoid huge repetitive logs; retain raw results in ignored artifacts and link concise summaries.

### 25.2 Repeat until acceptance

```text
1. Read repo instructions, this spec, current backlog, and last handoff.
2. Inspect the working tree; preserve unrelated changes.
3. Select the smallest unblocked ticket on the current milestone.
4. State the behavior and its acceptance check.
5. Inspect relevant source and pinned API documentation.
6. Implement one coherent vertical slice.
7. Run targeted checks; reproduce any failure independently.
8. Fix implementation/environment defects and rerun affected checks.
9. Run the milestone's relevant broader checks when the slice is stable.
10. If performance is implicated, measure/profile before optimizing.
11. Update backlog, validation, progress, and handoff with actual results.
12. Continue to the next dependency; do not stop at a plan or scaffold.
13. At MVP completion, run the full release acceptance/clean-machine checklist.
```

Keep progress updates concise and explain implemented behavior and remaining uncertainty. Routine reversible repo work proceeds autonomously. Installation/elevation follows the environment's approval mechanism. Never publish a release, alter the global shell, or operate on real personal files as an experiment without authorization.

### 25.3 Failure and blocker protocol

- Preserve the first useful failing trace and reproduction command.
- Classify: code defect, dependency mismatch, missing prerequisite, test capability, performance miss, or external credential/authorization.
- Repair root causes; do not disable checks, relax safety rules, replace native tests with mocks, or change expected results to fit a bug.
- After repeated failures, simplify the slice, consult official docs, create a minimal reproducer, or record an ADR. Do not repeatedly run the same unchanged failing command.
- When blocked by native environment, continue independent domain/frontend/docs work. Mark Windows behavior unverified and give the owner the precise missing requirement.
- Ask for a required privilege/credential only when necessary and after concrete reviewable work is ready. Do not treat missing approval as permission.
- If context/time limits interrupt execution, leave a truthful handoff and unfinished checklist. Do not declare the project done.

### 25.4 Suggested generated `AGENTS.md` contract

At M0, write a concise repository `AGENTS.md` that includes:

```markdown
# Agent contract
- Read the project specification and docs/agent/handoff.md before changing code.
- Implement Rust-owned filesystem behavior; keep UI/IPC adapters thin.
- Preserve lossless Windows paths; use native item tokens for mutation.
- Use the dedicated STA for Shell operations; never block the UI thread.
- All mutations use validated plans, journals, and explicit outcomes.
- Mutation tests use marked fixture roots only.
- Use the single per-user application executor; do not run competing mutation instances.
- Never silently fall back from Recycle to permanent deletion.
- Run targeted checks, then milestone gates; fix failures instead of disabling checks.
- Record real validation results and capability skips.
- Continue through milestone acceptance; maintain a resumable handoff.
- Do not publish externally or change global Windows shell settings without authorization.
```

## 26. Definition of done

### 26.1 Per ticket

- [ ] Required behavior implemented in the real production path.
- [ ] Relevant failure/cancellation/stale-input cases handled.
- [ ] Targeted tests/checks pass; actual Windows behavior validated when native APIs are involved.
- [ ] No unexplained warnings, new resource leaks, debug authority bypasses, or unrelated changes.
- [ ] Docs/DTOs/work-state updated as needed; no remaining TODO that hides required behavior.

### 26.2 MVP completion gate

- [ ] M0–M8 exit criteria are satisfied with evidence.
- [ ] Required user journey in section 1.1 works from an installed build under a standard account.
- [ ] Browsing, tabs, selection, keyboard navigation, favorites, and restored settings work.
- [ ] Copy/move/create/rename/recycle use real native backend and produce verified outcomes.
- [ ] Collision, cancellation, partial failure, locked file, unavailable location, and interrupted-job behavior are clear and safe.
- [ ] No silent overwrite policy, permanent recycle fallback, manual move-source deletion, or automatic crash retry exists.
- [ ] Explorer clipboard interoperability and supported associated-open/properties behavior are tested.
- [ ] Watcher normal events and forced overflow reconcile correctly; no index deletion from partial/offline scans.
- [ ] Opt-in indexed filename search works with root scope, stale-result validation, removal/rebuild, and known-answer tests.
- [ ] Real 100k folder/index fixtures complete without freeze or unbounded resource growth.
- [ ] Performance targets pass on the documented baseline, or each miss has an explicit approved waiver.
- [ ] Rust/frontend/Windows integration/native E2E checks pass; all capability skips are visible and release-required cases have separate evidence.
- [ ] IPC capabilities/CSP/input limits and path/reparse safety are verified.
- [ ] Dependency/license checks have no unreviewed relevant high/critical findings.
- [ ] NSIS installation, runtime dependencies, launch, upgrade, and uninstall pass on a clean Windows environment.
- [ ] Production artifact excludes test bypasses, secrets, personal metadata, and fixture state.
- [ ] README includes prerequisites, setup, run, test, benchmark, package, troubleshooting, and supported limitations.
- [ ] Installer/checksums/build metadata/validation report/release notes are staged locally and reviewable.
- [ ] Remaining non-MVP work is clearly in the backlog; no required work is mislabeled later to declare success.

If any mandatory gate remains unverified, report **MVP incomplete** with the exact remaining item. A usable partial build may still be delivered, accurately labeled.

## 27. Source references and documentation policy

The links below support API/tool behavior, not the project's proposed performance targets or architectural guarantees. Consult the version-specific primary documentation when implementing. This specification deliberately delegates current patch-version resolution to M0.

| Topic | Primary reference |
|---|---|
| Tauri Windows prerequisites | [Prerequisites](https://v2.tauri.app/start/prerequisites/) |
| Tauri security/capabilities | [Capabilities](https://v2.tauri.app/security/capabilities/), [Permissions](https://v2.tauri.app/security/permissions/), [Runtime authority](https://v2.tauri.app/security/runtime-authority/) |
| Tauri native testing | [WebDriver](https://v2.tauri.app/develop/tests/webdriver/), [WebDriver CI](https://v2.tauri.app/develop/tests/webdriver/ci/) |
| Windows installer/runtime modes | [Tauri Windows installer](https://v2.tauri.app/distribute/windows-installer/) |
| Rust Windows API bindings | [Microsoft windows-rs](https://github.com/microsoft/windows-rs) |
| Native operation engine | [IFileOperation](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation) |
| Native operation flags | [SetOperationFlags](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags) |
| Operation outcomes/progress | [IFileOperationProgressSink](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperationprogresssink), [PreDeleteItem](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperationprogresssink-predeleteitem) |
| Later transfer backend | [CopyFile2](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-copyfile2) |
| Native watcher limitations | [ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw) |
| Long paths | [Maximum path length limitation](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation) |
| Clipboard interoperability | [Shell clipboard formats](https://learn.microsoft.com/en-us/windows/win32/shell/clipboard) |
| Watch implementation | [notify](https://github.com/notify-rs/notify) |
| Filename index | [SQLite FTS5](https://www.sqlite.org/fts5.html) |

**First action for the implementing agent:** perform the M0 environment audit, create `docs/agent/backlog.md`, scaffold the pinned workspace, and launch a real Windows desktop window. Then continue through the milestone gates without treating initial scaffolding as completion.
