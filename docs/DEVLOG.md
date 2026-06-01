# Development Log

Catatan perkembangan DB-Eye. Update file ini setiap ada perubahan besar agar konteks development tidak hilang.

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
- SQL preview masih menampilkan SQL literal agar user mudah membaca perubahan sebelum save.
- Update/delete saat ini membutuhkan primary key tunggal.
- PostgreSQL metadata masih fokus schema `public`.
- Foreign-key UI masih berupa hints, belum dropdown/select.

### Validasi

- `cargo fmt` sukses.
- `cargo test` sukses: 9 tests.
- `cargo check` sukses.
- `cargo clippy` 0 error, masih ada warning cleanup non-blocking.

### Risiko / Debt

- Perlu integration tests lebih lengkap untuk flow CRUD end-to-end.
- Perlu handling tipe data yang lebih benar, terutama NULL vs string `"NULL"`.
- Perlu support composite primary key.
- Read-only SQL guard masih berbasis first-token allowlist; perlu parser/statement classifier lebih kuat untuk edge case.

### Next Recommended Work

1. Tambahkan support composite primary key.
2. Tambahkan PostgreSQL schema support.
3. Tambahkan help screen `?` untuk keybindings.
4. Tambahkan query history.
