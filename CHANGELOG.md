# Changelog

Semua perubahan penting DB-Eye dicatat di file ini.

Format mengikuti [Keep a Changelog](https://keepachangelog.com/) dan versi mengikuti Semantic Versioning sebisa mungkin.

## [Unreleased]

### Added

- Inline Cell Editing:
  - Edit cell langsung di panel data via `Enter` atau `e`.
  - Simpan perubahan ke database via parameterized `UPDATE`.
  - Visual feedback warna kuning pada cell yang sedang diedit.
- Data type handling:
  - Validasi input CRUD/inline edit untuk integer, number, boolean, dan binary/blob.
  - Nilai NULL ditampilkan sebagai `<NULL>` supaya string `NULL` tetap beda dari database NULL.
  - Input kosong atau `\\null` pada form CRUD/edit dikirim sebagai database NULL.
- Saved Connections:
  - Simpan konfigurasi koneksi (host, user, dsb) via `Ctrl+S`.
  - Pilih dari daftar koneksi tersimpan pada screen Connect via `Tab`.
  - Persistensi data menggunakan file JSON lokal.
  - Shortcut `Delete` untuk menghapus koneksi tersimpan.
- Help screen popup via `?` key displaying categorized keybindings.
- "Rows affected" reporting for custom write queries (INSERT, UPDATE, DELETE).
- In-memory query history per tab with `Up/Down` navigation in SQL query popup.
- PostgreSQL schema support:
  - Schema selection screen flow setelah pilih database.
  - Dukungan untuk schema non-public pada data browsing, CRUD, dan metadata.
  - Schema-qualified table names pada SQL generation.
  - Tampilan active schema pada UI table panel.
- `PRD.md` sebagai production roadmap.
- `docs/DEVELOPMENT.md` untuk alur development dan definition of done.
- `docs/DEVLOG.md` untuk catatan perkembangan.
- Template feature spec dan release checklist.
- Unit tests untuk CRUD statement builder.
- SQLite test untuk parameterized write values.
- Read-only mode via `--read-only` / `-r` untuk memblokir write actions dan destructive custom SQL.
- Friendly database error formatter untuk constraint, permission, connection, dan syntax errors.
- SQLite CRUD end-to-end test untuk insert/update/delete flow.
- Composite primary key support untuk update/delete statements dan SQLite metadata tests.
- GitHub Actions CI untuk fmt, clippy, test, dan release build.
- Direct PostgreSQL/MySQL connection URL input on the Connect screen.
- Terminal demo GIF in README (`docs/demo.tape`, recorded with `vhs`).
- Richer column metadata (SQLite/PostgreSQL/MySQL):
  - Required (`*`) and default-value indicators on insert/update form fields.
  - Declared length/precision in the displayed column type (e.g. `varchar(255)`, `numeric(10,2)`).
- Views listed alongside tables in the Tables panel, labeled `(view)` (previously silently mixed in unlabeled for MySQL, silently excluded for SQLite/PostgreSQL).
- Table stats popup (`s` in the data panel): index list and approximate on-disk size.
- Manual reconnect action (`Ctrl+R`): re-establishes a dropped connection on the current tab using the original connection URL, then reloads schema/table list and the open table's data if one was loaded. Previously a dropped connection required closing the tab (`Ctrl+W`) and reconnecting from scratch.
- Per-column sorting (`o` in the data panel): cycles asc → desc → off, implemented as a DB-side `ORDER BY` so it stays correct across pagination.
- Explicit pagination controls: `PageUp`/`PageDown` for full-page jumps, `g` to jump to a specific row number.
- Search now shows a live match count in the status bar instead of a stale pre-search message.
- FK dropdown on insert/update forms: `←`/`→` cycles known foreign-key values directly into the field, instead of only showing them as a hint you had to type manually.

### Fixed

- Data panel status hint mislabeled `e` as export (`e` is inline cell edit; `v` is CSV export).

### Changed

- CRUD insert/update/delete sekarang memakai bind placeholders untuk values, bukan menyisipkan value langsung ke SQL eksekusi.
- Custom SQL execution no longer uses deprecated `fetch_many`; read queries fetch rows and write queries report rows affected via `execute`.

## [0.2.0] - 2026-06-01

### Added

- CRUD row dasar dari data panel:
  - insert row
  - update selected row
  - delete selected row dengan konfirmasi
- SQL preview pada form insert/update.
- Foreign-key value hints pada form insert/update.
- Dokumentasi install via `cargo install db-eye`.

### Changed

- Version package dinaikkan dari `0.1.0` ke `0.2.0`.
- Status bar data panel menampilkan shortcut CRUD.

### Known Limitations

- CRUD belum menggunakan parameterized query.
- Update/delete membutuhkan primary key tunggal.
- PostgreSQL schema support masih fokus `public`.
- Belum ada automated integration tests.

## [0.1.0]

### Added

- Browse database SQLite, PostgreSQL, dan MySQL.
- Connect screen.
- Table browser.
- Data panel dengan pagination sederhana.
- Search/filter rows.
- Custom SQL query.
- Export CSV.
- Multi-tab connections.
