# Feature Spec — <Nama Fitur>

## Summary

Deskripsi singkat fitur.

## Problem

Masalah user yang ingin diselesaikan.

## Goals

- Goal 1
- Goal 2

## Non-Goals

- Hal yang sengaja tidak dikerjakan.

## User Flow

1. User melakukan ...
2. Sistem menampilkan ...
3. User mengonfirmasi ...

## Technical Plan

- File yang kemungkinan berubah:
  - `src/app.rs`
  - `src/db.rs`
  - `src/ui.rs`
- Perubahan data/state:
- Query/database impact:

## Safety Considerations

- Risiko SQL injection:
- Risiko data loss:
- Permission/read-only behavior:
- Error handling:

## Acceptance Criteria

- [ ] Kriteria 1
- [ ] Kriteria 2
- [ ] Dokumentasi diperbarui
- [ ] Validasi command sukses

## Test Plan

```bash
cargo fmt
cargo check
cargo clippy
cargo test
```

Manual test:

- [ ] SQLite
- [ ] PostgreSQL
- [ ] MySQL

## Notes

Catatan tambahan.
