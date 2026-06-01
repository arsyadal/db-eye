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

- CRUD saat ini masih membangun SQL string manual untuk values.
- Update/delete saat ini membutuhkan primary key tunggal.
- PostgreSQL metadata masih fokus schema `public`.
- Foreign-key UI masih berupa hints, belum dropdown/select.

### Risiko / Debt

- Perlu parameterized query sebelum dianggap production-ready.
- Perlu integration tests, minimal SQLite.
- Perlu handling tipe data yang lebih benar, terutama NULL vs string `"NULL"`.
- Perlu support composite primary key.

### Next Recommended Work

1. Implement parameterized CRUD.
2. Tambahkan SQLite integration tests.
3. Tambahkan read-only mode.
4. Perbaiki error message constraint violation.
