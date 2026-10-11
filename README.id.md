# DB-Eye

<img src="logo.svg" alt="Logo DB-Eye" width="360" />

[English](README.md) | **Bahasa Indonesia**

[![crates.io](https://img.shields.io/crates/v/db-eye.svg)](https://crates.io/crates/db-eye)
[![CI](https://github.com/arsyadal/db-eye/actions/workflows/ci.yml/badge.svg)](https://github.com/arsyadal/db-eye/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Browser database Terminal UI (TUI) yang cepat dan ringan untuk SQLite, PostgreSQL, dan MySQL.

![Demo DB-Eye](docs/demo.gif)

## Instalasi

Instal secara global menggunakan Cargo:

```bash
cargo install db-eye
```

Membutuhkan toolchain Rust yang mendukung edisi 2024 (rustc 1.85+). Berjalan di macOS, Linux, dan Windows.

Atau unduh file binary executable mandiri dari [GitHub Releases](https://github.com/arsyadal/db-eye/releases).

Untuk memperbarui instalasi yang sudah ada ke versi terbaru:

```bash
db-eye update
# atau: cargo install --force db-eye
```

Atau jalankan langsung dari source code:

```bash
cargo run -- ./path/to/file.db
```

## Penggunaan

```bash
# Buka file SQLite langsung (otomatis membuat file baru jika belum ada)
db-eye ./mydb.sqlite

# Buka dalam mode read-only (proteksi write di tingkat hardware/engine)
db-eye --read-only ./mydb.sqlite

# Buka tanpa argumen → layar koneksi
db-eye

# Cek dan pasang pembaruan otomatis
db-eye update

# Tampilkan versi aplikasi
db-eye --version

# Tampilkan panduan penggunaan
db-eye --help
```

## Layar Koneksi (Connect Screen)

| Tombol | Tindakan |
|--------|----------|
| `←` / `→` | Ganti tipe database (SQLite / PostgreSQL / MySQL) |
| `Tab` / `j` / `k` | Pindah antar field input form |
| `Enter` | Sambungkan (Connect) |
| `Esc` | Kembali ke tampilan utama (jika tab sudah terbuka) |

**SQLite** — masukkan path file database, lalu tekan Enter. *(File akan otomatis dibuat jika belum ada)*.

**PostgreSQL / MySQL** — isi Host, Port, User, Password → Enter → pilih database dari daftar.

Atau tempel URL koneksi lengkap pada kolom **Connection URL (optional)** lalu tekan Enter untuk menyambung langsung:

- `postgres://user:pass@host:5432/dbname`
- `mysql://user:pass@host:3306/dbname`

Anda juga dapat mengatur variabel lingkungan `DB_PASSWORD` agar password terisi otomatis saat membuka koneksi:

```bash
DB_PASSWORD=secret db-eye postgres://user@host:5432/dbname
```

Koneksi tersimpan (`Ctrl+S`) disimpan ke file JSON lokal — password tidak pernah disimpan demi keamanan:

- macOS/Linux: `~/.config/db-eye/connections.json`
- Windows: `%APPDATA%\db-eye\connections.json`

## Navigasi & Shortcut

### Global
| Tombol | Tindakan |
|--------|----------|
| `Ctrl+C` | Keluar dari aplikasi |
| `?` | Tampilkan / sembunyikan popup bantuan shortcut |
| `Ctrl+T` | Buka koneksi baru (Layar Connect) |
| `Ctrl+W` | Tutup tab aktif saat ini |
| `Ctrl+R` | Sambungkan ulang tab aktif (memulihkan koneksi terputus tanpa tutup tab) |
| `[` / `]` | Beralih ke tab sebelumnya / berikutnya |

### Layar Koneksi (Connect Screen)
| Tombol | Tindakan |
|--------|----------|
| `←` / `→` | Ganti tipe database (SQLite / PostgreSQL / MySQL) |
| `Tab` | Beralih fokus antara Form Input dan Daftar Koneksi Tersimpan |
| `j` / `k` | Navigasi field form atau daftar koneksi tersimpan |
| `Ctrl+S` | Simpan konfigurasi koneksi saat ini |
| `Delete` | Hapus koneksi tersimpan yang dipilih |
| `Enter` | Sambungkan / muat koneksi tersimpan |
| `Esc` | Kembali ke tampilan utama (jika tab sudah ada) |

### Panel Tabel (Kiri)
| Tombol | Tindakan |
|--------|----------|
| `j` / `k` | Navigasi daftar tabel |
| `Enter` | Buka dan tampilkan data tabel yang dipilih |
| `Tab` | Beralih fokus ke panel data |
| `Esc` | Kembali ke daftar database / schema |

### Panel Data (Kanan)
| Tombol | Tindakan |
|--------|----------|
| `j` / `k` | Gulir baris ke atas / ke bawah (auto-pagination) |
| `h` / `l` | Gulir kolom ke kiri / ke kanan |
| `Enter` / `e` | **Edit Sel Langsung (Inline Cell Edit)** pada sel yang dipilih |
| `/` | Cari dan filter baris secara real-time |
| `:` | Buka editor SQL query (bisa dari panel tabel maupun data) |
| `Ctrl+H` / `H` | Buka riwayat query SQL |
| `i` | Sisipkan baris baru (Insert row - diblokir dalam mode read-only) |
| `u` | Perbarui baris yang dipilih via form lengkap (Update row) |
| `d` | Hapus baris yang dipilih (membutuhkan Primary Key + konfirmasi) |
| `v` | Popup ekspor data (format CSV, JSON, atau SQL INSERT) |
| `I` | Impor file CSV ke tabel aktif (diblokir dalam mode read-only) |
| `y` | Salin nilai sel yang dipilih ke clipboard sistem |
| `Y` | Salin seluruh baris yang dipilih (dipisahkan tab) ke clipboard |
| `C` | Salin seluruh kolom yang dipilih (dipisahkan newline) ke clipboard |
| `s` | Tampilkan statistik tabel (daftar index dan perkiraan ukuran di disk) |
| `o` | Urutkan data berdasarkan kolom aktif (siklus: asc → desc → off via DB `ORDER BY`) |
| `PageUp` / `PageDown` | Pindah satu halaman penuh ke belakang / ke depan |
| `g` | Lompat langsung ke nomor baris tertentu |
| `Tab` | Beralih fokus kembali ke panel tabel |
| `q` / `Esc` | Kembali ke panel tabel |

### Input Editor SQL Query
| Tombol | Tindakan |
|--------|----------|
| `Enter` | Jalankan query (otomatis menyegarkan panel tabel bila menjalankan DDL `CREATE TABLE`, dll) |
| `←` / `→` | Gerakkan kursor ke kiri / ke kanan di dalam teks query |
| `Home` / `End` | Lompat ke awal / akhir teks query |
| `Backspace` / `Delete` | Hapus karakter sebelum / di bawah posisi kursor |
| `Ctrl+E` | Jalankan analisis rencana eksekusi query (Explain Query Plan) |
| `Ctrl+S` | Simpan query aktif sebagai Named Query |
| `Ctrl+N` | Buka daftar Named Queries tersimpan |
| `↑` / `↓` | Telusuri riwayat query sebelumnya |
| `Esc` | Batalkan dan tutup editor query |

### Riwayat Query (Query History)
| Tombol | Tindakan |
|--------|----------|
| `Ctrl+H` / `H` | Buka popup riwayat query |
| `j` / `k` | Navigasi daftar riwayat |
| `Enter` | Muat query terpilih ke dalam editor query untuk diedit |
| `r` | Jalankan ulang query yang dipilih secara langsung |
| `Esc` / `q` | Tutup popup riwayat |

### Form Insert / Update
| Tombol | Tindakan |
|--------|----------|
| `Tab` / `↑` / `↓` | Navigasi antar field |
| `←` / `→` | Pilih nilai relasi Foreign Key (pada kolom yang memiliki FK) |
| `F2` | Buka editor teks multiline mandiri untuk teks panjang |
| `Enter` | Simpan data ke database |
| `Esc` | Batalkan |

## Keamanan (Safety & Read-Only)

Gunakan flag `--read-only` / `-r` untuk mengaktifkan proteksi baca-saja tingkat engine:

- Koneksi SQLite otomatis menambahkan parameter `?mode=ro`.
- Shortcut operasi penulisan (Insert, Update, Delete) dan konfirmasi hapus otomatis diblokir.
- Query SQL kustom diperiksa oleh token scanner yang memblokir statement mutasi data, CTE penulisan (`WITH ... INSERT ... RETURNING`), injeksi multi-statement (`SELECT 1; DROP TABLE users;`), `SELECT ... INTO`, serta pragma mutasi (`PRAGMA ... = ...`).
- Pragma inspeksi seperti `PRAGMA table_info(...)` tetap diizinkan.

## Format Connection Strings

| Database | Format URL |
|----------|------------|
| SQLite | `./path/relatif.db` atau `/path/absolut/file.db` |
| PostgreSQL | `postgres://user:pass@host:5432/dbname` |
| MySQL | `mysql://user:pass@host:3306/dbname` |

## Fitur Unggulan

- Navigasi tabel dan data bergaya vim (`h/j/k/l`) yang responsif dan cepat
- Otomatis membuat file database SQLite jika file belum ada (`?mode=rwc`)
- Pembaruan mandiri via `db-eye update` dan notifikasi pembaruan non-blocking di latar belakang
- Pencarian dan filter baris real-time dengan penghitung jumlah data yang cocok
- Query SQL kustom dengan tampilan hasil tabel dan auto-refresh daftar tabel setelah DDL
- Navigasi kursor penuh di editor SQL query
- Durasi eksekusi query ditampilkan di status bar
- Riwayat query persisten yang tersimpan antar sesi (`~/.config/db-eye/history.json`)
- Operasi CRUD (Insert, Update, Delete) langsung dari panel data
- Dukungan composite primary key untuk operasi update dan delete
- Mode read-only untuk penjelajahan data yang aman
- Pesan error database informatif memetakan kode error SQLSTATE native (PostgreSQL, MySQL, SQLite)
- Validasi tipe data ketat, memisahkan tipe desimal presisi tinggi dari float
- Penanda nilai NULL khusus (`<NULL>`) agar berbeda dari teks string `"NULL"`
- Dropdown pemilih nilai Foreign Key (`←`/`→`) pada form data
- Editor teks multiline popup (`F2`) untuk field teks yang panjang
- Ekspor data ke CSV (dengan opsi delimiter/header), JSON, atau SQL INSERT
- Impor data dari file CSV dengan pemetaan kolom dan preview data
- Mendukung banyak koneksi sekaligus dalam tab terpisah
- Mendukung schema PostgreSQL non-public
- Penyesuaian lebar kolom otomatis
- Tampilan TUI monokrom kontras tinggi yang nyaman di semua terminal
- Tampilan views diberi label pembeda `(view)`
- Menampilkan panjang/presisi tipe data kolom (misal: `varchar(255)`, `numeric(10,2)`)
- Penanda kolom wajib isi (`*`) dan nilai default pada form input
- Popup statistik tabel (`s`): daftar index dan estimasi ukuran tabel di disk
- Sambung ulang koneksi terputus secara langsung (`Ctrl+R`) tanpa menutup tab

## Penomoran Versi (Versioning)

DB-Eye mengikuti standar [Semantic Versioning 2.0.0](https://semver.org/lang/id/):
- **MAJOR** (`X.0.0`): Perubahan besar yang tidak kompatibel ke belakang (*breaking changes*).
- **MINOR** (`0.X.0`): Fitur baru, dukungan database baru, atau shortcut baru (*backward-compatible*).
- **PATCH** (`0.0.X`): Perbaikan bug (*bugfixes*), patch keamanan, dan penyesuaian visual kecil.

Lihat [CHANGELOG.md](CHANGELOG.md) untuk riwayat rilis dan panduan lengkap.

## Lisensi

[MIT](LICENSE)
