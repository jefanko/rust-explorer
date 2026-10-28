# Native Win32 Migration Progress Log

Tanggal: 10 Oktober 2026
Branch: main
HEAD: a01b2a50443d16e07a79ffcbfe77c809a64aface

## Log Milestone

### M0 — Inventory, Baseline, dan Rencana Verifikasi
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Verifikasi toolchain: rustc 1.99.0, cargo 1.99.0, node v22.20.0, npm 10.9.3, git 2.53.0.
  - Verifikasi suite tes baseline: 60 tes unit/stress pada workspace Rust semuanya lulus (`cargo test --workspace --locked -- --test-threads=1 --skip test_clipboard_roundtrip`).
  - Verifikasi `cargo xtask doctor`: Semua check lolos (git, node, npm, SQLite FTS5 trigram, manifest consistency).
  - Dibuat `NATIVE_WIN32_PARITY.md` untuk melacak 26 acceptance requirement (P01-P26).
  - Dibuat `NATIVE_WIN32_PROGRESS.md` untuk mencatat kemajuan per milestone.

### M1 — Ekstraksi Application Layer (`crates/explorer-app`)
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Dibuat `crates/explorer-app` yang memuat durable `AppState`, `operations`, `requests`, `lifecycle`, dan `events` tanpa dependency Tauri.
  - Dilakukan optimasi memory-efficiency pada `crates/explorer-fs/src/snapshots.rs` dan `listing.rs` (indeks `u32` dan `token_map` efisien).
  - Di-update `legacy/desktop/src-tauri` (`commands.rs`, `lib.rs`, `state.rs`) sebagai thin adapter ke `explorer-app`.
  - Verifikasi: Semua 65 tes unit dan stress test Rust pada workspace lulus (`cargo test --workspace --locked -- --test-threads=1 --skip test_clipboard_roundtrip`).
  - Verifikasi: UI unit tests (13/13 vitest) dan `build:ui` tetap lulus tanpa regresi.

### M2 — Native Shell dan Lifecycle Dasar (`apps/native-win32`)
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Dibuat package `apps/native-win32` dengan manifest Common Controls v6, per-monitor DPI v2, long paths enabled.
  - Diimplementasikan RAII wrappers untuk font, brush, bitmap, dan icon (`resources.rs`).
  - Diimplementasikan single-instance lease locking via `InstanceLease` dan `WM_APP_WAKEUP` coalesced dispatcher queue (`dispatcher.rs`).
  - Diimplementasikan main window procedure dan accelerator table routing (`main.rs`, `window.rs`).
  - Unit tests lulus untuk dispatcher queue bounds dan batch draining.

### M3 — Browsing Lengkap (Virtual ListView, Paging, Tabs)
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Diimplementasikan virtual listview `LVS_OWNERDATA` + `LVS_REPORT` dengan callback `LVN_GETDISPINFOW` dan `LVN_ODCACHEHINT` (`window.rs`).
  - Diimplementasikan multi-tab control (`SysTabControl32`) dan `TabPresenter` dengan cache baris LRU (`BoundedRowCache`), history back/forward/up navigation, dan index mapping (`presenter.rs`).
  - Diimplementasikan in-folder live filter (`ext:`, `type:`, `*.ext`, substring) tanpa re-fetching ke disk.
  - Diimplementasikan address bar breadcrumb / edit navigasi (Enter / Esc) dan sorting kolom tanpa kloning snapshot.
  - Unit tests lulus untuk history back/forward, go up, bounded cache LRU, dan filter matching.

### M4 — Daily Feature Parity (P01–P26)
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Diimplementasikan Sidebar tree (`SysTreeView32`) memuat Quick Access Known Folders, Logical Drives (dengan kapasitas bebas/total GB), dan Favorites (`sidebar.rs`).
  - Diimplementasikan Preview pane (`preview.rs`) untuk file teks (64 KiB cap), gambar, dan direktori.
  - Diimplementasikan Native Dialogs (`dialogs.rs`): prompt folder name/rename, konfirmasi recycle bin, penolakan Shift+Delete permanent delete, Indexed Roots manager dialog, dan Job Activity viewer dialog.
  - Diimplementasikan Context Menu native (`HMENU`) dan keyboard shortcuts (Ctrl+T, Ctrl+W, Ctrl+F, Ctrl+L, Ctrl+C/X/V, Alt+Left/Right/Up, F5).
  - Diimplementasikan dark mode toggling dan hidden files visibility toggling.
  - Verifikasi: Seluruh 72 tes workspace Rust lulus tanpa error.

### M5 — Optimasi Memori dan Pengukuran RAM
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Dibuat `scripts/measure-memory.ps1` untuk mengukur Working Set dan Private Bytes secara otomatis.
  - Hasil pengukuran `rust-explorer.exe` (release build):
    - Fresh Launch (Idle 3s): Working Set **26.64 MB**, Private Bytes **10.87 MB**
    - Settle (Idle 8s): Working Set **26.47 MB**, Private Bytes **10.88 MB**
    - Kriteria lolos: **26.47 MB <= 60.0 MB** threshold
    - Penghematan RAM dibandingkan baseline Tauri (150-350 MB): **~86.8% penghematan memori**!
  - 100k items stress test (`crates/explorer-fs/tests/stress_tests.rs`) terverifikasi lulus dalam 2.76 detik dengan snapshot token indexing efisien.

### M7 — Hardening Runtime (Audit Crash & Bug Pemakaian Nyata)
- **Status**: SELESAI
- **Latar belakang**: pemakaian normal (bukan unit test) menunjukkan jendela kosong saat
  start, crash `0xc0000005` di `comctl32.dll` setelah beberapa menit, dan proses tidak
  mau tertutup. Audit dilakukan dengan menjalankan binary debug/release, mengambil
  stack lewat crash handler baru (DbgHelp + PDB), dan menggerakkan UI lewat input
  keyboard serta pesan Win32.
- **Akar masalah yang ditemukan**:
  - Navigasi awal dijalankan di `WM_CREATE`, sebelum `Dispatcher` diikat ke HWND asli;
    36+ pesan completion hilang sehingga alamat, tab, dan daftar kosong.
  - `LVN_ODCACHEHINT` meminta ulang halaman yang sudah ada di cache, dan setiap
    halaman memicu `LVM_SETITEMCOUNT` + `InvalidateRect`. Terbentuk loop repaint
    (~430 ribu `LVN_GETDISPINFO` per 90 detik) yang berujung AV di `comctl32`.
  - `WM_CLOSE` tidak menghentikan runtime Tokio; proses menunggu crawl index selesai.
  - Dialog modal (MessageBox/prompt/popup) memompa pesan sehingga `wnd_proc` masuk
    kembali ke state machine dan meminjam `WindowState` untuk kedua kalinya.
  - Panic Rust di `extern "system"` mematikan proses tanpa jejak.
  - Startup me-recrall semua indexed root (≈1 juta entri, DB ≈1 GB) setiap kali.
- **Perbaikan**:
  - Jendela dibuat tersembunyi, dispatcher diikat, font+layout diterapkan, baru
    `ShowWindow` dan navigasi awal; `Dispatcher::set_hwnd` menangani wakeup tertunda.
  - Cache halaman menjadi sumber kebenaran: `request_page` menolak halaman resident;
    hitungan item hanya dikirim bila berubah; cache diperbesar ke 24 halaman.
  - Penyaringan hidden/system dipindah ke enumerasi (`navigate_with`), sehingga indeks
    baris dan offset halaman tetap sinkron.
  - Guard modal (`in_modal`) menunda `WM_APP_WAKEUP`/`WM_COMMAND`/`WM_NOTIFY`/`WM_TIMER`
    selama dialog berjalan; `catch_unwind` membungkus window procedure.
  - `WM_CLOSE` melepas watch, lalu `rt.shutdown_background()` saat keluar.
  - Restore indexed root hanya melanjutkan crawl yang terputus dan melewati root ganda.
  - Fitur harian ditambahkan/diperbaiki: ikon shell, dark mode native, sort tanpa
    enumerasi ulang, watch folder aktif + debounce, buka file dari address bar,
    sort arrow, empty-state, DPI per-monitor, ukuran minimum, status bar non-modal,
    ID tab monotonik, rollback navigasi gagal.
  - Diagnostik permanen: `%LOCALAPPDATA%\RustExplorer\logs\rust-explorer.log` berisi
    panic, unhandled exception dengan stack tersimbolisasi, dan breadcrumb pesan bila
    `RUST_EXPLORER_TRACE=1`.
- **Verifikasi**:
  - Crash handler menangkap dan men-simbolkan stack `comctl32.dll` (root cause loop).
  - Uji interaktif: navigasi address/history/up, filter `prog` (124 → 2 item), tab
    baru/tutup, preview teks, sort kolom, create/rename/recycle, copy/paste, tutup
    jendela 0,06 detik.
  - `cargo test --workspace --locked`: 87 tes lulus; `cargo build --release` bersih.

### M6 — Packaging, Pemindahan Old Host ke Legacy Folder, dan Cutover Final
- **Status**: SELESAI (PASS)
- **Aktivitas**:
  - Pindahkan host Tauri lama dari `apps/desktop` ke `legacy/desktop` dengan struktur lengkap dipertahankan.
  - Perbarui root `Cargo.toml` sehingga `rust-explorer` (`apps/native-win32`) menjadi default workspace member.
  - Perbarui `xtask/src/versions.rs` dan root `package.json` untuk menyelaraskan manifest check dengan `legacy/desktop`.
  - Perbarui `scripts/package.ps1` untuk menghasilkan executable mandiri `target/release/rust-explorer.exe`, `SHA256SUMS.txt`, dan `BUILD_METADATA.json`.
  - Buat dan perbarui dokumentasi `README.md`, `ARCHITECTURE.md`, dan `RUNBOOK.md`.
  - Verifikasi final:
    - `cargo xtask doctor` lulus 100%.
    - `cargo test --workspace` lulus 100% (72 tes passing).
    - `cargo build` dan `cargo run` langsung membangun dan meluncurkan binary native Win32.
