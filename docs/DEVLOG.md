# Development Log

Catatan perkembangan DB-Eye. Update file ini setiap ada perubahan besar agar konteks development tidak hilang.

## 2026-06-02

### Selesai

- Implementasi PostgreSQL schema support:
  - `DbClient` sekarang mendukung `list_schemas` dan metadata schema-aware.
  - Alur schema selection screen setelah memilih database PostgreSQL.
  - Navigasi bolak-balik antara Main screen, Schemas, dan Databases screen.
  - Dukungan schema-qualified table names pada CRUD (Insert/Update/Delete).
  - Tampilan indikator active schema pada title panel tables.
- Menambahkan Help screen popup via `?` untuk membantu user mempelajari keybindings.
- Menambahkan laporan "rows affected" untuk custom write queries.
- Menambahkan in-memory query history per tab dengan navigasi `Up/Down`.
- Menambahkan fitur Saved Connections:
  - Persistensi menggunakan `serde` dan `serde_json` ke `~/.config/db-eye/connections.json`.
  - Dukungan untuk menyimpan SQLite path dan Server connection form.
  - Alur UI untuk switch focus (`Tab`) antara input form dan daftar saved connections.
  - Shortcut `Ctrl+S` untuk save dan `Delete` untuk hapus.

### Catatan Teknis

- Method `DbClient` (`list_tables`, `query_table`, `get_columns`, `count_rows`, `get_fk_values`) sekarang menerima parameter `schema` (Option<&str>).
- `CrudForm` menyisipkan schema-qualified identifier pada SQL generation untuk PostgreSQL.
- Borrow checker issue di `handle_schemas` diselesaikan dengan memisahkan state update dan borrow tab.
- `DbClient::execute_query` sekarang menggunakan `fetch_many` (via stream) untuk mendapatkan rows sekaligus metadata (rows affected).
- Menambahkan dependensi `futures`, `serde`, dan `serde_json`.
- Pindah logika `is_read_only_sql` ke `src/db.rs` sebagai utility database level.
- `ConnectForm` menggunakan `#[serde(skip)]` untuk field `pass` agar password tidak tersimpan dalam plain text (safety measure).
- Tests diperbarui agar sesuai dengan signature method baru yang schema-aware.

### Validasi

- `cargo fmt` sukses.
- `cargo test` sukses: 11 tests passed.
- `cargo check` sukses.
- Berhasil menangani composite PK dan schema non-public secara bersamaan.

### Risiko / Debt

- `Esc` pada `Schemas` screen saat ini selalu pop tab, yang bisa membingungkan jika user datang dari `Main` screen.
- Belum ada dropdown/select untuk schema switching langsung dari Main screen tanpa lewat pemilihan schema.
- Metadata MySQL/SQLite masih mengabaikan parameter schema (karena memang flat/database-level).

### Next Recommended Work

1. Tambahkan help screen `?` untuk keybindings.
2. Tambahkan query history.
3. Tambahkan rows affected untuk write query.
4. Tambahkan saved connections.

## 2026-06-01

### Selesai

- Menambahkan roadmap production di `PRD.md`.
- Menambahkan CRUD row dasar:
  - insert row
  - update selected row
  - delete selected row dengan konfirmasi
- Menambahkan foreign-key value hints di form insert/update.
- Memperbarui README agar install menggunakan `cargo install db-eye`.
- Version dinaikkan ke `0.2.0`.

### Catatan Teknis

- CRUD execution sudah memakai bind placeholders untuk values.
- Read-only mode ditambahkan via `--read-only` / `-r` untuk memblokir CRUD dan custom SQL write.
- Friendly database error formatter ditambahkan untuk constraint, permission, connection, dan syntax errors.
- SQLite CRUD end-to-end test ditambahkan untuk insert/update/delete.
- Composite primary key support ditambahkan untuk SQLite/PostgreSQL/MySQL metadata dan CRUD statements.
- SQL preview masih menampilkan SQL literal agar user mudah membaca perubahan sebelum save.
- Update/delete saat ini membutuhkan primary key tunggal.
- PostgreSQL metadata masih fokus schema `public`.
- Foreign-key UI masih berupa hints, belum dropdown/select.

### Validasi

- `cargo fmt` sukses.
- `cargo test` sukses: 11 tests.
- `cargo check` sukses.
- `cargo clippy` 0 error, masih ada warning cleanup non-blocking.

### Risiko / Debt

- Perlu integration tests lebih lengkap untuk flow CRUD end-to-end.
- Perlu handling tipe data yang lebih benar, terutama NULL vs string `"NULL"`.
- Perlu support composite primary key.
- Read-only SQL guard masih berbasis first-token allowlist; perlu parser/statement classifier lebih kuat untuk edge case.

### Next Recommended Work

1. Tambahkan PostgreSQL schema support.
2. Tambahkan help screen `?` untuk keybindings.
3. Tambahkan query history.
4. Tambahkan saved connections.
