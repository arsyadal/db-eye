# Changelog

Semua perubahan penting DB-Eye dicatat di file ini.

Format mengikuti [Keep a Changelog](https://keepachangelog.com/) dan versi mengikuti Semantic Versioning sebisa mungkin.

## [Unreleased]

### Added

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

### Changed

- CRUD insert/update/delete sekarang memakai bind placeholders untuk values, bukan menyisipkan value langsung ke SQL eksekusi.

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
