# Review keselarasan objective dan implementasi

Tanggal: 4 Oktober 2026 (Asia/Jakarta). Baseline audit: `fa934f0309a0e6930d163ade21d800b1fa2d5a4d`.
Working tree bersih saat audit baseline dimulai. Temuan R1–R12 di bawah menggambarkan keadaan pada baseline tersebut; bagian **Remediasi setelah audit** mencatat perubahan kode dan status terkini.

## Kesimpulan

**MVP incomplete menurut spesifikasi bagian 26.2.** Arsitektur dan cakupan fitur dasar cukup selaras: workspace Rust, Tauri 2, React, enumerasi Win32, virtualisasi, operasi `IFileOperation`, jurnal SQLite, watcher, dan FTS5 benar-benar ada. Ini sudah melampaui scaffold. Namun tahap yang dapat dipertanggungjawabkan adalah **implementasi fitur dasar lintas M1–M6 dengan packaging awal; acceptance, keselamatan mutasi, hardening, dan release validation belum selesai**.

Klaim M0–M8 “100% complete” pada [handoff](D:/dev/rust-explorer/docs/agent/handoff.md:80), backlog, dan laporan DoD tidak didukung oleh beberapa jalur produksi serta kualitas bukti yang tersedia. Milestone bersifat dependency-ordered: hadirnya installer tidak menutup gap pada M1/M3/M4/M5/M6. Persentase completion tidak diberikan karena checklist saat ini belum memisahkan implementasi, pengujian, dan acceptance.

## Remediasi setelah audit

Perubahan berikut diterapkan setelah snapshot audit. Temuan lama tetap dipertahankan sebagai catatan baseline; status baris ini adalah status kode saat ini. Perubahan belum dianggap lulus acceptance native hanya karena berhasil dikompilasi.

| Temuan | Status kode saat ini |
|---|---|
| R1 — Otoritas plan/IPC | **Sebagian ditutup.** Commit menerima `PlanId` dan mengambil plan yang disimpan Rust; hasil commit plan yang sama disimpan selama lima menit setelah eksekusi. Paths, identities, dan commit token tidak diserialisasi ke renderer. Recycle/copy/cut memakai item token; tujuan memakai folder token. Paste membaca CF_HDROP langsung di Rust; aksi “Copy path” menulis teks CF_UNICODETEXT, bukan daftar file. Session/window binding dan acceptance keamanan tetap terbuka. |
| R2 — Path Windows lossless | **Perbaikan jalur utama diterapkan.** Enumerator menyimpan nama UTF-16 native; snapshot, crawler, index, clipboard, search/navigation history, copy-path, root index, dan watch notifications mempertahankan unit native. Label tampilan tetap lossy secara sengaja; operasi tidak merekonstruksi path dari label. Pipeline masih memerlukan marked fixture untuk membuktikan surrogate edge cases. |
| R3 — Identity/preflight | **Blocker implementasi ditutup.** Policy Rust melindungi app/state/drive/share/fixture boundaries, mengurangi selection ancestor-child tanpa menggabungkan hard links, memeriksa seluruh source tree dan existing output tree untuk reparse/placeholder, serta memvalidasi identity sumber dan parent. Pemeriksaan diulang sebelum journal dan di STA sebelum submission. Preflight bounded/cancelable; progress/cancel UI untuk tree besar dan acceptance edge cases tetap terbuka. |
| R4 — Partial outcomes/cancellation | **Blocker correctness ditutup.** USER_IGNORED → Skipped; USER_CANCELLED → Canceled; native failures tidak lagi disamakan dengan user cancel. Source native ditangkap sebelum mutasi, hasil dipasangkan berdasarkan path dan digabung per selected root; kegagalan child tidak ditimpa success parent. Actual Shell destination/native HRESULT dipersist dan ditampilkan. Semua operation kinds mengirim outcome ke writer journal. App cancellation/live progress/retry dan native partial/cut acceptance masih terbuka. |
| R5 — Crawl offline/parsial | **Perbaikan implementasi diterapkan.** Crawl melewatkan prune untuk parent yang gagal dienumerasi, melakukan scoped prune hanya pada parent sukses, dan menandai hasil offline/parsial secara sesuai. Root path juga tetap memakai unit native. Belum dijalankan pada fixture acceptance setelah perubahan. |
| R6 — Watch overflow/freshness | **Sebagian ditutup.** Overflow channel ditangkap melalui sinyal atomik; coalescer memakai satu penanda full-reconcile agar batas dirty-directory tidak tumbuh tanpa batas; lag receiver Tauri memicu refresh tab dan indexed-root recrawl. Indexed roots kini mendaftarkan recursive watch sebelum crawl baseline, dipulihkan pada startup, dan perubahan/overflow memicu recrawl per-root yang didebounce; watch startup gagal menandai root Offline/NeedsReconcile. Polling/resume saat volume kembali online, freshness completion, dan convergence end-to-end masih perlu ditutup/dibuktikan. |
| R7 — Clipboard | **NULL-owner blocker ditutup.** HWND diambil dari originating main window oleh host, divalidasi live/owned-by-process di Rust, lalu dipakai kedua writer. Null owner ditolak sebelum clipboard dibuka. Clipboard round-trip dan OLE completion/partial-cut interoperability tetap belum accepted; clipboard pengguna tidak diubah dalam validasi ini. |
| R8 — Single instance/queue/STA | **Blocker executor ganda ditutup.** Lease file per-user state dir diambil sebelum DB/recovery/executor; launch kedua hanya mencoba focus HWND yang dipublikasi proses pemilik dan keluar. Lease dipertahankan oleh executor sampai drain; STA memiliki bounded queue, COM readiness handshake, message pump, dan join pada shutdown. Exclusion/relaunch lintas proses terbukti di fixture; actual second-launch focus dan graceful-close UX belum accepted. |
| R9 — Large listing | **Sebagian ditutup.** Backend membatasi page ke 256 item; UI mempublikasikan hasil secara bertahap dan mengabaikan respons navigation/listing stale. Enumerasi backend tetap eager, seluruh hasil akhirnya disimpan di UI, dan benchmark physical 100k belum ada. |
| R10 — Interaction/DTO/settings | **Sebagian ditutup.** Balasan stale ditolak, nama tidak lagi di-trim sebelum planner, validasi panjang memakai UTF-16, hasil search dinavigasi melalui native path, Shift+Delete menjelaskan bahwa permanent delete tidak tersedia, preferensi hidden files kini memiliki toggle tersimpan yang memfilter listing, dan job drawer menampilkan outcome per item. Row listing memakai opsi multi-select semantik. Keyboard/accessibility acceptance, widths/schema/large-number DTO, relative path basis, dan acceptance interaksi tetap terbuka. |
| R11 — Native/installer E2E | **Terbuka.** Tidak ada native E2E, installer journey, upgrade/uninstall, atau clean-machine check yang dijalankan dalam remediasi. |
| R12 — Performance/release evidence | **Terbuka.** Tidak ada benchmark physical atau pengukuran release baru yang dijalankan. |
| R13 — Durable journal startup | **Blocker implementasi ditutup.** Startup fallible tanpa in-memory fallback untuk state/index; absolute LOCALAPPDATA wajib; journal memakai WAL/FULL/busy timeout dan recovery error dipropagasikan. Startup failure mendapat native error message sebelum IPC mutasi tersedia. Corrupt/unavailable disk state dan durable reopen/recovery diuji pada fixture. |

Validasi remediasi awal sebelum penutupan blocker (snapshot historis, 4 Oktober 2026): `cargo fmt --all`, `cargo check --workspace --locked --offline`, `npm run typecheck`, dan `git diff --check` berhasil. Tidak ada test suite, mutation harness, clipboard interaction, app instance, installer, atau benchmark yang dijalankan.

## Penutupan lima blocker — 4 Oktober 2026

**R3, R4 correctness, R7 NULL owner, R8 multi-instance, dan R13 fallback journal telah ditutup pada jalur produksi.** Pengembangan dan integrasi pada capability yang telah diuji dapat dilanjutkan. Acceptance MVP/release bagian 26.2 tetap OPEN; hasil ini bukan klaim seluruh requirements sudah accepted.

- Preflight metadata-only menolak links/reparse di entry, ancestor, source tree dan existing output tree; flags OFFLINE/RECALL ditolak tanpa membuka konten. Queue 16,384, batas 250,000 entries dan 10 detik; cancel/timeout berarti tidak tervalidasi, bukan safe. Alias ancestry memakai canonical entry spelling dan native directory IDs; distinct hard-link entries dipertahankan. Native identity handle menggunakan OPEN_REPARSE_POINT. Sources dan parents dicek ulang tepat sebelum native work.
- Callback tidak lagi dilabeli berdasarkan urutan. Native paths disimpan sebelum source move/delete, child failures digabung ke selected root, actual collision output dan HRESULT disimpan. Callback tak terikat ditampilkan sebagai unknown/skipped, tanpa mengarang success; native failures sebelum callback juga membawa kode asli. JobSummary terminal dikembalikan untuk semua status, dan UI membuka job drawer untuk hasil non-success.
- Clipboard owner berasal dari authorized main window, bukan renderer-provided HWND. Kedua writer memakai owner non-null; invalid owner gagal sebelum EmptyClipboard.
- Instance lease berbasis file sharing pada state dir per-user, bukan mutex machine-global. Launch kedua tidak membuka DB/recover jobs/create executor. Focus hanya memakai HWND/PID yang dipublikasi owner; tidak ada navigation/mutation intent dari argv yang diteruskan. Guard hidup bersama executor, dan STA join/drain mendahului pelepasannya.
- Durable startup/recovery gagal secara terlihat dan tidak mengekspos executor. Tidak ada fallback in-memory di host. Journal WAL + synchronous FULL dan recovery/reopen telah diuji.

Native fixture acceptance menemukan dua kondisi host: IFileOperation RenameItem untuk directory gagal dengan 0x80070002 sebelum callback, sedangkan Shell folder ITransferSource::RenameItem berhasil. Adapter memilih **Shell transfer provider, TSF_NORMAL/no overwrite, untuk directory rename**, memakai HRESULT dan IShellItem output yang benar (tidak mengarang callback). File rename, create/copy/move tetap IFileOperation. Ini perbedaan terarah dari urutan literal spec 13.2; backend tetap native Shell pada dedicated STA, tanpa fallback ke byte-transfer/Win32 rename engine. Directory rename success dan collision preservation lulus. Referensi: [ITransferSource::RenameItem](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itransfersource-renameitem).

Recycle support query pada volume fixture host mengembalikan **0x80070005 (Access denied)**. Adapter menolak sebelum DeleteItem/PerformOperations; source terbukti tetap utuh dan error tampil di outcome. Successful recycle adalah **CAPABILITY SKIP**, bukan PASS. Pada percobaan awal Shell menampilkan tawaran permanent deletion; tawaran ditolak, tidak ada persetujuan permanent deletion. Guard PreDelete tetap menolak transfer tanpa recycle flag. Jangan mengklaim guarantee recycle untuk filesystem/host lain sebelum fixture acceptance di sana.

Validasi: workspace tests serial **51 passed, 1 clipboard round-trip filtered**, lalu final targeted run **37 passed** setelah penambahan dua regression tests/final safety changes; format, Clippy semua target -D warnings, doctor, UI typecheck, **8 UI tests**, dan UI production build lulus. Targeted suite memverifikasi native create/file-folder rename/copy/move, folder-collision no-overwrite, junction refusal, hard-link preservation, parent swap, out-of-order outcomes, disk journal reopen/corruption, dan cross-process lease/relaunch. Logs: `artifacts/reviews/gap-closure-2026-10-04/`. Test roots bertanda; tidak ada desktop app instance, installer, clipboard writes/reads, personal-file mutation, global shell-setting change, atau physical release benchmark. Cloud tests memakai attribute cases; real provider hydration acceptance belum dilakukan. Clipboard round-trip sengaja belum dijalankan karena belum ada harness pemulihan seluruh format clipboard pengguna.

Masih terbuka: session/window-bound plan acceptance, live progress/cancel/retry, clipboard/OLE/Explorer interop, UI focus/close journey, network/cloud/long-path full pipeline, indexed-root resume, physical performance, installer dan release E2E. Batas race path-based Shell tetap berlaku sesuai spec 7.2; tidak ada klaim filesystem sandbox/atomic plan.

## Review ulang sebelum penutupan blocker — snapshot historis 4 Oktober 2026

Scope: kode working tree di atas HEAD `fa934f0309a0e6930d163ade21d800b1fa2d5a4d`, termasuk seluruh perubahan remediasi yang belum di-commit. Tidak ada perubahan kode produksi pada review ulang ini. Keputusan sementara: **boleh melanjutkan pengembangan UI/browsing/search; belum lolos untuk acceptance mutasi file atau release MVP**. Ada blocker implementasi yang penting, sehingga permintaan meloloskan sementara bila tidak ada masalah penting belum terpenuhi.

| Prioritas / temuan | Bukti pada kode aktual dan dampak |
|---|---|
| **P1 — R3: preflight keselamatan mutasi belum lengkap** | [planner.rs:204](D:/dev/rust-explorer/crates/explorer-jobs/src/planner.rs:204) dan [planner.rs:286](D:/dev/rust-explorer/crates/explorer-jobs/src/planner.rs:286) hanya memeriksa ancestry secara lexical. Tidak ada traversal preflight untuk reparse/placeholder atau perlindungan app/state directory; [policy.rs](D:/dev/rust-explorer/crates/explorer-fs/src/policy.rs) masih placeholder. [identity.rs:31](D:/dev/rust-explorer/crates/explorer-win/src/identity.rs:31) tidak membuka link dengan `FILE_FLAG_OPEN_REPARSE_POINT`; untuk symbolic link, [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew) mengembalikan handle target tanpa flag itu. Identitas yang dibaca belum membuktikan identitas link entry. Tree/link yang menurut spec 7.4 harus ditolak masih dapat dipasok ke Shell. Ini temuan inspeksi, tanpa eksperimen mutasi. |
| **P1 — R4: skipped/canceled salah diklasifikasikan** | [sink.rs:43](D:/dev/rust-explorer/crates/explorer-win/src/sink.rs:43) mengubah semua HRESULT nonnegatif menjadi `Succeeded`, semua negatif menjadi `Failed`. Probe pada sink produksi menghasilkan `user_ignored_callback_status=Succeeded; completed=1` dan `user_canceled_callback_status=Failed; failed=1`. Windows mendokumentasikan `COPYENGINE_S_USER_IGNORED` sebagai jawaban No dan `COPYENGINE_E_USER_CANCELLED` sebagai pembatalan ([Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itransfersource-openitem)). Ringkasan job belum dapat diandalkan pada collision/skip/cancel. |
| **P1 — R7: clipboard writer kehilangan owner** | [clipboard.rs:148](D:/dev/rust-explorer/crates/explorer-win/src/clipboard.rs:148) dan [clipboard.rs:243](D:/dev/rust-explorer/crates/explorer-win/src/clipboard.rs:243) memakai HWND `NULL`, lalu mengosongkan dan menulis clipboard. [Kontrak OpenClipboard](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard) menyatakan pola tersebut menjadikan owner `NULL` dan menyebabkan `SetClipboardData` gagal. Copy/Cut/Copy Path memerlukan owner window yang valid. Risiko juga mencakup clipboard lama sudah kosong ketika write gagal. Ini verifikasi kode terhadap kontrak API; tidak diuji dengan clipboard pengguna. |
| **P1 — R8: dua instance dapat memiliki executor terhadap state yang sama** | [lib.rs:60](D:/dev/rust-explorer/apps/desktop/src-tauri/src/lib.rs:60) selalu membuat `AppState`; tidak ada mutex/single-instance guard atau forwarding sebelum inisialisasi. [state.rs:37](D:/dev/rust-explorer/apps/desktop/src-tauri/src/state.rs:37) langsung menjalankan interrupted recovery. Launch kedua bisa menandai job aktif instance pertama sebagai Interrupted sekaligus membentuk executor kedua. Serialisasi commit hanya berlaku di dalam satu proses. Tidak meluncurkan instance kedua untuk membuktikannya karena kontrak repo melarang competing mutation instances. |
| **P1 — R13: mutasi tetap aktif tanpa durable journal** | [state.rs:33](D:/dev/rust-explorer/apps/desktop/src-tauri/src/state.rs:33) melakukan fallback `JobJournal::open_in_memory()` saat DB disk gagal dibuka, kemudian tetap membuat operation service. Job dan outcome hilang setelah proses berakhir; UI tidak diberi status storage degraded. [state.rs:37](D:/dev/rust-explorer/apps/desktop/src-tauri/src/state.rs:37) juga membuang error recovery. Harus fail closed untuk mutasi saat journal durable/recovery tidak tersedia, dengan error yang dapat dilihat pengguna. |

R4 juga belum mengikat callback ke sumber: `PostCopyItem`/`PostMoveItem` mengabaikan `psiItem` dan returned destination item, sedangkan [executor.rs:318](D:/dev/rust-explorer/crates/explorer-jobs/src/executor.rs:318) dan [executor.rs:400](D:/dev/rust-explorer/crates/explorer-jobs/src/executor.rs:400) memberi label sumber menurut ordinal callback. Kesesuaian ordinal terhadap sumber/tree tidak dibuktikan; review ini tidak mengklaim telah mereproduksi callback reorder pada transfer native. Spec 13.2 meminta identitas sumber dan tujuan aktual, termasuk hasil rename saat collision.

Perbaikan yang dikonfirmasi probe: unpaired surrogate tetap ada setelah extended prefix; `report ` ditolak; outcome individual yang sudah commit muncul kembali dengan state Interrupted; crawl root offline mempertahankan 1 entri dan menandai Offline. Ini menutup defect helper baseline yang diprobe, bukan acceptance seluruh pipeline native.

Yang dapat ditunda selama pengembangan: penyempurnaan columns/keyboard/accessibility, schema dan DTO precision, relative address basis, enumerasi streaming/physical 100k, polling/resume indeks, serta installer/performance evidence. Semua tetap dicatat sebagai belum accepted; native E2E dan release gates tidak boleh diberi PASS tanpa bukti. Crash window antara perubahan filesystem dan journal write memang diakui spec 12.2; keberadaan window itu sendiri bukan bukti kegagalan seluruh desain journal.

Validasi ulang: format check, workspace Clippy seluruh target dengan `-D warnings`, TypeScript typecheck, dan **12 tests nonmutasi** (domain/store, path, name, sort, query) lulus. Probe memakai production libraries, interface sink lokal dengan HRESULT yang diinjeksi, dan SQLite in-memory. Source: [probe.rs](D:/dev/rust-explorer/artifacts/reviews/objective-recheck-2026-10-04/probe.rs); output: [probe-output.txt](D:/dev/rust-explorer/artifacts/reviews/objective-recheck-2026-10-04/probe-output.txt); command: `./artifacts/reviews/objective-recheck-2026-10-04/run-probe.ps1`. Tidak menjalankan transfer Shell, recycle, clipboard, desktop app, installer, atau benchmark physical. Dua invocation standalone awal gagal karena pemilihan `.rlib`/feature graph; diperbaiki dengan satu build grup dependency, lalu probe compile/run berhasil. Ini kegagalan setup probe, bukan workspace compile failure.

## Pemetaan milestone

| Milestone | Implementasi yang ditemukan | Gap acceptance utama | Penilaian |
|---|---|---|---|
| M0 | Workspace, lockfiles, exact Rust toolchain, build/check scripts, native host | ESLint/Playwright tidak tersedia; command contract belum lengkap; versi langsung npm masih ranges dan environment belum merekam resolusi lengkap/MSRV | Fondasi tersedia; gate perlu dibuka kembali |
| M1 | Known folders/drives, Win32 enumeration, snapshot, sorting, navigation, virtual list | Path native menjadi lossy; listing tidak streaming/cancelable; UI hanya memuat 5.000 entri; relatif belum berbasis tab | Sebagian |
| M2 | Tabs/history, multi-selection, keyboard, favorites/theme, open/properties, restore path tab | Stale reply belum ditolak; hidden toggle/column widths belum terhubung; row accessibility dan interaction/DPI acceptance belum terbukti | Sebagian |
| M3 | Create/rename via STA, plan expiry, commit-token set, SQLite journal, interrupted recovery | Plan dapat dikirim/diubah oleh IPC caller; tanpa identity revalidation; tanpa bounded job queue dan message pump; single-instance belum ada | Sebagian; gap correctness |
| M4 | Native copy/move/recycle; recycle guard; CF_HDROP/drop effect | Tanpa app cancellation/per-item results; partial success hilang; tree policy belum preflight; clipboard parsing belum aman; interoperabilitas Explorer belum dibuktikan | Sebagian; gap keselamatan |
| M5 | Notify adapter, debounce, refcounts, visible-folder refresh, indexed-root watch/recrawl | Polling/resume dan bukti convergence end-to-end belum tersedia | Sebagian |
| M6 | Root opt-in, FTS5 triggers, parser, batch crawler, event-driven recursive root watch/recrawl, search UI | Offline/partial scan dan recrawl serialization sudah diperbaiki di kode; freshness state/polling/resume, pagination/cancellation, dan fixture convergence belum lengkap | Sebagian; gap acceptance |
| M7 | Synthetic 100k sort/query tests, snapshot count cap, CSP | Bukan real 100k directory/crawl acceptance; ringkasan benchmark hardcoded; process-tree metrics/p95/native-copy baseline/audit belum terbukti | Belum accepted |
| M8 | NSIS currentUser configuration, release bundle metadata/checksums | Native runner hanya process smoke; installer smoke stub; clean-machine install/journey/upgrade/uninstall belum memiliki bukti memadai | Packaging ada; MVP gate belum accepted |

Referensi exit criteria: [spesifikasi bagian 24](D:/dev/rust-explorer/RUST_WINDOWS_EXPLORER_AGENT_SPEC.md:1005).

## Temuan prioritas

P1 berarti harus diselesaikan sebelum acceptance MVP karena berdampak pada authority, data safety, correctness, atau fitur wajib. P2 berarti gap kontrak/UX/validasi yang tetap harus ditutup untuk acceptance.

### R1 — P1: IPC commit mempercayai plan milik caller

`commit_plan` menerima seluruh `OperationPlan` yang dapat dideserialisasi, termasuk paths, kind, expiry, dan commit token. Executor hanya memeriksa expiry dan apakah token pernah dipakai; tidak mencari immutable plan dalam registry Rust, tidak mengikat session/window, dan tidak membuktikan bahwa planner pernah menghasilkan plan tersebut. Caller IPC dapat mengubah plan atau memasok plan baru sehingga checks pada planner tidak menjadi authority boundary. Copy/move/recycle juga menerima `Vec<String>` paths; UI membentuk source path dari display label.

Lokasi: [commands.rs:253](D:/dev/rust-explorer/apps/desktop/src-tauri/src/commands.rs:253), [commands.rs:347](D:/dev/rust-explorer/apps/desktop/src-tauri/src/commands.rs:347), [executor.rs:37](D:/dev/rust-explorer/crates/explorer-jobs/src/executor.rs:37), [App.tsx:843](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:843).

Spec 7.1, 8.2, 12.1: native source/destination tokens; immutable server-owned plan; explicit confirmation; idempotent client request. Perbaikan: simpan plan Rust, commit dengan plan ID + confirmation + request ID, resolve native tokens, dan kembalikan job yang sama pada duplicate submission.

### R2 — P1: Lossless Windows paths rusak dalam jalur produksi

Konversi dasar `encode_wide/from_wide` benar, tetapi `ensure_extended_prefix` membangun ulang path dari `to_string_lossy()`. Enumeration menyimpan display string saja; snapshot dan crawler membangun path kembali dari `display_name`. Nama berisi unpaired UTF-16 dapat berubah menjadi replacement character, dan nama berbeda dapat menuju display path yang sama. Clipboard import memakai `String::from_utf16_lossy` pula. Penyimpanan index sebagai BLOB tidak memulihkan informasi yang sudah hilang sebelum insert.

Lokasi: [path.rs:37](D:/dev/rust-explorer/crates/explorer-win/src/path.rs:37), [enumerate.rs:82](D:/dev/rust-explorer/crates/explorer-win/src/enumerate.rs:82), [snapshots.rs:29](D:/dev/rust-explorer/crates/explorer-fs/src/snapshots.rs:29), [crawl.rs:86](D:/dev/rust-explorer/crates/explorer-index/src/crawl.rs:86).

Probe pada helper nyata mengonfirmasi `unpaired_surrogate_preserved_by_prefix=false`. Ini bukan uji mutasi file bernama demikian; ini bukti bahwa transformasi path native sudah menghilangkan unit aslinya. Spec 7.1/19.2 membutuhkan native path tetap terpisah dari label display hingga resolve token dan operasi.

### R3 — P1: Revalidasi identitas dan preflight mutasi belum ada

Plan menyimpan path tanpa identitas file/volume/parent. `identity.rs` dan `policy.rs` masih komentar placeholder. Commit tidak re-query identity sebelum operasi Shell; file yang diganti di path yang sama setelah planning dapat dikenai operasi yang awalnya ditujukan kepada file lain. Copy/move memakai lexical `starts_with` tanpa resolved identity/ancestry, tidak menghapus selected child di bawah selected ancestor, dan tidak memeriksa seluruh tree untuk reparse points/placeholders. Perlindungan app/state directory juga belum diterapkan. Pemeriksaan sebagian `exists()` di backend tidak menyelesaikan identity swap.

Lokasi: [operations.rs:32](D:/dev/rust-explorer/crates/explorer-domain/src/operations.rs:32), [planner.rs:157](D:/dev/rust-explorer/crates/explorer-jobs/src/planner.rs:157), [executor.rs:80](D:/dev/rust-explorer/crates/explorer-jobs/src/executor.rs:80).

Spec 7.2/7.4/12.1: identity revalidation, protected roots, conservative reparse semantics, cancelable metadata preflight. Acceptance membutuhkan stale-item, reparse, protected-path, nesting, dan source-preservation matrix.

### R4 — P1: Job model tidak mempertahankan partial outcomes atau menyediakan cancellation

Executor mereduksi `SinkReport` menjadi `completed`, membuang `failed`/error detail, kemudian memberi state `Succeeded` untuk setiap `Ok`. Ketika Shell membatalkan sesudah beberapa item selesai, report berubah menjadi `Err`, sehingga completed items hilang dan semua item dihitung gagal. `PartialFailure`, `CancelRequested`, dan `ItemOutcome` hanya didefinisikan di domain; tidak digunakan oleh jalur eksekusi/jurnal. `UpdateProgress` kosong; tidak ada registered `cancel_job`, live progress events, atau Retry failed items. Create/rename bahkan belum memakai progress sink yang ada.

Lokasi: [executor.rs:132](D:/dev/rust-explorer/crates/explorer-jobs/src/executor.rs:132), [executor.rs:161](D:/dev/rust-explorer/crates/explorer-jobs/src/executor.rs:161), [sink.rs](D:/dev/rust-explorer/crates/explorer-win/src/sink.rs), [lib.rs:32](D:/dev/rust-explorer/apps/desktop/src-tauri/src/lib.rs:32).

Spec 12.2/13/26.2: explicit per-item outcomes dan terminal aggregation yang benar. Simpan report lengkap bahkan ketika PerformOperations gagal/cancel; catat hasil individual sebelum event terminal.

### R5 — P1: Offline/partial crawl menghapus metadata yang seharusnya dipertahankan

Crawler `continue` pada enumeration error, lalu tetap menjalankan `prune_unseen_entries` untuk seluruh root dan menandai `Ready`. Root offline dapat kehilangan semua indexed entries; subfolder AccessDenied dapat kehilangan indexed descendants. Ini penghapusan metadata index, bukan penghapusan file pengguna.

Lokasi: [crawl.rs:76](D:/dev/rust-explorer/crates/explorer-index/src/crawl.rs:76), [crawl.rs:146](D:/dev/rust-explorer/crates/explorer-index/src/crawl.rs:146).

Probe in-memory dengan satu entry lama dan root yang dipastikan tidak ada menghasilkan `before=1; after=0; state=Ready`. Spec 14.4/26.2 secara eksplisit melarang index-wide sweep dari partial/offline scans. Pruning harus per parent yang selesai dibaca dengan sukses; keadaan gagal/offline wajib dipertahankan.

### R6 — P1: Watch overflow dan freshness index belum pulih secara end-to-end

Saat ingress penuh, adapter mencoba mengirim overflow ke channel yang masih penuh. Atomic overflow flag disimpan tetapi `check_and_clear_overflow` tidak dipanggil oleh produksi. Forwarder Tauri memakai `while let Ok(...)`, sehingga `RecvError::Lagged` menghentikan pengiriman event selamanya. Batas dirty dirs juga masih melakukan insert via `mark_dir_overflow` ketika melewati 1.024. ReconciliationManager tidak terhubung ke dirty/overflow lifecycle; index service tidak mendaftarkan recursive watches atau menangani perubahan index setelah crawl.

Lokasi: [adapter.rs:47](D:/dev/rust-explorer/crates/explorer-watch/src/adapter.rs:47), [lib.rs:21](D:/dev/rust-explorer/apps/desktop/src-tauri/src/lib.rs:21), [coalesce.rs:100](D:/dev/rust-explorer/crates/explorer-watch/src/coalesce.rs:100), [service.rs:54](D:/dev/rust-explorer/crates/explorer-index/src/service.rs:54).

Spec 14.4/15: dirty signal yang tidak dapat hilang, root watching sebelum crawl, fallback polling, forced overflow convergence. Recrawl juga mengganti cancel flag tanpa membatalkan/join worker lama, sehingga repeated rebuild dapat menjalankan beberapa crawl/prune epochs bersamaan.

### R7 — P1: Clipboard parser membaca native buffer tanpa memvalidasi ukuran alokasi

Parser mendereferensi `DROPFILES`, offset `pFiles`, UTF-16 elements, dan preferred effect tanpa `GlobalSize`/validasi ukuran dan alignment. Batas scan 1 MiB adalah batas iterasi, bukan bukti bahwa memory allocation berukuran 1 MiB. Clipboard malformed dari aplikasi lain dapat menyebabkan out-of-bounds read/crash. Import juga tidak memverifikasi double-NUL completion dan dapat berhenti di cap tanpa explicit oversized error. Write mengabaikan kegagalan SetClipboardData. Test roundtrip lokal tidak membuktikan Explorer cut completion/partial-result semantics.

Lokasi: [clipboard.rs:158](D:/dev/rust-explorer/crates/explorer-win/src/clipboard.rs:158), [clipboard.rs:176](D:/dev/rust-explorer/crates/explorer-win/src/clipboard.rs:176), [clipboard.rs:111](D:/dev/rust-explorer/crates/explorer-win/src/clipboard.rs:111).

Spec 11.2/19.2: validate untrusted buffer, preserve native paths, reject oversized imports, proper ownership/error handling, Explorer two-way acceptance. Review ini tidak memasukkan malformed data ke clipboard pengguna.

### R8 — P1: Single-instance, bounded execution, dan STA lifecycle belum memenuhi kontrak

Host membuat AppState/executor baru pada setiap launch tanpa single-instance plugin atau per-user mutex/forwarding. Dua launch dapat memakai state DB yang sama dan menghasilkan dua mutation executors; startup recovery instance kedua dapat menandai job instance pertama Interrupted. OperationService bukan queue 32 jobs; STA memakai unbounded channel dan blocking `recv`, tanpa message pump atau join shutdown. Async mutation handlers memanggil executor blocking langsung; ini memblokir worker async, bukan bukti bahwa main UI thread diblokir. Shell properties/open berada di arbitrary `spawn_blocking`; metadata/clipboard STA terpisah belum ada. Index memakai satu shared mutex connection, bukan writer actor + reader workers.

Lokasi: [lib.rs:12](D:/dev/rust-explorer/apps/desktop/src-tauri/src/lib.rs:12), [state.rs:20](D:/dev/rust-explorer/apps/desktop/src-tauri/src/state.rs:20), [queue.rs:12](D:/dev/rust-explorer/crates/explorer-jobs/src/queue.rs:12), [com.rs:16](D:/dev/rust-explorer/crates/explorer-win/src/com.rs:16), [commands.rs:253](D:/dev/rust-explorer/apps/desktop/src-tauri/src/commands.rs:253).

Spec 6.2 dan AGENTS.md: satu executor per user/app identity, bounded scheduling, proper apartments, graceful shutdown, dan persistence drain.

### R9 — P1: Large-list contract berhenti di 5.000 entri

`loadDirectory` meminta satu page offset 0 limit 5.000 lalu langsung menandai loading selesai; tidak memakai `is_last_page` atau mengambil page berikutnya. Pada folder 100k, entri ke-5.001 dst tidak tersedia untuk scroll/filter/select. Backend juga belum membatasi page size 256; native enumeration mengumpulkan seluruh Vec sebelum navigate mengembalikan hasil. Virtualisasi DOM dan synthetic sort benchmark tidak membuktikan streamed first rows atau 100k UI acceptance. Indexed search UI analoginya hanya meminta 100 hasil pertama.

Lokasi: [App.tsx:241](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:241), [App.tsx:415](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:415), [lib.rs:47](D:/dev/rust-explorer/crates/explorer-fs/src/lib.rs:47).

Spec 8.2/9/18: bounded page IPC, cancelable enumeration, complete logical results dan measured first-row latency pada physical fixture.

### R10 — P2: Interaction, DTO, dan settings acceptance masih kurang

Navigation/listing replies tidak dicek terhadap request terkini dalam tab: A yang selesai setelah B dapat menimpa state B. Refresh menghapus selection dan native enumeration membuat token baru pada setiap snapshot. Folder filter masih diproses penuh di React. `show_hidden_files` tersimpan tetapi tidak dipakai UI; column widths tidak ada di AppSettings. File rows tidak mempunyai selection roles/aria state/roving focus. Shift+Delete masuk handler Delete biasa, tanpa pesan disabled sesuai spec. Address relatif canonicalize terhadap process CWD, bukan tab directory. DTO tanpa schema version dan u64 bytes/filetime tetap dikirim sebagai JS number.

Lokasi: [App.tsx:243](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:243), [App.tsx:265](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:265), [App.tsx:1117](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:1117), [App.tsx:1713](D:/dev/rust-explorer/apps/desktop/ui/src/app/App.tsx:1713), [settings.rs:8](D:/dev/rust-explorer/crates/explorer-store/src/settings.rs:8).

Validasi nama juga `trim()` sebelum memeriksa trailing space; probe `validate_file_name("report ")` menghasilkan `true`. Batas 255 memakai UTF-8 bytes, bukan volume component limit/UTF-16. Spec 7/8/10/23 mengharuskan input asli tidak diam-diam dinormalisasi dan stale requests tidak diterapkan.

### R11 — P1 (release gate): Native E2E/installer acceptance belum dibuktikan

Native runner spawn exe, menunggu 3,5 detik, membaca Get-Process/WorkingSet64, dan melihat state/bundle paths. Tidak ada tauri-driver/WebdriverIO/Edge Driver, UI actions, IPC assertions, atau verifikasi changes pada fixture. Bahkan Responding/state-directory-exists hanya dilog, bukan mandatory assertions. `xtask smoke-installer` adalah stub yang selalu return Ok. Karena itu runner tidak membuktikan installed standard-user journey, conflicts, cancellation, Explorer clipboard, Recycle recovery, upgrade, uninstall, atau developer-tools-free clean machine.

Lokasi: [runner.js:40](D:/dev/rust-explorer/apps/desktop/tests/native-e2e/runner.js:40), [runner.js:65](D:/dev/rust-explorer/apps/desktop/tests/native-e2e/runner.js:65), [smoke.rs:1](D:/dev/rust-explorer/xtask/src/smoke.rs:1).

Vitest hanya menguji helper yang disalin/dibuat di file test dan range math, tanpa import/render production App atau deterministic bridge. ESLint dan Playwright tidak terdaftar dalam devDependencies dan command-nya gagal. Spec 19/24/26 mewajibkan outcome assertions dan manual/native evidence terpisah; process smoke bukan substitusi.

Pemeriksaan read-only tambahan membuktikan exe, NSIS, dan MSI memang tersedia dan SHA-256 ketiganya cocok dengan BUILD_METADATA.json. Namun `sizeBytes` metadata tidak cocok dengan file aktual: NSIS 3.720.530 vs metadata 3.722.744; MSI 5.398.528 vs 5.402.624; exe 15.130.112 vs 15.126.528 bytes. Packaging nyata ada, tetapi metadata perlu diregenerasi dan kecocokan hash tidak membuktikan installer journey.

### R12 — P1 (release gate): Performance report tidak berasal dari pengukuran gate yang lengkap

`xtask bench` menjalankan synthetic tests, lalu menulis angka tetap (~34 ms, ~102 ms, ~5.400 items/sec, dst) ke summary tanpa mengambil hasil run. Tests listing membuat Vec sintetis; index memasukkan metadata sintetis, bukan physical crawl. Benchmark bermanfaat untuk algorithms tetapi tidak membuktikan physical 100k listing/crawl. Startup check mengukur host working set setelah fixed sleep, bukan usable-window time atau process-tree private bytes termasuk WebView2. Tidak ada p95 multi-run, native Shell control throughput, watcher latency, idle CPU/leak report, atau explicit approved waivers pada evidence yang diperiksa.

Lokasi: [bench.rs:48](D:/dev/rust-explorer/xtask/src/bench.rs:48), [stress_tests.rs:63](D:/dev/rust-explorer/crates/explorer-fs/tests/stress_tests.rs:63), [stress_tests.rs:11](D:/dev/rust-explorer/crates/explorer-index/tests/stress_tests.rs:11), [runner.js:66](D:/dev/rust-explorer/apps/desktop/tests/native-e2e/runner.js:66).

Spec 18/20/26: actual metrics, physical fixtures, build/seed/environment, iterations, p50/p95/max, raw JSON/CSV, failures, process-tree resources. Dependency/license checks dan clean-machine report juga belum mempunyai bukti yang cukup untuk klaim complete. MSI adalah scope LATER; membuatnya tidak menutup required NSIS acceptance.

## Validasi yang benar-benar dijalankan dalam review

Semua command dijalankan pada baseline di atas, 4 Oktober 2026. Rust memakai PATH tambahan `%USERPROFILE%\.cargo\bin`, tanpa instalasi atau perubahan global. `--offline --locked` dipakai untuk Rust checks yang mendukungnya.

| Command | Hasil | Arti/batas |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | Formatting |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS | Compile/lint Rust seluruh target; bukan native behavior acceptance |
| `cargo test --package explorer-domain --locked --offline` | PASS, 2 tests | IDs/serialization dan structured error |
| `cargo test --package explorer-store --locked --offline` | PASS, 2 tests | In-memory settings dan journal recovery |
| `cargo test --package explorer-win path::tests --lib --locked --offline` | PASS, 3 tests | Basic Unicode conversion/prefix/safe path cases; tidak menguji surrogate pada pipeline |
| `cargo test --package explorer-fs listing::tests --lib --locked --offline` | PASS, 2 tests | Natural sort/folders first |
| `cargo test --package explorer-index query::tests --lib --locked --offline` | PASS, 2 tests | Parser/ranking/filter pada in-memory DB |
| `cargo test --package explorer-jobs test_name_validation --lib --locked --offline` | PASS, 1 test | Kasus nama yang sudah ada; tidak mencakup trailing space bug |
| `npm run typecheck` | PASS | TypeScript compile |
| `npm run test:unit` | PASS, 8 tests | Helper tests; tidak menguji production component interactions |
| `npm run build:ui` | PASS | Production UI bundle |
| `npm run lint` | FAIL, exit 1 | `eslint` tidak dikenali; dependency/config belum tersedia |
| `npm run test:e2e:ui` | FAIL, exit 1 | `playwright` tidak dikenali; dependency/harness belum tersedia |
| `npm ls eslint @playwright/test playwright @testing-library/react --depth=0` | Empty, exit 1 | Konfirmasi packages tidak tersedia |
| `cargo build --package explorer-win --package explorer-jobs --package explorer-index --lib --locked --offline` | PASS | Build libraries untuk probe |
| `Get-FileHash -Algorithm SHA256` untuk exe/NSIS/MSI vs BUILD_METADATA.json | 3 hashes MATCH; 3 size fields MISMATCH | Artifacts tersedia; metadata size perlu diperbaiki; bukan clean-machine acceptance |
| Probe helper native path + name + offline crawl/in-memory DB | Executed, exit 0; 3 defects observed | Path surrogate hilang; trailing space diterima; offline scan 1→0 entries dan Ready |

Probe disimpan di [probe.rs](D:/dev/rust-explorer/artifacts/reviews/objective-alignment-2026-10-04/probe.rs), output di [probe-output.txt](D:/dev/rust-explorer/artifacts/reviews/objective-alignment-2026-10-04/probe-output.txt). Probe memakai existing libraries dan in-memory SQLite; tidak membuat source fixture files, menjalankan Shell mutations, atau membaca/menulis clipboard. Compile standalone pertama gagal menemukan native Windows library; setelah native search paths ditambahkan compile dan run sukses. Ini kesalahan invocation probe, bukan kegagalan Cargo project build.

**Tidak dijalankan:** full workspace runtime tests yang mencakup mutasi/clipboard; native runner; installer install/uninstall; physical 100k/copy benchmarks; audit tools dan manual edge cases. Sebagian mutation tests masih memakai unmarked `tempdir()`; fixture generator memakai fixed `target/fixtures/sample_tree` tanpa marker/run manifest. Sesuai AGENTS.md, review tidak menjalankan harness mutasi tersebut atau meluncurkan competing app instance. Ini batas cakupan review, bukan capability pass atau approved waiver. Tidak ada native/clean-machine test yang baru dinyatakan lulus.

## Urutan perbaikan yang disarankan

1. Selesaikan sisa R1–R4/R7/R8: ikat plan ke session/window dan request identity; lengkapi tree/protected/reparse preflight; tambahkan incremental outcomes untuk Create/Rename, app-level cancellation, live progress, dan retry; lengkapi single-instance, bounded execution, STA message pump/shutdown; jalankan safe clipboard acceptance.
2. Selesaikan sisa R6: polling/resume saat watch/volume pulih, freshness completion yang konsisten, dan buktikan convergence setelah overflow. Recursive root watch, event-triggered debounced recrawl, safe scoped pruning, crawl cancellation/serialization, serta overflow/lag recovery sudah diterapkan tetapi belum diterima pada fixture.
3. Selesaikan sisa R9–R10: streamed/cancelable native listing dan physical 100k behavior; lengkapi DTO precision, relative-path basis, settings persistence, keyboard/accessibility, dan live search freshness. Page cap, incremental UI publication, stale-response guards, hidden-file toggle, dan row selection semantics sudah diterapkan.
4. Bangun marked fixture harness serta production-component/native outcome checks; jalankan correctness/edge-case matrix dengan capability skips eksplisit. Lengkapi lint/dependency/license checks.
5. Ukur section 18/20 dengan physical fixtures dan runtime process tree; gunakan actual results dalam report. Terakhir jalankan NSIS standard-user install/journey/upgrade/uninstall di clean Windows environment dan gate 26.2.

Objective produk tidak perlu diubah. Yang perlu diperbaiki adalah invariant produksi dan standar bukti acceptance sebelum menyebut aplikasi MVP selesai.
