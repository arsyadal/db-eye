# Changelog

All notable changes to DB-Eye are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

### Semantic Versioning Guidelines (SemVer)

Releases follow the **MAJOR.MINOR.PATCH** structure:

- **MAJOR (`X.0.0`)** — Incremented when introducing incompatible API or breaking changes:
  - Changes to CLI flags or arguments that break existing scripts or workflows.
  - Incompatible configuration, query history, or saved connection schema migrations.
  - Major architectural overhauls that fundamentally alter the application workflow.

- **MINOR (`0.X.0`)** — Incremented when adding new backward-compatible features:
  - New features, dialogs, or screens (e.g. export/import, persistence, auto-refresh schema, self-update).
  - Support for new database drivers or data types.
  - New keybindings or configuration options that preserve existing navigation.
  - Significant functional enhancements without breaking backward compatibility.

- **PATCH (`0.0.X`)** — Incremented when making backward-compatible bug fixes or adjustments:
  - Bug fixes, edge-case handling, or crash resolutions.
  - Security patches and dependency vulnerability updates.
  - Typo fixes, UI styling refinements, or status label tweaks.
  - Internal refactoring and performance optimizations without changing public behavior.

## [Unreleased]

## [0.3.0] - 2026-10-11

### Added

- Self-Update Command & Background Notification:
  - Added `db-eye update` CLI subcommand to automatically check, download, and replace the executable in-place using `self-replace`.
  - Added non-blocking background update check with 24-hour cache TTL (`~/.config/db-eye/update_check.json`) without startup latency.
  - Subtle update indicator on Connect screen and Help screen whenever a new release is available.
- Automated Multi-Platform Release CI/CD:
  - Added GitHub Actions workflow (`.github/workflows/release.yml`) triggered on version tags (`v*`).
  - Pre-built binary archives and standalone executables for Linux x86_64, macOS Apple Silicon, macOS Intel, and Windows x64 with SHA256 checksums.
- SQLite Auto-Creation & Read-Only Flags:
  - Automatically create non-existent SQLite files upon connection via URL normalization with `?mode=rwc`.
  - Read-only mode (`-r` / `--read-only`) applies `mode=ro` at the connection string level for engine-level write rejection.
  - Connect help text displays `(SQLite creates file if missing)`.
- Token-Level Read-Only SQL Scanner:
  - Multi-pass tokenizer stripping SQL comments (`--`, `/* */`), string literals (`'...'`), and quoted identifiers (`"..."`, ``` `...` ```).
  - Blocks mutating CTEs (`WITH ins AS (INSERT ...) SELECT ...`), multi-statement injection (`SELECT 1; DROP TABLE ...;`), `SELECT ... INTO`, and mutating pragmas (`PRAGMA ... = ...`).
  - Permits inspection pragmas such as `PRAGMA table_info(...)`.
- Exact Type Mapping & Decimal Separation:
  - Separated fixed-point decimal types (`decimal`, `numeric`, `dec`, `fixed`) from floating-point types (`real`, `double`, `float`).
  - Prevented false integer matching from `.contains("int")` on custom types/enums (`point`, `international_code`, `interval`).
  - High-precision decimal input validation without lossy conversion or casting to `f64`.
  - Removed `{:.4}` truncation when formatting table rows, rendering natural precision.
- Driver Error Code & SQLSTATE Classification:
  - Native driver error code classification (PostgreSQL SQLSTATE, MySQL error codes, SQLite extended result codes) before message string fallback.
- Flat Row Decoding Architecture:
  - Replaced 6–7 levels of nested closures in row decoding with a flat `decode_any_cell` helper across `row_cell`, `row_to_strings`, and `row_to_strings_checked`.
- Persistent Query History:
  - SQL query history persists across sessions in `~/.config/db-eye/history.json` (up to 100 entries).
- UX & Discoverability Refactor:
  - Cursor navigation in query editor (`Left`, `Right`, `Home`, `End`, `Delete`, `Backspace`, mid-string character insertion).
  - Schema auto-refresh: tables panel updates immediately after DDL execution (`CREATE TABLE`, etc.).
  - SQL query editor shortcut updated to `: (SQL query)` in help text for clarity.
  - Moved Query Editor and Query History shortcuts to the top `General` section in the Help screen (`?`).
- Export & Import Dialogs:
  - Export dialog (`v`) supporting CSV, JSON, and SQL INSERT formats with delimiter and header options.
  - CSV import dialog (`I` / `Shift+I`) with case-insensitive column matching, 5-row sample preview, and column mapping review.
- Explain Query Plan:
  - Added `EXPLAIN` (PostgreSQL/MySQL) and `EXPLAIN QUERY PLAN` (SQLite) via `Ctrl+E` in the SQL Query editor.
- Named Queries & Environment Password:
  - Save active query as a named query via `Ctrl+S`, browse and load named queries via `Ctrl+N` (persisted in `~/.config/db-eye/queries.json`).
  - Read default password from `DB_PASSWORD` environment variable when connecting.
- Clipboard Shortcuts:
  - Copy active cell value (`y`), copy active row tab-separated (`Y`), copy active column newline-separated (`C`).
- Inline Cell Editing:
  - Edit cells directly in the data panel with `Enter` or `e`, writing back via parameterized `UPDATE`.
- PostgreSQL Schema Support:
  - Schema selection screen after database selection with non-public schema browsing, CRUD, and metadata support.
- Richer Column Metadata & Navigation:
  - Required (`*`) and default-value indicators in CRUD forms.
  - Declared length and precision in column types (e.g. `varchar(255)`, `numeric(10,2)`).
  - Views listed alongside tables with a `(view)` badge.
  - Table stats popup (`s`): index list and on-disk size estimates.
  - Manual connection reconnect action (`Ctrl+R`).
  - Per-column sorting (`o`): cycles asc → desc → off as DB-side `ORDER BY`.
  - Pagination controls (`PageUp`/`PageDown`, `g` jump to row).
  - Multiline text editor popup (`F2`) for editing long values.

### Changed

- CRUD write operations (INSERT, UPDATE, DELETE) use bind placeholders rather than string interpolation.
- Query execution routes read queries to rows fetch and write queries to rows-affected execution.
- Refactored `classify_db_message` from a long `else if` chain into a declarative table lookup.

## [0.2.0] - 2026-06-01

### Added

- Basic row CRUD operations from data panel (insert, update, delete with confirmation).
- SQL preview on insert/update forms.
- Foreign-key value hints on insert/update forms.
- Installation documentation via `cargo install db-eye`.

### Changed

- Package version bumped to `0.2.0`.
- Data panel status bar displays CRUD keybindings.

## [0.1.0] - 2026-04-15

### Added

- SQLite, PostgreSQL, and MySQL database browsing.
- Interactive connection screen.
- Table list browser.
- Data panel with pagination and vim-style navigation.
- Real-time row filtering and search.
- Custom SQL query editor with inline results.
- CSV export.
- Multi-tab simultaneous connections.
