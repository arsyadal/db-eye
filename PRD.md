# PRD — DB-Eye Production Roadmap

## Current Status

DB-Eye is already usable as a terminal UI database browser for SQLite, PostgreSQL, and MySQL. Basic features are available: database connections, table browsing, search, custom queries, CSV export, multi-tab, and simple row CRUD.

The goal of this document is to list the features that still need to be implemented, or could be built out, for DB-Eye to be production-grade.

## Product Goals

- Be a lightweight, fast, and safe TUI database browser.
- Support the daily workflow of developers/DBAs for inspecting, querying, and editing data.
- Be safe to use against local databases as well as development/staging/production servers.
- Be easy to install via `cargo install db-eye`.

## Priority P0 — Required for Production

### 1. Query and CRUD Security

- Use parameterized queries/bind values for insert, update, delete.
- Avoid manually building SQL values from user input.
- Validate table/column identifiers against database metadata.
- Add a read-only mode for production connections.
- Add extra confirmation for destructive operations:
  - delete row
  - update many rows
  - drop/truncate if a custom query runs a destructive statement

### 2. Automated Testing

- SQLite integration tests for:
  - connect
  - list table
  - query table
  - search/filter
  - insert/update/delete
  - export CSV
- Test SQL quoting for table/column names with spaces or reserved keywords.
- Test error handling for constraint violations.
- Test multi-tab state isolation.
- Set up GitHub Actions CI:
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo test`
  - build release

### 3. Schema and Primary Key Handling

- PostgreSQL: support schemas other than `public`.
- MySQL: support richer database/table metadata. ✅ initial implementation (nullable/default, type precision, views, index/size stats)
- Support composite primary keys for update/delete.
- If a table has no primary key:
  - show a warning
  - disable update/delete, or
  - use a safe row-identity strategy where possible

### 4. Data Type Handling

- Detect column data type from metadata. ✅ initial implementation
- Input forms must handle:
  - string
  - integer ✅ initial validation
  - float/decimal ✅ initial validation
  - boolean ✅ initial validation
  - date/time/timestamp
  - NULL ✅ initial implementation
  - blob/binary as read-only/placeholder ✅ initial validation
- Don't treat all input as strings.
- Display NULL values distinctly from the string `"NULL"`. ✅ initial implementation

### 5. Error Handling and Recovery ✅

- User-friendly error messages for:
  - connection failed
  - permission denied
  - foreign key violation
  - unique constraint violation
  - invalid SQL
  - network timeout
- Don't crash when a query fails or the connection drops. ✅ all DB calls are already `Result`-based, no `.unwrap()` on any production path
- Add a reconnect action. ✅ `Ctrl+R` — reconnects the active tab using its stored `reconnect_url`, then reloads the schema/table list

## Priority P1 — Strongly Recommended

### 1. Better CRUD UX

- Inline cell edit from the data panel.
- Insert/update form with:
  - required/not-null indicator
  - default value indicator
  - FK dropdown/select, not just a hint ✅ `←`/`→` cycles known values into the field
  - reset field
  - clear-to-NULL shortcut
- Correctly support multi-character paste.
- Support horizontal scroll in the form for long text.
- Support a multiline text editor popup. ✅ `F2` on an active field, own cursor/insert/delete/line-navigation

### 2. Query Experience

- Query history per tab.
- Save named queries.
- SQL syntax highlighting.
- Run the selected statement if the input contains multiple statements.
- Show rows affected for write queries.
- Explain query plan:
  - SQLite `EXPLAIN QUERY PLAN`
  - PostgreSQL `EXPLAIN`
  - MySQL `EXPLAIN`

### 3. Data Browsing

- Sorting per column.
- Filter per column.
- More explicit pagination:
  - next page
  - previous page
  - jump to offset/page
- Show total filtered rows where possible.
- Column hide/show.
- Copy cell/row/column value to clipboard.

### 4. Connection Management

- Saved connections/config file.
- Password via environment variable or secure prompt.
- Manual connection string input. ✅ initial implementation
- SSH tunnel support.
- TLS options for PostgreSQL/MySQL.
- Recent connections list.

### 5. Export/Import

- Export CSV with delimiter/header options.
- Export JSON.
- Export SQL insert statements.
- Import CSV into an existing table.
- Preview before import.

## Priority P2 — Nice to Have

### 1. Visual and Navigation

- Theme support.
- Help screen `?` covering all keybindings.
- Command palette.
- Optional mouse support.
- More informative status bar:
  - active database
  - active schema
  - read/write mode
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
- Lightweight schema diff between databases.

### 4. Packaging and Distribution

- Release binaries for macOS/Linux/Windows.
- Homebrew formula.
- AUR package.
- Optional Docker image.
- Demo GIF and screenshots in README.

## Current Non-Goals

- Becoming a full database administration suite like DataGrip/DBeaver.
- A complex visual query builder.
- An ORM/migration framework.
- Production write operations without a guard/read-only mode.

## Development Documentation Standard

Every piece of development must be documented so the product direction stays clear:

- New features start from a feature spec using `docs/templates/FEATURE_SPEC.md`.
- User-facing changes must update `README.md`.
- Roadmap/priority changes must update `PRD.md`.
- Changes going into a release must be noted in `CHANGELOG.md`.
- Large changes/technical debt must be noted in `docs/DEVLOG.md`.
- Significant architecture decisions must be written as an ADR in `docs/adr/`.
- Releases must follow `docs/templates/RELEASE_CHECKLIST.md`.

## Production v1.0 Acceptance Criteria

DB-Eye can be considered production-ready when:

- All P0 items are done.
- CI is green for fmt, clippy, test, and release build. ✅ initial implementation
- CRUD uses parameterized queries.
- Update/delete are safe for both single-column and composite primary keys. ✅ initial implementation
- Non-public PostgreSQL schemas are supported.
- A read-only mode exists.
- Error handling doesn't cause crashes in common scenarios.
- README is complete with install, usage, keybindings, safety notes, and screenshots/demo.
- A release tag and changelog are available.

## Release Roadmap

### v0.3

- Parameterized CRUD. ✅ initial implementation
- SQLite integration tests. ✅ initial CRUD coverage
- Read-only mode. ✅ initial implementation
- Better error messages. ✅ initial implementation
- Data type and NULL handling. ✅ initial implementation

### v0.4

- PostgreSQL schema support.
- Composite primary key support. ✅ initial implementation
- Query history.
- Rows affected for write queries.

### v0.5

- Inline cell edit.
- FK dropdown.
- Saved connections.
- Help screen.

### v1.0

- Full P0 complete.
- CI/release pipeline. ✅ CI initial implementation
- Production safety polish.
- Complete documentation.
