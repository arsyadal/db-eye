# PRD — DB-Eye Production Roadmap

## Status Saat Ini

DB-Eye sudah usable sebagai terminal UI database browser untuk SQLite, PostgreSQL, dan MySQL. Fitur dasar sudah tersedia: koneksi database, browse table, search, custom query, export CSV, multi-tab, dan CRUD row sederhana.

Target dokumen ini adalah daftar fitur yang perlu diimplementasikan atau bisa dikembangkan agar DB-Eye siap production-grade.

## Tujuan Produk

- Menjadi database browser TUI yang ringan, cepat, dan aman.
- Mendukung workflow harian developer/DBA untuk inspeksi, query, dan edit data.
- Aman digunakan pada database lokal maupun server development/staging/production.
- Mudah di-install via `cargo install db-eye`.

## Prioritas P0 — Wajib untuk Production

### 1. Keamanan Query dan CRUD

- Gunakan parameterized query/bind values untuk insert, update, delete.
- Hindari build SQL value manual untuk input user.
- Validasi identifier table/column dengan metadata database.
- Tambahkan mode read-only untuk koneksi production.
- Tambahkan konfirmasi ekstra untuk operasi destructive:
  - delete row
  - update banyak row
  - drop/truncate jika custom query menjalankan statement destructive

### 2. Testing Otomatis

- Integration test SQLite untuk:
  - connect
  - list table
  - query table
  - search/filter
  - insert/update/delete
  - export CSV
- Test SQL quoting untuk nama table/column dengan spasi atau reserved keyword.
- Test error handling untuk constraint violation.
- Test multi-tab state isolation.
- Setup CI GitHub Actions:
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo test`
  - build release

### 3. Schema dan Primary Key Handling

- PostgreSQL: support schema selain `public`.
- MySQL: support database/table metadata lebih lengkap.
- Support composite primary key untuk update/delete.
- Jika table tidak punya primary key:
  - tampilkan warning
  - disable update/delete, atau
  - gunakan safe row identity strategy bila memungkinkan

### 4. Data Type Handling

- Deteksi tipe data column dari metadata.
- Input form harus handle:
  - string
  - integer
  - float/decimal
  - boolean
  - date/time/timestamp
  - NULL
  - blob/binary sebagai read-only/placeholder
- Jangan treat semua input sebagai string.
- Tampilkan nilai NULL beda dari string `"NULL"`.

### 5. Error Handling dan Recovery

- Pesan error user-friendly untuk:
  - connection failed
  - permission denied
  - foreign key violation
  - unique constraint violation
  - invalid SQL
  - network timeout
- Jangan crash saat query gagal atau koneksi putus.
- Tambahkan reconnect action.

## Prioritas P1 — Sangat Direkomendasikan

### 1. UX CRUD Lebih Baik

- Inline cell edit dari data panel.
- Form insert/update dengan:
  - required/not-null indicator
  - default value indicator
  - FK dropdown/select, bukan hanya hint
  - reset field
  - clear to NULL shortcut
- Support paste multi-character dengan benar.
- Support horizontal scroll di form untuk text panjang.
- Support multiline text editor popup.

### 2. Query Experience

- Query history per tab.
- Save named queries.
- Syntax highlighting SQL.
- Run selected statement jika input berisi banyak statement.
- Tampilkan rows affected untuk write query.
- Explain query plan:
  - SQLite `EXPLAIN QUERY PLAN`
  - PostgreSQL `EXPLAIN`
  - MySQL `EXPLAIN`

### 3. Data Browsing

- Sorting per column.
- Filter per column.
- Pagination yang lebih eksplisit:
  - next page
  - previous page
  - jump offset/page
- Show total filtered rows jika memungkinkan.
- Column hide/show.
- Copy cell/row/column value ke clipboard.

### 4. Connection Management

- Saved connections/config file.
- Password via environment variable atau prompt secure.
- Connection string input manual.
- SSH tunnel support.
- TLS options untuk PostgreSQL/MySQL.
- Recent connections list.

### 5. Export/Import

- Export CSV dengan opsi delimiter/header.
- Export JSON.
- Export SQL insert statements.
- Import CSV ke table existing.
- Preview sebelum import.

## Prioritas P2 — Nice to Have

### 1. Visual dan Navigasi

- Theme support.
- Help screen `?` berisi semua keybindings.
- Command palette.
- Mouse support opsional.
- Status bar lebih informatif:
  - active database
  - active schema
  - mode read/write
  - query duration

### 2. Observability

- Query timing.
- Slow query warning.
- Basic connection stats.
- Optional debug log file.

### 3. Advanced Database Tools

- Table schema viewer.
- Index viewer.
- Foreign key relation viewer.
- Table create SQL/DDL viewer.
- Database size/table size info.
- Migration diff ringan antar schema.

### 4. Packaging dan Distribution

- Release binary untuk macOS/Linux/Windows.
- Homebrew formula.
- AUR package.
- Docker image optional.
- Demo GIF dan screenshots di README.

## Non-Goals Saat Ini

- Menjadi full database administration suite seperti DataGrip/DBeaver.
- Visual query builder kompleks.
- ORM/migration framework.
- Production write operations tanpa guard/read-only mode.

## Development Documentation Standard

Setiap development wajib terdokumentasi agar arah produk tetap jelas:

- Fitur baru dimulai dari feature spec menggunakan `docs/templates/FEATURE_SPEC.md`.
- Perubahan user-facing wajib memperbarui `README.md`.
- Perubahan roadmap/prioritas wajib memperbarui `PRD.md`.
- Perubahan yang akan masuk rilis wajib dicatat di `CHANGELOG.md`.
- Perubahan besar/technical debt wajib dicatat di `docs/DEVLOG.md`.
- Keputusan arsitektur signifikan wajib dibuat sebagai ADR di `docs/adr/`.
- Release wajib mengikuti `docs/templates/RELEASE_CHECKLIST.md`.

## Acceptance Criteria Production v1.0

DB-Eye bisa dianggap production-ready jika:

- Semua P0 selesai.
- CI hijau untuk fmt, clippy, test, dan release build.
- CRUD menggunakan parameterized query.
- Update/delete aman untuk primary key tunggal dan composite key.
- PostgreSQL schema non-public didukung.
- Ada read-only mode.
- Error handling tidak menyebabkan crash pada skenario umum.
- README lengkap dengan install, usage, keybindings, safety notes, dan screenshots/demo.
- Release tag dan changelog tersedia.

## Roadmap Rilis

### v0.3

- Parameterized CRUD. ✅ initial implementation
- SQLite integration tests. ✅ initial CRUD coverage
- Read-only mode. ✅ initial implementation
- Better error messages. ✅ initial implementation

### v0.4

- PostgreSQL schema support.
- Composite primary key support.
- Query history.
- Rows affected untuk write query.

### v0.5

- Inline cell edit.
- FK dropdown.
- Saved connections.
- Help screen.

### v1.0

- Full P0 complete.
- CI/release pipeline.
- Production safety polish.
- Documentation lengkap.
