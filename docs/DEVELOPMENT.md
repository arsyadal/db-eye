# Development Guide

Dokumen ini menjadi aturan kerja supaya setiap pengembangan DB-Eye lebih terarah, bisa dilacak, dan mudah direview.

## Prinsip

- Setiap fitur harus punya tujuan jelas sebelum coding.
- Setiap perubahan penting harus tercatat.
- Setiap rilis harus punya changelog.
- Setiap keputusan teknis besar harus punya alasan.
- Production safety lebih penting daripada cepat menambah fitur.

## Alur Development

### 1. Plan

Sebelum implementasi, tulis ringkas:

- Masalah yang diselesaikan.
- Scope fitur.
- Non-scope.
- Risiko.
- Acceptance criteria.

Gunakan template:

- `docs/templates/FEATURE_SPEC.md`

### 2. Implement

Saat coding:

- Jaga perubahan tetap kecil dan fokus.
- Pisahkan fitur, refactor, dan docs jika memungkinkan.
- Hindari SQL manual untuk input user baru; prioritaskan parameterized query.
- Update dokumentasi bersamaan dengan perubahan fitur.

### 3. Validate

Minimal sebelum commit:

```bash
cargo fmt
cargo check
cargo clippy
```

Jika sudah ada test terkait:

```bash
cargo test
```

Untuk fitur database, validasi minimal di SQLite dulu, lalu PostgreSQL/MySQL jika fitur cross-database.

### 4. Document

Setiap perubahan wajib mengecek dokumen berikut:

- `README.md` — jika user-facing behavior berubah.
- `PRD.md` — jika roadmap/prioritas berubah.
- `CHANGELOG.md` — jika fitur/fix/perubahan layak masuk rilis.
- `docs/DEVLOG.md` — catatan development harian/perubahan besar.
- ADR baru — jika ada keputusan arsitektur signifikan.

### 5. Commit

Gunakan Conventional Commits:

- `feat:` fitur baru
- `fix:` bug fix
- `docs:` dokumentasi
- `refactor:` perubahan struktur tanpa behavior baru
- `test:` tambah/ubah test
- `chore:` maintenance

Contoh:

```text
feat: add parameterized row updates
fix: handle null values in CRUD forms
docs: update production roadmap
```

### 6. Release

Sebelum release:

- Jalankan checklist di `docs/templates/RELEASE_CHECKLIST.md`.
- Pastikan `CHANGELOG.md` diperbarui.
- Pastikan version `Cargo.toml` sesuai.
- Buat git tag.

## Definition of Done

Sebuah task dianggap selesai jika:

- Acceptance criteria terpenuhi.
- `cargo fmt` sukses.
- `cargo check` sukses.
- `cargo clippy` tidak punya error baru.
- Test terkait ditambah/diperbarui jika memungkinkan.
- README/PRD/CHANGELOG/DEVLOG diperbarui jika relevan.
- Perubahan sudah dicommit dengan pesan jelas.

## Dokumentasi Keputusan Teknis

Untuk keputusan besar, buat ADR di:

```text
docs/adr/YYYY-MM-DD-short-title.md
```

Contoh keputusan yang perlu ADR:

- Strategi parameterized query lintas SQLite/PostgreSQL/MySQL.
- Cara mendukung composite primary key.
- Format config saved connections.
- Read-only mode dan safety guard.

Format ADR:

```md
# ADR: Judul

## Status
Accepted / Proposed / Superseded

## Context
Masalah dan constraint.

## Decision
Keputusan yang diambil.

## Consequences
Dampak positif/negatif.
```

## Prioritas Development Saat Ini

Ikuti `PRD.md`:

1. P0 dulu untuk production readiness.
2. P1 setelah safety dan test kuat.
3. P2 setelah core stabil.
