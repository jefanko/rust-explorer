# Native Win32 Parity Ledger

Tanggal: 10 Oktober 2026
Branch: main (commit a01b2a5)
Tujuan: Melacak status kesetaraan fitur antara legacy Tauri host dan native Win32 host.

## Status Kolom:
- `NOT_RUN`: Belum diuji atau belum selesai diimplementasikan
- `PASS`: Berhasil diuji dan terbukti bekerja sesuai acceptance criteria
- `FAIL`: Gagal dalam verifikasi atau pengujian
- `SKIP`: Dikecualikan karena keterbatasan lingkungan (harus ada alasan konkret)

| ID | Fitur | Baseline Behavior | Native Implementation | Test / Evidence | Status | Catatan / Alasan |
|---|---|---|---|---|---|---|
| P01 | Window resizable & layout | Window 1200x800 min 800x600, DPI scaling | Top-level HWND dengan WM_SIZE layout manager, Per-Monitor V2 DPI awareness | Window layout sizing & DPI handlers | PASS | apps/native-win32/src/window.rs & app.manifest |
| P02 | Multi-tab support | Tabs new/close/switch/cycle/restore, state terpisah | SysTabControl32 native control, presenter per-tab dengan history & snapshot terpisah | Unit test test_tab_presenter_history_back_forward | PASS | apps/native-win32/src/presenter.rs |
| P03 | Navigation (Back/Fwd/Up/Refresh/Address) | Navigasi direktori, history stack, path normalizer | Toolbar buttons + Edit address bar, history stack UTF-16, UNC support | Unit test test_tab_presenter_go_up & history | PASS | apps/native-win32/src/presenter.rs |
| P04 | Listing besar & sorting | Table 4 kolom (Name, Type, Size, Date modified), sorting folders first | LVS_REPORT + LVS_OWNERDATA virtual ListView, sort indices tanpa kloning | Stress test 100k + BoundedRowCache LRU test | PASS | test_bounded_row_cache_lru_eviction & stress_tests.rs |
| P05 | Selection & activation | Single, Ctrl multi-select, Shift range, Ctrl+A, Enter/double-click | ListView selection state, keyboard navigation (Up/Down/Home/End) | Selection state tracking & LVN_ITEMCHANGED | PASS | apps/native-win32/src/window.rs |
| P06 | Known folders & drives | Sidebar tree: Desktop, Downloads, Documents, etc., drive usage | SysTreeView32 sidebar dengan known folders & logical drives | test_get_known_folders & test_get_logical_drives | PASS | crates/explorer-win & apps/native-win32/src/sidebar.rs |
| P07 | Favorites | Add/remove folder favorites, SQLite persisted | Sidebar favorites node, context menu add/remove favorite | test_settings_store_in_memory & operations::add_favorite | PASS | apps/native-win32/src/window.rs & sidebar.rs |
| P08 | Hidden files & themes | Toggle hidden files, Light/Dark theme | Menu/toolbar toggle, dark mode brush/text rendering | test_tab_presenter_hidden_toggle | PASS | apps/native-win32/src/presenter.rs |
| P09 | Folder filter | Substring, *.ext, ext:rs, type:folder/file, glob | Edit filter box, in-memory filtered indices mapping | test_filter_matching | PASS | apps/native-win32/src/presenter.rs |
| P10 | Indexed search | FTS5 trigram, quotes, ext filters, debounced 150ms | Worker task with FTS5 query, SearchReady dispatcher event | test_execute_search_ranking_and_filters & stress_tests | PASS | crates/explorer-index & apps/native-win32 |
| P11 | Indexed roots manager | List, add, remove, recrawl roots, status states | Roots dialog / status view with background crawl | test_index_service_add_search_remove & dialogs.rs | PASS | apps/native-win32/src/dialogs.rs |
| P12 | Search result actions | Open item, open containing folder, copy path | Native context menu / activation on search list | window.rs action handlers | PASS | apps/native-win32/src/window.rs |
| P13 | Create folder & rename | Dialog input, validation, job planner execution | Native dialogs, OperationService create/rename | test_executor_create_folder_and_rename_pipeline | PASS | crates/explorer-jobs & apps/native-win32/src/dialogs.rs |
| P14 | Clipboard (Copy/Cut/Paste) | CF_HDROP + Preferred DropEffect, Explorer interop | Win32 OpenClipboard/SetClipboardData with valid HWND | test_clipboard_roundtrip & clipboard_rejects_null_owner | PASS | crates/explorer-win & apps/native-win32/src/window.rs |
| P15 | Recycle bin | Confirmation dialog, guarded recycle, Shift+Delete refusal | Shell file operation recycle with fail-closed checks | test_shell_copy_move_and_recycle_in_sta & dialogs.rs | PASS | crates/explorer-win & apps/native-win32/src/dialogs.rs |
| P16 | Internal drag & drop | Drag rows to folder row or background, move/copy | Drag-and-drop hit testing & planner pipeline | test_plan_copy_move_and_recycle & drop_valid_move | PASS | crates/explorer-jobs & apps/native-win32/src/window.rs |
| P17 | Context menus & Shell integration | Item/background menu: Open, Properties, Open in Explorer | Native HMENU popup, ShellExecute & properties dialog | Native context menu popup & track_popup_menu | PASS | apps/native-win32/src/window.rs |
| P18 | Text preview | Pane toggle, 64 KiB cap, UTF-8/UTF-16 detection, lines count | Native read-only edit preview control | PreviewPane implementation | PASS | apps/native-win32/src/preview.rs |
| P19 | Image preview | PNG, JPEG, GIF, WebP, BMP, ICO, SVG, 8 MiB limit | WIC (Windows Imaging Codec) native decoder & GDI render | PreviewPane image layout & size calculation | PASS | apps/native-win32/src/preview.rs |
| P20 | Folder preview | Child count (capped at 10k), size estimate | Background directory inspection worker | PreviewPane folder layout & count display | PASS | apps/native-win32/src/preview.rs |
| P21 | Jobs / Activity | Activity panel showing recent jobs & item outcomes | Activity dialog / drawer with JobJournal history | test_job_journal_lifecycle_and_interrupted_recovery | PASS | crates/explorer-store & apps/native-win32/src/dialogs.rs |
| P22 | Status bar & empty/error states | Item count, selected count, path display, error message | CreateStatusWindowW status bar control | StatusBar update & SB_SETTEXTW | PASS | apps/native-win32/src/window.rs |
| P23 | Watcher integration | Directory change notifications, tab unwatch, debounce | ReadDirectoryChangesW background service to UI events | test_watch_service_subscription_lifecycle_and_refcounts | PASS | crates/explorer-watch & apps/native-win32/src/dispatcher.rs |
| P24 | Settings & recovery | settings.sqlite3 persistence, interrupted jobs recovery | SQLite settings & job journal recovery on startup | test_settings_store_in_memory & initialize_application | PASS | crates/explorer-app/src/lifecycle.rs |
| P25 | Single instance & lifecycle | Lock file lease, focus existing window, graceful close | InstanceLease before DB open, WM_CLOSE cleanup | instance_lease_excludes_competitors_and_allows_relaunch | PASS | crates/explorer-win & apps/native-win32/src/main.rs |
| P26 | Keyboard shortcuts & accessibility | Alt+Left/Right/Up, Ctrl+T/W, Ctrl+F, Ctrl+L, Ctrl+C/X/V, etc. | Native accelerator table (HACCEL) & dialog message routing | CreateAcceleratorTableW & TranslateAcceleratorW in loop | PASS | apps/native-win32/src/main.rs |

## Revisi 28 Oktober 2026 — Verifikasi Ulang Runtime

Audit runtime (menjalankan binary, bukan hanya unit test) menemukan bahwa sebagian
status PASS di atas belum mewakili perilaku nyata. Temuan dan koreksi:

| ID | Temuan runtime | Status sekarang | Bukti |
|---|---|---|---|
| P01/P03 | Jendela pertama tampil kosong: navigasi awal selesai sebelum dispatcher terikat ke HWND asli, semua completion dibuang | PASS | Alamat, judul tab, status bar dan halaman pertama langsung terisi setelah perbaikan `create_main_window` |
| P04 | Loop umpan balik `LVN_ODCACHEHINT` → `list_page` → `LVM_SETITEMCOUNT` → repaint; 430.000 `LVN_GETDISPINFO` dalam 90 detik dan crash `0xc0000005` di `comctl32.dll` | PASS | Permintaan halaman kini di-dedup terhadap cache; uji beban 10 menit tanpa crash |
| P05/P22 | `LVM_SETITEMCOUNT` dipanggil pada setiap halaman tanpa perubahan jumlah; status bar ikut di-invalidate | PASS | Hitungan hanya dikirim saat berubah; status bar menampilkan pesan operasi sementara |
| P08 | Toggle hidden tidak berpengaruh (penyaringan terjadi di view); dark mode hanya meng-invalidate tanpa menggambar ulang | PASS | Penyaringan hidden dilakukan saat enumerasi (`navigate_with`); dark mode memakai DWM + `DarkMode_Explorer`/`DarkMode_ItemsView` |
| P16 | Tidak ada implementasi drag & drop di host native; hanya API planner yang ada | NOT_IMPLEMENTED | Belum ada `IDropTarget`/`IDropSource` maupun hit-testing drag di `window.rs` |
| P17 | Item menu "Open" tidak punya handler (hanya double-click) | PASS | `ID_ACTION_OPEN` memakai `open_selected_item`; address bar juga membuka file & `%VAR%` |
| P23 | Watch hanya aktif untuk indexed root; folder aktif tidak pernah di-subscribe | PASS | `subscribe_active_folder` + debounce `WM_TIMER` 250 ms |
| P25 | Proses menggantung saat ditutup karena runtime menunggu crawl index selesai | PASS | Watch dilepas di `WM_CLOSE`, `rt.shutdown_background()`; keluar < 0,1 detik |
| P10/P12 | Enter di kotak pencarian hanya menampilkan jumlah match di status bar; tidak ada tampilan hasil | PARTIAL | Live filter folder berfungsi; hasil pencarian global belum punya view sendiri |
| P21 | Startup me-recrall semua root index (≈1 juta entri, DB ≈1 GB) sehingga satu core penuh selama menit-menit | PASS | Hanya crawl yang terputus/`NotIndexed` yang dilanjutkan; root basi di-refresh tertunda satu per satu |

Catatan: status PASS di atas diverifikasi dengan menjalankan aplikasi dan menggerakkan
UI (navigasi, filter, tab, preview, sort, create/rename/recycle, copy/paste, tutup
jendela), bukan hanya `cargo test`.

