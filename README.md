# DB-Eye

<img src="logo.svg" alt="DB-Eye Logo" width="360" />

**English** | [Bahasa Indonesia](README.id.md)

[![crates.io](https://img.shields.io/crates/v/db-eye.svg)](https://crates.io/crates/db-eye)
[![CI](https://github.com/arsyadal/db-eye/actions/workflows/ci.yml/badge.svg)](https://github.com/arsyadal/db-eye/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Fast, lightweight Terminal UI database browser for SQLite, PostgreSQL, and MySQL.

![DB-Eye demo](docs/demo.gif)
## Install

Install globally via Cargo:

```bash
cargo install db-eye
```

Requires a Rust toolchain supporting edition 2024 (rustc 1.85+). Runs on macOS, Linux, and Windows.

Or download pre-compiled standalone binaries from [GitHub Releases](https://github.com/arsyadal/db-eye/releases).

To update an existing installation to the latest version:

```bash
db-eye update
# or: cargo install --force db-eye
```

Or run from source:

```bash
cargo run -- ./path/to/file.db
```
## Usage

```bash
# Open SQLite file directly (creates file automatically if missing)
db-eye ./mydb.sqlite

# Open in read-only mode (enforces hardware/engine-level rejection of writes)
db-eye --read-only ./mydb.sqlite

# Launch without args → connect screen
db-eye

# Check for and install updates
db-eye update

# Show version
db-eye --version

# Show usage help
db-eye --help
```
## Connect Screen

| Key | Action |
|-----|--------|
| `←` / `→` | Switch DB type (SQLite / PostgreSQL / MySQL) |
| `Tab` / `j` / `k` | Move between form fields |
| `Enter` | Connect |
| `Esc` | Back to main (if tab open) |

**SQLite** — enter file path, press Enter.

**PostgreSQL / MySQL** — fill Host, Port, User, Password → Enter → pick database from list.

Or paste a full connection URL in **Connection URL (optional)** and press Enter to connect directly:

- `postgres://user:pass@host:5432/dbname`
- `mysql://user:pass@host:3306/dbname`

You can also set the `DB_PASSWORD` environment variable to automatically supply the password for connections:

```bash
DB_PASSWORD=secret db-eye postgres://user@host:5432/dbname
```

Saved connections (`Ctrl+S`) persist to a local JSON file — passwords are never saved:

- macOS/Linux: `~/.config/db-eye/connections.json`
- Windows: `%APPDATA%\db-eye\connections.json`

## Navigation

### Global
| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit |
| `?` | Show / hide help popup |
| `Ctrl+T` | New connection (Connect screen) |
| `Ctrl+W` | Close current tab |
| `Ctrl+R` | Reconnect current tab (recovers a dropped connection) |
| `[` / `]` | Previous / Next tab |

### Connect Screen
| Key | Action |
|-----|--------|
| `←` / `→` | Switch DB type (SQLite / PostgreSQL / MySQL) |
| `Tab` | Switch focus between Form and Saved Connections |
| `j` / `k` | Move between form fields or saved items |
| `Ctrl+S` | Save current connection configuration |
| `Delete` | Delete selected saved connection |
| `Enter` | Connect / Load saved connection |
| `Esc` | Back to main (if tab open) |

### Tables Panel (left)
| Key | Action |
|-----|--------|
| `j` / `k` | Navigate tables |
| `Enter` | Open table |
| `Tab` | Switch focus to data panel |
| `Esc` | Back to database / schema list |

### Data Panel (right)
| Key | Action |
|-----|--------|
| `j` / `k` | Scroll rows (auto-pagination) |
| `h` / `l` | Scroll columns left / right |
| `Enter` / `e` | **Inline Cell Edit** (starts editing selected cell) |
| `/` | Search / filter rows (real-time) |
| `:` | Open SQL query editor (works from the tables or data panel; supports history with `↑`/`↓`) |
| `Ctrl+H` / `H` | Open query history |
| `i` | Insert row (disabled in read-only mode) |
| `u` | Update selected row (full form) |
| `d` | Delete selected row (requires PK + confirmation) |
| `v` | Export data popup (CSV, JSON, SQL INSERT) |
| `I` | Import CSV file into active table (disabled in read-only mode) |
| `y` | Copy selected cell value to clipboard |
| `Y` | Copy selected row (tab-separated) to clipboard |
| `C` | Copy selected column (newline-separated) to clipboard |
| `s` | Show table stats (indexes, approximate size) |
| `o` | Sort by column under cursor (cycles asc → desc → off) |
| `PageUp` / `PageDown` | Previous / next page |
| `g` | Jump to row number |
| `Tab` | Switch focus to tables panel |
| `q` / `Esc` | Back to tables panel |

### SQL Query Input
| Key | Action |
|-----|--------|
| `Enter` | Run query (auto-refreshes schema if DDL like `CREATE TABLE` is executed) |
| `←` / `→` | Move cursor within query text |
| `Home` / `End` | Jump to start / end of query |
| `Backspace` / `Delete` | Delete character before / under cursor |
| `Ctrl+E` | Explain query plan |
| `Ctrl+S` | Save query as a Named Query |
| `Ctrl+N` | Open Saved Named Queries list |
| `↑` / `↓` | Cycle query history |
| `Esc` | Close input |
### Query History
| Key | Action |
|-----|--------|
| `Ctrl+H` / `H` | Open query history |
| `j` / `k` | Navigate history |
| `Enter` | Load selected query into query input |
| `r` | Re-run selected query |
| `Esc` / `q` | Close history |

### Insert/Update Form
| Key | Action |
|-----|--------|
| `Tab` / `↑` / `↓` | Navigate fields |
| `←` / `→` | Pick a value on FK fields |
| `F2` | Open multiline text editor for the active field |
| `Enter` | Save row |
| `Esc` | Cancel |

## Safety

Use `--read-only` / `-r` to enforce hardware- and engine-level read-only protection. In read-only mode:

- SQLite connections automatically enforce `?mode=ro`.
- Insert, update, and delete shortcuts and confirmation screens are blocked.
- Custom SQL is inspected by a robust token scanner blocking mutating statements, write CTEs (`WITH ... INSERT ... RETURNING`), multi-statement injection (`SELECT 1; DROP TABLE users;`), `SELECT ... INTO`, and mutating pragmas (`PRAGMA ... = ...`).
- Safe inspection pragmas like `PRAGMA table_info(...)` remain permitted.
## Connection Strings

| DB | Format |
|----|--------|
| SQLite | `./relative/path.db` or `/abs/path.db` |
| PostgreSQL | `postgres://user:pass@host:5432/dbname` |
| MySQL | `mysql://user:pass@host:3306/dbname` |

## Features

- Browse tables and data with vim-style navigation
- Automatic SQLite database creation if the file does not exist
- Self-update via `db-eye update` and non-blocking background update notifications
- Real-time search across all columns with match count
- Custom SQL queries with results displayed inline and schema auto-refresh
- SQL query editor cursor navigation and full inline editing
- Query execution duration in status bar
- Persistent query history across sessions (`~/.config/db-eye/history.json`)
- Insert, update, and delete rows from the data panel
- Composite primary key support for update/delete
- Read-only mode for safer browsing with hardware-level rejection
- Friendly database error descriptions mapping native SQLSTATE and driver error codes
- Data-type validation for CRUD and inline edits, separating exact decimal from float
- Distinct NULL display (`<NULL>`) so string `NULL` remains editable as text
- CRUD/edit forms treat empty input or `\\null` as database NULL
- Foreign-key dropdown (`←`/`→` to pick a value) in insert/update forms
- Multiline text editor popup (`F2`) for long field values
- Export current view to CSV (with delimiter/header options), JSON, or SQL INSERT statements
- Import CSV into an existing table with column mapping and data preview
- Multiple simultaneous DB connections (tabs)
- PostgreSQL/MySQL: connect via form flow or full connection URL
- PostgreSQL: connect to server → pick database from list → schema selector
- Auto column width fitting
- Monochrome TUI — works in any terminal
- Views listed alongside tables, labeled `(view)`
- Column type shown with declared length/precision (e.g. `varchar(255)`, `numeric(10,2)`)
- Required and default-value indicators on insert/update form fields
- Table stats popup (`s`): index list and approximate on-disk size
- Manual connection reconnect (`Ctrl+R`) recovering dropped connections in place
## Versioning

DB-Eye adheres to [Semantic Versioning 2.0.0](https://semver.org/):
- **MAJOR** (`X.0.0`): Incompatible changes or breaking CLI/config updates.
- **MINOR** (`0.X.0`): New backward-compatible features, database support, or shortcuts.
- **PATCH** (`0.0.X`): Bug fixes, security patches, and minor UI refinements.

See [CHANGELOG.md](CHANGELOG.md) for release notes and full versioning guidelines.

## License

[MIT](LICENSE)
