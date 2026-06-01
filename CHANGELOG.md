# Changelog

Semua perubahan penting DB-Eye dicatat di file ini.

Format mengikuti [Keep a Changelog](https://keepachangelog.com/) dan versi mengikuti Semantic Versioning sebisa mungkin.

## [Unreleased]

### Added

- `PRD.md` sebagai production roadmap.
- `docs/DEVELOPMENT.md` untuk alur development dan definition of done.
- `docs/DEVLOG.md` untuk catatan perkembangan.
- Template feature spec dan release checklist.
- Unit tests untuk CRUD statement builder.
- SQLite test untuk parameterized write values.

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
