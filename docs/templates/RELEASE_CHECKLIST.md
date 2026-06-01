# Release Checklist — vX.Y.Z

## Pre-release

- [ ] Semua P0 blocker untuk rilis ini selesai.
- [ ] `Cargo.toml` version sudah benar.
- [ ] `Cargo.lock` sudah diperbarui.
- [ ] `README.md` sesuai fitur terbaru.
- [ ] `PRD.md`/roadmap diperbarui jika ada perubahan arah.
- [ ] `CHANGELOG.md` diperbarui.
- [ ] `docs/DEVLOG.md` diperbarui.

## Validation

```bash
cargo fmt --check
cargo check
cargo clippy
cargo test
cargo build --release
```

- [ ] SQLite manual smoke test.
- [ ] PostgreSQL manual smoke test jika fitur menyentuh PostgreSQL.
- [ ] MySQL manual smoke test jika fitur menyentuh MySQL.
- [ ] CRUD destructive actions sudah dikonfirmasi aman.

## Git

- [ ] Working tree bersih.
- [ ] Commit message jelas.
- [ ] Tag dibuat.

```bash
git tag vX.Y.Z
git push origin master --tags
```

## Publish

- [ ] Publish crates.io jika applicable.
- [ ] GitHub release dibuat.
- [ ] Release notes berisi fitur, fix, breaking changes, dan known issues.

## Post-release

- [ ] Install test via `cargo install db-eye`.
- [ ] Buka issue untuk known limitations berikutnya.
