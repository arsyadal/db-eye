# DB-Eye

Terminal UI database browser. SQLite, PostgreSQL, MySQL.

## Install

```bash
cargo install db-eye
```

Or run from source:

```bash
cargo run -- ./path/to/file.db
```

## Usage

```bash
# Open SQLite file directly
db-eye ./mydb.sqlite

# Open in read-only mode (disables writes/CRUD)
db-eye --read-only ./mydb.sqlite

# Launch without args → connect screen
db-eye
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

## Navigation

### Global
| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit |
| `?` | Show / hide help popup |
| `Ctrl+T` | New connection (Connect screen) |
| `Ctrl+W` | Close current tab |
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
| `:` | Enter SQL query (supports history with `↑`/`↓`) |
| `Ctrl+H` / `H` | Open query history |
| `i` | Insert row (disabled in read-only mode) |
| `u` | Update selected row (full form) |
| `d` | Delete selected row (requires PK + confirmation) |
| `v` | Export visible data to CSV (mnemonic: view/export) |
| `Tab` | Switch focus to tables panel |
| `q` / `Esc` | Back to tables panel |

### Query History
| Key | Action |
|-----|--------|
| `Ctrl+H` / `H` | Open query history |
| `j` / `k` | Navigate history |
| `Enter` | Load selected query into query input |
| `r` | Re-run selected query |
| `Esc` / `q` | Close history |

## Safety

Use `--read-only` / `-r` to disable write operations. In read-only mode:

- Insert/update/delete shortcuts are blocked.
- Insert/update/delete confirmation screens cannot execute.
- Custom SQL is limited to read-style statements such as `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, and `DESCRIBE`.

## Connection Strings

| DB | Format |
|----|--------|
| SQLite | `./relative/path.db` or `/abs/path.db` |
| PostgreSQL | `postgres://user:pass@host:5432/dbname` |
| MySQL | `mysql://user:pass@host:3306/dbname` |

## Features

- Browse tables and data with vim-style navigation
- Real-time search across all columns
- Custom SQL queries with results displayed inline
- Query execution duration in status bar
- In-memory query history with edit/re-run actions
- Insert, update, and delete rows from the data panel
- Composite primary key support for update/delete
- Read-only mode for safer browsing
- Friendly database errors for constraints, permissions, and SQL syntax
- Foreign-key value hints in insert/update forms
- Export query results to CSV
- Multiple simultaneous DB connections (tabs)
- PostgreSQL: connect to server → pick database from list
- Auto column width fitting
- Monochrome TUI — works in any terminal

## Development Docs

- `PRD.md` — production roadmap and v1.0 acceptance criteria
- `CHANGELOG.md` — release notes and known limitations
- `docs/DEVELOPMENT.md` — development workflow and definition of done
- `docs/DEVLOG.md` — chronological development notes
- `docs/templates/` — feature spec and release checklist templates
