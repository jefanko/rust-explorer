# Handover agent: migrasi Rust Explorer ke UI Win32 native

Tanggal: 10 Oktober 2026, Asia/Jakarta. Status: **spesifikasi implementasi; migrasi belum dikerjakan dalam handover ini**.

## 1. Tujuan dan cakupan

Migrasikan aplikasi Rust Explorer dari Tauri 2 + React/TypeScript + WebView2 menjadi executable Rust dengan UI Win32 native. Pertahankan kapabilitas, fitur, interoperabilitas Windows Explorer, dan keselamatan operasi file. Tujuan performanya adalah mengurangi konsumsi RAM, terutama overhead browser dan salinan data UI, dengan hasil yang terukur.

Keputusan implementasi:

- Gunakan HWND, message loop Win32, dan Windows Common Controls melalui crate `windows` yang sudah dipakai repository. Mulai dari versi workspace `0.58`; cocokkan signature dengan versi terkunci di `Cargo.lock`.
- Gunakan kontrol native untuk tabel file, sidebar, tabs, address/search, toolbar, status bar, menu, dan dialog. Native drawing terbatas boleh dipakai untuk preview, ikon, dan theme.
- Pertahankan Rust backend yang ada. Ekstrak orchestration yang masih melekat pada Tauri, bukan menulis ulang engine filesystem.
- Build final harus berjalan tanpa Tauri, React, JavaScript runtime, WebView2, Node/npm, atau web server. Native codec/image renderer boleh menjadi dependency apabila diperlukan untuk mempertahankan preview.
- Tampilan boleh menyesuaikan kontrol Windows; kesetaraan fitur, keyboard, tema, aksesibilitas, dan DPI wajib. Layout CSS yang identik bukan syarat.
- Jangan menghemat RAM dengan menonaktifkan indexing, watcher, multi-tab, preview, atau fitur lain. Jangan menyatakan migrasi selesai hanya karena jendela native sudah terbuka.

Handover ini mengarahkan pekerjaan lokal yang dapat direview. Publishing, perubahan global Windows Shell/file association, dan penghapusan data pengguna bukan bagian pekerjaan.

## 2. Baseline yang diverifikasi saat penyusunan

| Item | Kondisi aktual |
| --- | --- |
| Checkout yang diperiksa | `D:\dev\rust-explorer` |
| Branch | `main` |
| HEAD | `a01b2a50443d16e07a79ffcbfe77c809a64aface` |
| Git sebelum dokumen dibuat | Tidak ada perubahan yang dilaporkan `git status --short` |
| Host sekarang | Package Cargo `rust-explorer`, di `apps/desktop/src-tauri` |
| UI | `apps/desktop/ui/src`, React 18 + TanStack Virtual |
| Target yang didokumentasikan | Windows 11 x86-64, MSVC |
| Native host / application crate usulan | Belum ada `apps/native-win32` atau `crates/explorer-app` di checkout ini |
| Database | `%LOCALAPPDATA%\RustExplorer\state.sqlite3` dan `index.sqlite3` |
| Acceptance MVP menurut handoff lokal | Masih terbuka; jangan mengubah catatan historis menjadi klaim lulus |

Verifikasi ulang branch, HEAD, working tree, toolchain, dan kode sebelum implementasi. Angka di atas merupakan snapshot, bukan perintah untuk reset checkout.

Ada panduan lokal lama `docs/agent/native-win32-migration.md` bertanggal 6 Oktober 2026. Panduan itu menyebut branch/worktree di lokasi lain dan agent prompt yang tidak ditemukan di checkout ini. Jangan menganggap lokasi tersebut masih tersedia atau langsung pindah ke sana. Gunakan dokumen ini untuk handover terkini; panduan lama boleh menjadi referensi tambahan setelah validasi.

Seluruh `docs/` diabaikan oleh `.gitignore`, dan `git ls-files docs` kosong pada baseline ini. Dokumen ini sengaja berada di root agar terlihat di Git tanpa perubahan aturan ignore atau staging paksa. Dokumen lanjutan yang perlu ikut checkout agent harus berada pada path yang tracked, atau memakai pengecualian ignore yang sempit dan eksplisit. Jangan bergantung pada dokumen lokal ignored sebagai satu-satunya tempat kontrak keselamatan.

### Sumber kode yang perlu dibaca

| Sumber | Tanggung jawab / alasan dibaca |
| --- | --- |
| `apps/desktop/src-tauri/src/state.rs` | Inisialisasi services, state directory, durable journal, recovery |
| `apps/desktop/src-tauri/src/lib.rs` | Lease instance, publikasi HWND, watcher events, restore indexed roots, reconciliation |
| `apps/desktop/src-tauri/src/commands.rs` | Kontrak operasi UI, resolusi token, blocking work, clipboard owner |
| `apps/desktop/ui/src/bridge/client.ts` dan `types.ts` | Checklist seluruh command dan bentuk hasil/event |
| `apps/desktop/ui/src/app/App.tsx` | Perilaku interaksi, shortcut, paging, drag/drop, search, settings |
| `apps/desktop/ui/src/state/tabs.ts` | History, path native, selection, focus, scroll, sort, filter per tab |
| `apps/desktop/ui/src/components/*.tsx` | Tabs, sidebar, toolbar, menu, dialog, activity, preview, status |
| `apps/desktop/ui/src/lib/{filter,searchQuery,selection,paths,recycle,format}.ts` | Semantik filtering, validasi query, selection, path, error dan format |
| `crates/explorer-fs/src/{lib,snapshots,listing,preview}.rs` | Snapshot, generation, batas paging, sorting, preview |
| `crates/explorer-jobs/src/{queue,planner,executor,shell_backend}.rs` | Plan, journal, commit, identitas native, hasil per item |
| `crates/explorer-win/src/{com,instance,clipboard,path,shell,sink}.rs` | STA, lease, clipboard/Shell, UTF-16 dan guarded recycle |
| `crates/explorer-index`, `explorer-watch`, `explorer-store` | Search, root recovery, watcher, persistence |
| `scripts/{check,package,bootstrap}.ps1`, `xtask/src/{doctor,versions,bench}.rs`, `.github/workflows/ci.yml` | Build, prerequisite, packaging, audit, CI dan metadata |
| `apps/desktop/tests/native-e2e/runner.js` | Smoke lama mengukur proses utama Tauri, bukan UI Win32 baru |

Jika tersedia, baca `docs/agent/AGENTS.md`, `handoff.md`, `objective-code-review-2026-10-04.md`, `validation.md`, dan `backlog.md`. Spec asal yang disebut handoff belum ditemukan dalam tracked files saat audit ini; catat sebagai sumber yang belum tersedia, jangan mengarang isinya. Kontrak keselamatan utama sudah disalin ke bagian 7 agar handover tetap berguna tanpa folder docs lokal.

### Temuan yang memengaruhi migrasi

1. `loadDirectory` membaca halaman 256 item sampai seluruh folder terkumpul dalam `allEntries`, lalu menyimpan `entries` per tab. TanStack hanya memvirtualisasi rendering; dataset React tetap lengkap. Native host harus memakai halaman sesuai viewport.
2. `FolderSnapshot` menyimpan entries asli, `HashMap<ItemToken, PathBuf>` untuk seluruh item, dan clone penuh entries dalam sorted cache. Menghapus WebView2 saja belum menghilangkan duplikasi backend ini.
3. `FolderService` membatasi 16 snapshot dan 256 item per halaman. Pruning memilih `created_at` paling lama; bukan LRU berdasarkan akses meskipun komentarnya menyebut LRU. Tidak ada byte budget atau pin ownership tab pada API sekarang.
4. Preview text dibatasi 64 KiB; gambar dibatasi 8 MiB file encoded dan dikirim sebagai Base64. Batas file tidak membatasi ukuran decoded pixels. Folder preview berhenti menghitung pada 10.000 item; jangan menyebut hasil itu total eksak folder besar.
5. Filter folder bekerja pada listing folder saat ini. Tombol Search subfolders beralih ke indexed search, dapat menambah root jika belum tercakup, lalu pencarian aktual memakai `rootId = null` untuk seluruh indexed roots. README tidak cukup untuk menentukan semantik ini.
6. Folder filter dan history tersimpan per tab. Indexed query/scope, preview toggle, dan activity toggle sekarang merupakan state global UI. Saved tabs hanya menyimpan path; full history/selection/scroll belum dipersist lintas restart.
7. README menyebut startup sekitar 31 MiB, tetapi runner mengambil `WorkingSet64` hanya dari `child.pid`. Angka ini tidak membuktikan total RAM aplikasi beserta proses WebView2.
8. `xtask bench` menjalankan stress tests lalu menulis summary dengan angka hardcoded. Gunakan output pengukuran aktual; summary tersebut bukan benchmark desktop baru.

## 3. Arsitektur target

```text
apps/native-win32                 binary sementara: rust-explorer-native
  HWND / Common Controls / menu / dialog / native preview
  presenter Rust, tab state, page cache, dispatcher UI
                      |
              typed requests + completions
                      v
crates/explorer-app               tanpa dependency Tauri
  durable AppState + runtime ownership + lifecycle
  orchestration navigation/settings/plans/clipboard/watch/index
                      |
  explorer-domain    explorer-fs      explorer-index
  explorer-store     explorer-watch   explorer-jobs
                                            |
                                       explorer-win
                                  dedicated file-operation STA

apps/desktop/src-tauri -> adapter sementara ke explorer-app
```

`explorer-app` memiliki services dan orchestration lintas host. Native host memiliki HWND, fokus, kontrol, rendering, dialogs, dan runtime lifecycle. Native host mendaftarkan konteks owner/session yang tervalidasi untuk kebutuhan clipboard/Shell; angka HWND dari caller bukan bukti otorisasi. Tetap gunakan typed Rust values tanpa JSON serialization pada jalur internal native.

Susunan file usulan, dibuat ketika slice membutuhkannya:

```text
crates/explorer-app/src/{lib,state,operations,lifecycle,events}.rs
apps/native-win32/Cargo.toml
apps/native-win32/build.rs
apps/native-win32/resources/{app.manifest,app.rc,icon.ico}
apps/native-win32/src/{main,window,controls,presenter,dispatcher}.rs
apps/native-win32/src/{list_view,tabs,sidebar,dialogs,preview,resources}.rs
scripts/check-native.ps1
scripts/package-native.ps1
scripts/measure-memory.ps1
NATIVE_WIN32_PARITY.md
NATIVE_WIN32_PROGRESS.md
artifacts/native-ui/<run-id>/
```

Tambahkan `apps/native-win32` secara eksplisit ke workspace. `crates/*` sudah mencakup application crate baru; tambahkan workspace dependency jika dibutuhkan. Jangan membuat seluruh file sebagai stub kosong lalu menganggap itu milestone selesai.

### Threading dan dispatch

- UI thread memiliki semua HWND dan presenter; callback tidak melakukan enumerasi, sort besar, SQLite, crawl, decode image, atau menunggu `StaWorker::execute`.
- Gunakan satu runtime aplikasi milik host untuk mengganti `tauri::async_runtime`. Jalankan pekerjaan synchronous melalui worker/blocking pool dengan admission yang terbatas, bukan task/thread tanpa batas.
- Pertahankan file-operation STA yang ada. Jika memakai OLE drag/drop, UI STA diinisialisasi secara terpisah; COM interface yang apartment-bound tidak boleh dipindah sembarang thread.
- Request baca membawa `session_id`, `tab_id`, `request_id`, dan `view_epoch`. Page juga membawa `folder_token`, `generation`, sort, filter, offset; preview membawa item token. Abaikan hasil lama setelah tab ditutup, navigasi, sort/filter, atau selection berubah.
- Gunakan completion queue yang memiliki payload Rust dan wakeup `WM_APP + N`. Hindari raw heap pointer di `LPARAM` yang lifetime-nya tidak jelas. Drain dengan batch terbatas agar paint/input tetap diproses.
- Batasi jumlah dan bytes queue. Coalesce watch/progress yang dapat diganti; mutation outcome harus tetap reliable melalui journal dan delivery yang terjamin. Overflow/lag harus memicu rescan/reconcile, bukan hilangnya perubahan.
- Tangani enqueue/drain wakeup race, kegagalan `PostMessageW`, shutdown, dan payload oversize. Membatalkan async task tidak menjamin closure `spawn_blocking` yang sedang berjalan berhenti.
- Jangan unwind panic melintasi callback Win32. Audit `unsafe`, buffer UTF-16, lifetime window state, reentrancy, handle affinity, dan destroy-once. Jangan menahan mutable borrow atau lock melewati `SendMessageW`/dialog yang dapat masuk kembali ke WndProc.
- Message loop membedakan `GetMessageW > 0`, `== 0`, dan `== -1`. Routing accelerator/dialog/focus harus menghormati kontrol edit sebelum dispatch command file.

## 4. Matriks parity wajib

Agent membuat `NATIVE_WIN32_PARITY.md` dengan kolom ID, baseline, native implementation, test/evidence, status (`NOT_RUN`, `PASS`, `FAIL`, `SKIP`), dan alasan. Seluruh baris di bawah merupakan acceptance requirement; SKIP yang terkait kemampuan lingkungan bukan bukti fitur lulus.

| ID | Fitur yang dipertahankan | Native UI / acceptance utama |
| --- | --- | --- |
| P01 | Window resizable, layout, minimum size | Top-level HWND; pertahankan kenyamanan layout sekitar 1200×800 dan minimum baseline 800×600 dengan scaling DPI |
| P02 | Multi-tab: new/close/switch/cycle/restore | Tab control; satu content view dipakai ulang; path/history/sort/filter/selection/focus/scroll terpisah; tutup tab terakhir mengikuti UX baseline atau perubahan yang dicatat |
| P03 | Back, Forward, Up, Refresh, address, breadcrumb | Toolbar + EDIT; native UTF-16 history, navigasi root/UNC benar, errors/loading terlihat |
| P04 | Listing besar dan sorting | Owner-data ListView: Name, Type, Size, Date modified; arah sort, natural order, folder ordering sama; folder 100k tidak dimaterialisasi seluruhnya di UI |
| P05 | Selection dan activation | Single/Ctrl multi-select/Shift range/Ctrl+A; focus dan anchor; Up/Down/Home/End; double-click/Enter membuka directory atau file association |
| P06 | Known folders dan drives | TreeView: Desktop, Downloads, Documents, Pictures, Music, Videos, drives, kapasitas/free space sesuai data tersedia |
| P07 | Favorites | Add current/selected folder, navigate, remove; SQLite compatible; resolusi path selected folder dari token/native name, bukan display_name |
| P08 | Hidden files dan themes | Toggle hidden; System/Light/Dark dapat dipilih, disimpan, diterapkan; system changes dan high contrast; command disabled state sesuai selection |
| P09 | Folder filter | Substring, `*.exe`, `.exe`, `ext:rs`, `type:folder/dir/directory/file`, glob `*`/`?`; match logic dan jumlah hasil sesuai baseline |
| P10 | Indexed search dan Search subfolders | Quoted phrases, ext/type filter, trigram validation, debounce 150 ms, ranked results, total/capped indicator; preserve/jelaskan scope aktual seluruh roots; backend cap 1000, UI baseline meminta 100 hasil halaman pertama |
| P11 | Indexed Roots manager | Add current root, recrawl, remove, state scanning/ready/degraded/offline/needs_reconcile/error; recovery dan watch root tetap berjalan |
| P12 | Search result actions | Open, Open containing folder, Copy path; row menyimpan native path; focus/Enter memakai hasil search aktif, bukan selection directory lama |
| P13 | Create folder dan rename | Dialog native dengan error validation; planner/job pipeline dan collision refusal dipertahankan |
| P14 | Copy, Cut, Paste | Clipboard `CF_HDROP` + Preferred DropEffect, HWND owner valid; interoperabilitas dua arah dengan Windows Explorer |
| P15 | Recycle | Dialog konfirmasi, guarded recycle, unsupported-volume refusal; `Shift+Delete` menolak permanent delete |
| P16 | Internal drag/drop | Selected rows ke folder row atau background destination; default move/Ctrl copy; highlight, token resolution, cycle/descendant refusal; cancel tidak memutasi |
| P17 | Context menus dan Shell integration | Item/background/search menus sesuai baseline; Properties dan Open in Windows Explorer; menu keyboard dapat diakses |
| P18 | Text preview | Pane toggle/close, filename/type/size/modified, UTF-8/UTF-16 LE/BE, line/encoding/truncation metadata, binary/unsupported/error states |
| P19 | Image preview | PNG, JPEG, GIF, WebP, BMP, ICO, SVG; preserve perilaku animasi yang terbukti baseline; native render dan bounded decode; tidak memakai browser fallback |
| P20 | Folder preview | Child count/unknown/reparse handling; batas counting 10k dinyatakan sebagai capped/lower bound, bukan total palsu |
| P21 | Jobs/activity | Show/hide, job kind/state/time, succeeded/failed/canceled/skipped, error, requested/actual destination, detail per item; baseline recent jobs 20/30 dan detail tampilan 100 item |
| P22 | Status bar dan empty/error states | Counts, filtered-from, selected, active path, capped search; loading/empty/denied/stale errors dapat dibedakan |
| P23 | Watcher | Tab subscriptions, unsubscribe, refresh directory aktif/terbuka, debounce, overflow rescan; root-index reconciliation dan restore tetap berjalan |
| P24 | Settings, durable state, recovery | Theme/hidden/restore_tabs/saved_tabs/favorites tetap terbaca; interrupted jobs recovery; database lama tidak dihapus |
| P25 | Single instance dan lifecycle | Lease sebelum DB/executor, trusted focus second launch, startup failure, graceful active-job shutdown tanpa kehilangan journal |
| P26 | Keyboard, accessibility, DPI | Semua shortcut bagian 5, tab order, Narrator, focus cue, contrast, 100/125/150/200% DPI, perpindahan monitor |

Scope clarification: full-text isi file, external OLE drag/drop, cancel/retry UX baru, recursive tree seluruh filesystem, dan shell replacement bukan fitur accepted yang terlihat di baseline. Jangan mengiklankan sebagai selesai tanpa implementasi dan evidence. Clipboard Explorer tetap termasuk parity wajib.

Bug routing keyboard lama tidak perlu dipertahankan: handler Enter directory berada sebelum Alt+Enter dan search Enter; beberapa branch search tidak terjangkau. Native routing harus mengutamakan modifier, modal/edit focus, dan view aktif. Catat perbaikan itu sebagai baseline defect, bukan menghapus shortcut.

### Checklist semua command/event lama

Seluruh entry point di `client.ts` harus punya padanan application/native adapter atau alasan konsolidasi yang disertai coverage. Nama command dapat berubah setelah cutover; perilaku tetap tersedia.

| Area | Command baseline |
| --- | --- |
| Startup/navigation | `bootstrap`, `navigate`, `navigate_native_path`, `list_page`, `refresh` |
| Open/Shell | `open_item`, `open_path`, `show_properties`, `open_in_explorer` |
| Settings/favorites | `load_settings`, `save_settings`, `add_favorite`, `remove_favorite` |
| Plan | `plan_create_folder`, `plan_rename`, `plan_paste`, `plan_recycle`, `plan_transfer`, `commit_plan` |
| Convenience mutations | `create_folder`, `rename_item` — dapat dikonsolidasi ke plan/commit yang sama |
| Clipboard | `clipboard_write_items`, `clipboard_write_path` |
| Preview/activity | `read_preview`, `list_jobs` |
| Watch | `watch_folder`, `unwatch_all`, event `watch-notification` |
| Indexed roots/search | `list_indexed_roots`, `add_indexed_root`, `remove_indexed_root`, `recrawl_indexed_root`, `search_indexed` |

## 5. Shortcut dan routing input

| Input | Aksi |
| --- | --- |
| Alt+Left / Alt+Right / Alt+Up | Back / Forward / Up |
| Ctrl+L / Alt+D | Focus dan select address |
| Ctrl+F | Focus dan select search/filter |
| Ctrl+T / Ctrl+W | New tab / close active tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / Cut / Paste items ketika file view fokus; edit control tetap memiliki text clipboard sendiri |
| Ctrl+A | Select seluruh hasil view aktif; select text jika EDIT fokus |
| Delete / Shift+Delete | Recycle dengan konfirmasi / refusal permanent deletion |
| F2 / Ctrl+Shift+N / F5 | Rename single selected / new folder / refresh |
| Ctrl+P / Alt+P | Toggle preview pane |
| Alt+Enter | Properties item atau current folder sesuai konteks |
| Up / Down / Home / End, dengan Shift | Focus row / range selection |
| Enter / double-click | Open item/hasil search; submit dialog/address sesuai focus |
| Escape | Dismiss modal/menu/activity menurut konteks, lalu selection/focus sesuai baseline |

Jangan membuat file operation accelerator memakan Ctrl+C/X/V/A atau Delete saat pengguna mengedit address/search/nama. Dialog native harus parented, memiliki default/cancel button, dan mengembalikan fokus dengan benar.

## 6. Implementasi UI native dan penghematan memori

### Kontrol dan resource

Gunakan Tab control untuk tabs, TreeView untuk sidebar, Toolbar/buttons untuk navigation/commands, EDIT untuk address/search, report-mode owner-data ListView untuk file/search rows, HMENU untuk context menu, dialog native untuk input/konfirmasi, read-only text control dan image surface untuk preview, serta status bar native.

Embed manifest Common Controls v6, Per-Monitor V2 DPI awareness, icon, dan version resource. Handle resize, `WM_DPICHANGED`, theme/settings changes, font recreation, dan high contrast. Setting DPI melalui manifest sebelum window dibuat mengikuti [panduan Microsoft](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process).

Gunakan documented APIs dan targeted custom draw bila perlu untuk Light/Dark. Tema adalah fitur wajib; keterbatasan dark styling pada kontrol/menu tertentu harus diselesaikan atau dicatat sebagai parity blocker, bukan diam-diam menghapus pilihan Dark. Custom control/drawing tetap harus memberikan nama, role, state, dan keyboard access yang dapat dibaca accessibility tools.

### Virtual list dan filtering

Create ListView dengan `LVS_REPORT | LVS_OWNERDATA`, lalu set logical count melalui `LVM_SETITEMCOUNT`. Sorting dikelola model aplikasi; owner-data tidak memakai `LVM_SORTITEMS` atau `LVS_SORTASCENDING/DESCENDING`. Ini mengikuti [kontrak virtual ListView Microsoft](https://learn.microsoft.com/en-us/windows/win32/controls/list-view-controls-overview).

`LVN_GETDISPINFOW` membaca bounded cache tanpa I/O; miss menampilkan placeholder dan menjadwalkan page request. Tangani `LVN_ODCACHEHINT`, selection/range notifications, activation, column click, dan type-to-select. Pastikan UTF-16 text callback memiliki lifetime valid; copy ke buffer caller dengan batas kapasitas dan NUL termination. Lihat [LVN_GETDISPINFO](https://learn.microsoft.com/en-us/windows/win32/controls/lvn-getdispinfo).

Type-to-select perlu lookup cepat terhadap view lengkap, bukan hanya cached rows. Karena notification memerlukan hasil synchronous, sediakan index lookup yang cepat, atau implementasikan input presenter dengan completion async; jangan menyisipkan await/scan besar ke callback.

Paging saja tidak mempertahankan filter folder jika filtering hanya dilakukan pada empat halaman cache. Tambahkan view mapping di backend: snapshot immutable + sorted/filtered indices + visible count + bounded page API. Pertahankan hidden/filter semantics dan lookup selection pada seluruh hasil. View key mencakup folder token, generation, sort, filter, hidden policy.

Row index bukan identitas file. Selection menyimpan identitas dalam snapshot/view epoch yang tepat dan memakai range/bitmap untuk Ctrl+A besar. Saat mengajukan operasi, materialisasikan token secara bounded di worker dan periksa batas planner; jangan menyalin 100k UUID strings pada UI thread. Token stale tidak boleh diubah menjadi path dari teks tampilan.

### Snapshot dan budget

Setelah native browsing bekerja, optimalkan backend secara terpisah dengan equivalence tests:

1. Ganti cloned `sorted_cache: Vec<FileEntry>` dengan indeks ke entries immutable. Pertahankan comparator/tie semantics dan API lama selama dual host.
2. Kurangi `token -> full PathBuf` menjadi `token -> entry index`; rekonstruksi path dari parent native dan `native_name_utf16` saat resolusi. Identity revalidation sebelum operasi tetap wajib.
3. Tambah snapshot lifetime/pin lease untuk tab aktif dan request/plan yang perlu resolusi. History menyimpan path, bukan snapshot penuh. Inactive tabs dapat melepas display pages dan reload ketika diaktifkan.
4. Tambah accounting untuk entries/maps/strings/capacity, sorted/filtered indices, transient enumeration/sort, queue, bitmap, icon, dan SQLite cache. LRU yang benar mencatat akses; count limit bukan byte limit.
5. Snapshot yang lebih besar dari soft budget tidak boleh diam-diam menghasilkan listing terpotong. Evict cache yang tidak dipin; izinkan satu oversize working snapshot dengan accounting yang jelas atau implementasikan spill/paged backing. Jika hard resource limit tak dapat dipenuhi, tampilkan error, dan catat gagal acceptance dataset tersebut.

Budget awal untuk eksperimen, **bukan angka penggunaan RAM yang sudah dicapai atau hard limit total proses**:

| Komponen | Policy awal |
| --- | --- |
| Display pages | 256 rows/page, maksimum 4 halaman di view aktif, 2 prefetch; soft byte cap 8 MiB |
| Inactive tab UI | Metadata/history/selection; tidak menyimpan seluruh entries atau satu set HWND per tab |
| Retained snapshots | Soft cap 128 MiB dengan explicit pins dan oversize policy |
| Preview | Satu preview aktif; decoded surface awal maksimum 16 MiB, downsample untuk viewport; cap frames/scratch buffers juga |
| Icons | Lazy load; cache owned resources maksimum 16 MiB; hindari thumbnail seluruh folder |
| Completion queue | Awal 128 events / 4 MiB, oversize policy dan reliable mutation results |
| Read/decode/crawl work | Awal maksimum 2 expensive read tasks bersamaan; semaphore/admission; mutation queue mengikuti engine sendiri |

Ubah budget berdasarkan evidence dan dokumentasikan alasan. Byte limits harus memperhitungkan nama/path panjang dan payload job outcomes; count-only limits tidak cukup.

### Preview dan codec parity

Tambahkan binary preview API internal; Base64 tetap hanya pada adapter legacy sampai cutover. Preserve batas text 64 KiB dan file gambar 8 MiB; gunakan checked arithmetic untuk dimensions/stride/frame bytes sebelum alokasi, dan batasi total decoder intermediate memory. Downsample gambar untuk viewport agar format/fitur tidak hilang hanya untuk mengejar budget.

WIC adalah jalur kandidat untuk format raster standar; inventory codec yang tersedia pada clean supported Windows, jangan menganggap optional WebP codec selalu terpasang. Pilih fallback decoder yang dibundel jika perlu. [WIC overview](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-about-windows-imaging-codec) menjelaskan model codec tersebut.

SVG memerlukan renderer tersendiri. Direct2D mendukung subset SVG, sehingga kemampuan render semua SVG yang ditampilkan baseline harus diuji; [dokumentasi Microsoft](https://learn.microsoft.com/en-us/windows/win32/direct2d/svg-support) mencatat fitur yang tidak didukung. Gunakan rasterizer native/Rust tambahan jika diperlukan, tanpa embedded browser atau resource network. Sertakan corpus SVG dengan text, gradients, clipping, transforms dan fitur baseline relevan. GIF animasi perlu frame/timing disposal yang benar jika terbukti berjalan di baseline; jangan menyimpan semua frame decoded tanpa batas.

Gunakan RAII untuk resource owned (bitmap/icon/font/brush/image list/COM objects). Borrowed system image lists dan stock objects tidak boleh dihancurkan. Restore GDI selections sebelum delete resource. Perubahan selection, preview close, tab close, theme/DPI, dan stale completion harus melepas resource lama.

## 7. Invariant keselamatan, data, dan lifecycle

- Semua filesystem behavior tetap Rust-owned; native UI hanya mengirim intent dan menampilkan hasil.
- Preserve `PathBuf`/UTF-16 untuk identitas path, long path, UNC, reparse policy dan clipboard. `display_name`/`path_display` hanya untuk presentasi. Tolak embedded NUL; test unpaired surrogate names. Saved tabs/favorites baseline berupa string: pertahankan kompatibilitas dan tambahkan representasi lossless melalui migrasi schema bila diperlukan, tanpa reset data lama.
- Lease per-user diperoleh **sebelum** membuka DB, recovery, atau membuat executor. Native dan legacy sengaja memakai lease/state directory yang sama dan tidak dijalankan sebagai competing executors. Second launch hanya fokus owner HWND/PID terpercaya lalu keluar.
- Startup gagal jika durable state tidak dapat dibuka dengan benar; tidak ada fallback in-memory journal. Tampilkan error yang dapat ditindaklanjuti dan jangan expose mutation executor.
- Mutations memakai planner -> review/intent acceptance -> commit plan ID -> native identity revalidation -> execution pada STA -> durable item outcomes -> reconcile. Preserve input/confirmation dialogs baseline; jangan bypass planner lewat direct `std::fs::copy`, `rename`, atau deletion.
- Plan expiry, token ownership dan commit-once behavior harus dipertahankan. Session/window binding merupakan gap baseline yang perlu ditangani pada extracted API: plan terikat live host/session, bukan sembarang ID dari UI lama/tertutup. Catat sebagai remediation yang terkait lifetime migrasi.
- Preserve HRESULT classification, protected/tree/reparse/placeholder policy, source-parent identity, collision refusal, dan requested/actual destination outcomes. Baseline folder rename mempunyai Shell provider adapter yang didokumentasikan; jangan menggantinya tanpa equivalence evidence.
- Recycle selalu fail-closed. Volume unsupported/UNC tidak boleh beralih ke permanent deletion. Jangan menawarkan permanent delete sebagai fallback. `Shift+Delete` tetap menolak.
- Clipboard menggunakan owner HWND utama yang tervalidasi dan format Windows Shell. Clipboard tests harus serial dan memiliki isolasi/safe restoration seluruh format relevan; jangan menghilangkan clipboard pengguna untuk menjalankan test.
- Preserve database paths dan data settings/favorites/journal/index. Backward-compatible schema selama dual host, transactional upgrade jika diperlukan, dan install/uninstall tidak menghapus data tanpa pilihan eksplisit.
- Shutdown: hentikan admission mutasi baru, tetap pump messages, tampilkan state menunggu job, drain/persist hasil, stop subscriptions/tasks/workers, tutup DB/runtime, lalu release lease. Jangan join worker blocking di `WM_CLOSE`. Jangan menampilkan Cancel/Retry aktif jika backend belum mendukung semantik itu.
- Mutation tests hanya pada marked fixture roots `.rust-explorer-fixture-root`; marker berada di parent fixture, di luar directory/index dataset yang dihitung. Jangan memakai personal folders sebagai mutation fixtures.

Inherited acceptance gaps: handoff lokal melaporkan successful recycle sebagai capability skip karena `SHQueryRecycleBinW` menghasilkan access denied pada volume fixture; clipboard round-trip belum memiliki complete safe restoration; real desktop focus/active-job close, clean-machine installer dan physical performance belum accepted. Reproduksi kondisi saat ini, pertahankan guards, dan laporkan `SKIP`/`NOT_RUN` secara jujur. Fitur yang terlihat di kode bukan bukti end-to-end accepted.

## 8. Tahapan implementasi dan exit criteria

### M0 — Inventory, baseline, dan rencana yang dapat direview

Verifikasi repo/working tree, baca sources, buat parity ledger dan native progress log. Reuse branch/worktree yang cocok jika sudah ada; jika perlu branch baru gunakan `codex/native-win32-ui` kecuali ada instruksi pengguna yang berbeda. Jangan reset/overwrite pekerjaan lain. Build release legacy dan rekam process-tree baseline sebelum menghapus host. Capture baseline journeys/screenshots pada fixture.

Exit: baseline identitas repo dan behavior tercatat; setiap fitur mempunyai acceptance scenario; memory baseline terukur atau diberi blocker spesifik. Baseline measurement blocked tidak mencegah extraction, tetapi mencegah klaim penghematan.

### M1 — Ekstrak application layer tanpa regresi legacy

Pindahkan durable state, command orchestration dan watch/index lifecycle ke `explorer-app`. Ganti Tauri state/runtime/event coupling dengan typed API dan host-owned runtime/dispatcher. Legacy commands menjadi wrappers, dengan kontrak IPC tetap valid. Preserve startup tests, HWND owner authorization, lease ordering dan root reconciliation.

Exit: legacy build/targeted tests tetap berjalan; application crate tidak membawa Tauri; shutdown/task ownership dan error mapping jelas.

### M2 — Native shell dan lifecycle dasar

Tambahkan package native, manifest/resources, window, control layout, message loop, command routing, dispatcher, instance lease/HWND publication, runtime ownership dan graceful close. Jangan expose mutasi sebelum durable initialization selesai.

Exit: real HWND opens/resizes/scales; startup failure aman; second launch/focus behavior dan shutdown diperiksa; window-only prototype belum dianggap migrasi selesai.

### M3 — Browsing lengkap

Implementasikan owner-data listing, bounded pages, sorting, filtering/hidden view mapping, address/breadcrumb, history, selection/activation, status/error states, stale-result rejection dan tabs. Prioritaskan folder kosong, folder kecil, physical folder 100k, root/UNC/denied paths dan rapid navigate/sort/close races.

Exit: P01–P05/P09/P22 sesuai slice tercakup, responsive scroll dan selection benar; UI tidak fetch semua pages sekaligus.

### M4 — Daily feature parity

Implementasikan sidebar/favorites/themes/settings, indexed search/root manager, watcher lifecycle, dialogs/file operations, clipboard, drag/drop, Shell integration, activity dan preview semua format. Gunakan native active-view keyboard routing. Pertahankan baseline accepted behavior, perbaiki gap yang menghambat native parity dengan test terpisah.

Exit: P01–P26 mempunyai implementasi dan evidence, atau blocker konkret yang mencegah acceptance; tidak ada fitur yang dihapus hanya karena sulit dipindahkan.

### M5 — Memory optimization dan performance evidence

Refactor sorted indices/token map/lifetime accounting dan preview payload secara bertahap. Uji equivalence sebelum/ sesudah; lakukan release measurements dengan fitur dan dataset yang sama. Perbaiki cache leaks, handles, queue pressure dan regressions. Simpan raw data, bukan hanya screenshot Task Manager.

Exit: target bagian 9 terpenuhi dengan bukti, tanpa correctness/safety regression; stable retained objects/cache/handle counts pada repeated journeys.

### M6 — Native packaging, CI, dan cutover final

Tambahkan native checks/doctor dan portable packaging terlebih dahulu, lalu per-user NSIS/MSI dengan tooling installer yang dipilih/terverifikasi. Tauri bundle script tidak otomatis dapat memaketkan executable baru. Preserve version consistency, icon/manifest, checksums UTF-8, metadata SHA/size/toolchain, optional signing dan upgrade/uninstall behavior.

Native final CI tidak membutuhkan npm/WebView2. Pertahankan Rust audit dan native checks; legacy npm checks hanya di fase dual host. Pisahkan verification dari publication; manual/branch validation tidak otomatis mengirim release atau memindahkan tag.

Setelah parity, safety, memory dan clean-machine gates lolos, hapus host Tauri dan React, npm workspaces/lockfiles yang tidak dipakai, Tauri config/capabilities/schemas/build dependencies, WebView2 prerequisites, dan legacy script assumptions. Update `Cargo.lock`, README/ADR, doctor/version checks, release commands dan native executable product name. Jangan uninstall WebView2 yang terpasang pada komputer pengguna; hanya hilangkan kebutuhan runtime aplikasi.

Exit: default release/installer memakai native host, portable dan installer berjalan pada clean supported Windows tanpa WebView2, dependency graph bersih, build/CI/documentation akurat.

## 9. Protokol pengukuran RAM dan gate performa

Implementasikan harness reproducible, misalnya `scripts/measure-memory.ps1`:

1. Build legacy dan native **release**, catat commit/backend baseline, compiler/lockfile, Windows build, CPU/RAM, window size/DPI, fixture, settings, roots/index state, preview dan timestamps. Gunakan sesi berurutan karena lease bersama. Pisahkan cold/warm start dan indexing/recovery startup dari steady idle.
2. Track main PID beserta descendants selama run, termasuk renderer/browser/GPU/utility WebView2 yang benar-benar milik aplikasi. Pakai PID + creation time; validasi attribution dengan process command lines/data directory. Jangan menghitung semua Edge/WebView2 komputer, dan jangan menghitung file association/Shell apps yang sengaja dibuka sebagai host UI.
3. Sample 250–500 ms, minimal lima run per scenario. Catat per-process/aggregate private bytes, working set, peak, process count, native GDI/USER handles, cache bytes/pages, preview bytes dan queue depth. Simpan raw CSV/JSON serta median/range summary.
4. Private commit dan resident working set merupakan metrik berbeda. `PrivateUsage` pada [PROCESS_MEMORY_COUNTERS_EX](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex) mengukur private commit; `WorkingSet64` mengukur residency. Penjumlahan working set dapat menghitung shared pages berulang. Laporkan label/scope metrik dengan benar; jangan menyebut private bytes sebagai jumlah exact physical RAM.
5. Scenarios: small-folder idle; physical folder 100k browse/sort/scroll/filter/select-all; 10 open tabs; 100 navigate/open-close-tab cycles; indexed search/root crawl; text dan seluruh format image previews; repeated image/theme/DPI changes; watcher burst/overflow; marked-fixture copy/move/recycle sesuai capability; restart recovery dan active-job close.
6. Sesudah churn gunakan idle interval tetap, misalnya 30 detik. Pastikan retention plateau dan resource accounting stabil. Jangan melakukan forced working-set trimming agar angka terlihat kecil. Synthetic FS/index stress tests tidak menggantikan physical UI journeys.
7. Tulis evidence ke `artifacts/native-ui/<run-id>/`; generated artifacts di-ignore saat ini, jadi attach/salin bukti dengan sadar jika harus tersedia bagi reviewer. Summary harus dihitung dari captured data, tanpa angka hardcoded.

Gate awal berikut adalah target implementasi yang dapat dikalibrasi dari baseline dengan alasan tercatat; bukan janji hasil yang sudah terbukti:

| Gate | Kriteria awal |
| --- | --- |
| Browser independence | Native release tidak memerlukan/menjalankan Tauri/WebView2/browser UI processes |
| Idle private commit | Median aggregate native private bytes setidaknya 30% lebih rendah daripada aggregate legacy dengan fitur/settings sama |
| Idle residency | Target investigasi native working set ≤100 MiB; laporkan actual value dan scope, bukan klaim dari main PID legacy |
| Folder 100k / previews / tabs | Peak dan settled memory membaik terhadap baseline; complete result set, selection, preview tetap benar |
| Responsiveness | Tidak ada I/O berat di UI callback; initial p95 common input-to-feedback ≤100 ms, pisahkan loading selesai dari feedback; smooth large-folder scroll diverifikasi |
| Retention | Cache/queue bytes terikat; tidak ada pertumbuhan monotonic retained resources atau GDI/USER handles dalam churn runs |
| Correctness | Sort/filter/native paths, clipboard, plans, outcomes, failure/refusal dan recovery sama atau perbaikan baseline terdokumentasi |

Rumus penghematan: `(median_legacy_total_private - median_native_total_private) / median_legacy_total_private * 100%`. Sertakan kedua nilai input, process attribution dan scenario. Binary size atau SQLite disk footprint tidak menggantikan metrik RAM.

## 10. Verifikasi implementasi

Handover ini tidak menjalankan build, tests, UI journeys atau benchmarks aplikasi; tidak ada hasil PASS baru yang diklaim. Agent implementasi menjalankan targeted checks lalu milestone gates dan merekam output aktual.

Contoh commands pada fase dual host; nama package baru valid setelah M1/M2 dibuat:

```powershell
git status --short
git branch --show-current
git rev-parse HEAD
cargo check -p explorer-app -p rust-explorer --locked
cargo check -p rust-explorer-native --locked
cargo test -p explorer-app --locked -- --test-threads=1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1 --skip test_clipboard_roundtrip
npm run typecheck
npm run test:unit
npm run build:ui
cargo xtask doctor
cargo build -p rust-explorer-native --release --locked
cargo tree -p rust-explorer-native
```

`test_clipboard_roundtrip` dikecualikan pada baseline lokal karena safe restoration belum tersedia; skip ini eksplisit dan sementara. Setelah harness isolasi/restoration tersedia, jalankan interop tests dan catat hasil. Jangan menganggap skipped clipboard journey telah lulus. Inspeksi safety fixtures sebelum menjalankan tests; `scripts/check.ps1` sekarang menjalankan workspace tests tanpa filter/serialization tersebut, sehingga perlu disesuaikan untuk native validation yang aman.

Dependency baru dapat memerlukan lockfile update yang disengaja; review hasilnya lalu kembali ke `--locked`. Gunakan `--offline` hanya jika semua dependency sudah tersedia. Setelah cutover, npm/legacy doctor diganti native checks; jangan mempertahankan gate yang selalu gagal atau menonaktifkan test untuk mengejar kelulusan.

Tambahkan test bermakna untuk:

- Stale navigation/page/preview completions, closed-tab results, wakeup races, queue saturation/shutdown.
- Sorted/filtered index equivalence, hidden count, snapshot leases/release/oversize policy, UTF-16 token resolution, large selection.
- Plan lifetime/owner/commit-once/revalidation, preserved collisions/refusals/outcomes dan interrupted-job recovery.
- Decoder bounds, malformed/oversize images, missing codec fallback, SVG/GIF parity, stale preview/resource release.
- Real keyboard/menu/edit focus journeys, screen reader/high contrast, multi-DPI/monitor, watch overflow, second launch dan active-job shutdown.
- Clean-machine portable/installer, version/manifest/icon, dependency/process inspection, upgrade/uninstall/data retention dan checksum/signing metadata.

Screenshots dapat membantu review layout; process `Responding = true` bukan bukti keyboard, scroll, mutation, accessibility, atau graceful shutdown berhasil.

## 11. Deliverables dan definition of done

- [ ] `explorer-app` host-independent dan semua lifecycle orchestration terport.
- [ ] UI/default executable Win32 native dengan P01–P26 dan command/shortcut coverage.
- [ ] Backend safety/UTF-16/durable-state/instance/clipboard invariants tetap berlaku.
- [ ] Seluruh image format baseline dipertahankan; theme, DPI, focus dan accessibility tercakup.
- [ ] Paging/filtering/selection tidak menggandakan seluruh dataset ke UI; cache/preview/queues/lifetimes di-account.
- [ ] Measured release comparison memenuhi gate RAM/performance dengan raw evidence.
- [ ] Native checks/CI/doctor/portable/NSIS/MSI, metadata dan clean-machine acceptance tersedia.
- [ ] Tauri/React/WebView2 dan tooling browser yang tidak dipakai dihapus pada cutover final.
- [ ] README/build instructions, parity ledger, progress log, known gaps dan final handoff akurat dan tersedia bagi checkout penerima.

Pisahkan status migrasi UI dari inherited MVP acceptance gaps. Jangan memberi status complete tanpa evidence seluruh fitur; capability skip yang belum dapat diuji tetap dinyatakan terbuka dengan kebutuhan lingkungan yang spesifik.

Pada akhir milestone update `NATIVE_WIN32_PROGRESS.md`: branch/HEAD, changed files, commands dan hasil, fixture/evidence paths, PASS/FAIL/SKIP/NOT_RUN, blocker, dan next concrete step. File progress harus membantu agent berikutnya melanjutkan tanpa audit ulang dari nol.

## 12. Prompt siap diberikan ke agent implementasi

```text
Implementasikan migrasi Rust Explorer ke UI Win32 native sesuai
NATIVE_WIN32_HANDOVER.md di root repository. Baca dokumen itu terlebih dahulu,
verifikasi checkout/working tree dan sumber kode aktual, lalu kerjakan M0–M6.
Pertahankan seluruh fitur P01–P26, shortcut/command parity, UTF-16 paths,
dedicated STA, validated plans/journal, guarded recycle, clipboard owner dan
single-instance lifecycle. Ekstrak explorer-app; buat apps/native-win32 dengan
real Common Controls dan bounded data/resource lifetime. Keep legacy host
working selama transisi; cutover setelah parity, safety, measured RAM improvement
dan packaging acceptance selesai. Jangan berhenti pada prototype window atau
menonaktifkan fitur demi RAM. Buat NATIVE_WIN32_PARITY.md dan
NATIVE_WIN32_PROGRESS.md, jalankan targeted checks dan milestone gates, serta
simpan raw release measurements untuk seluruh process tree. Bedakan baseline
defects/capability skips dari migration regressions; jangan mengarang PASS.
Jangan reset pekerjaan pengguna, publish externally, mengubah global Windows
Shell, atau melakukan mutation tests pada personal folders. Jika ada blocker,
selesaikan pekerjaan independen yang masih dapat dilakukan dan tinggalkan
handoff konkret dengan state/evidence/next steps. Lokasi branch/worktree pada
docs lama harus diverifikasi; jangan menganggapnya masih tersedia.
```
