use sqlx::{AnyPool, Column, Row, any::AnyRow, error::ErrorKind};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DbType {
    Sqlite,
    Postgres,
    Mysql,
    Mssql,
}

impl DbType {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("postgres") {
            DbType::Postgres
        } else if url.starts_with("mysql") {
            DbType::Mysql
        } else if url.starts_with("mssql") || url.starts_with("sqlserver") {
            DbType::Mssql
        } else {
            DbType::Sqlite
        }
    }
}

pub type MssqlStream = Compat<TcpStream>;
pub type MssqlClient = Arc<Mutex<tiberius::Client<MssqlStream>>>;

#[derive(Clone)]
pub enum DbBackend {
    Sqlx(AnyPool),
    Mssql {
        client: MssqlClient,
        #[allow(dead_code)]
        config: Box<tiberius::Config>,
    },
}

#[derive(Clone)]
pub struct DbClient {
    pub backend: DbBackend,
    pub db_type: DbType,
}

#[derive(Debug)]
pub enum DbError {
    Sqlx(sqlx::Error),
    Mssql(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Sqlx(e)
    }
}

impl From<String> for DbError {
    fn from(s: String) -> Self {
        DbError::Mssql(s)
    }
}

impl From<&str> for DbError {
    fn from(s: &str) -> Self {
        DbError::Mssql(s.to_string())
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sqlx(e) => write!(f, "{e}"),
            DbError::Mssql(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DbError {}

pub fn format_db_error(action: &str, error: &DbError) -> String {
    let prefix = if action.is_empty() {
        "Database error".to_string()
    } else {
        format!("{} failed", action)
    };

    match error {
        DbError::Sqlx(sqlx_err) => format_sqlx_error(&prefix, sqlx_err),
        DbError::Mssql(msg) => classify_db_message(&prefix, msg),
    }
}

fn format_sqlx_error(prefix: &str, error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(db_error) => {
            let constraint = db_error
                .constraint()
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();
            match db_error.kind() {
                ErrorKind::UniqueViolation => {
                    format!("{prefix}: duplicate value violates a unique constraint{constraint}")
                }
                ErrorKind::ForeignKeyViolation => format!(
                    "{prefix}: foreign key constraint failed{constraint}; check referenced rows first"
                ),
                ErrorKind::NotNullViolation => {
                    format!("{prefix}: required field cannot be NULL{constraint}")
                }
                ErrorKind::CheckViolation => {
                    format!("{prefix}: value violates a check constraint{constraint}")
                }
                _ => {
                    if let Some(code) = db_error.code()
                        && let Some(summary) = classify_db_code(code.as_ref())
                    {
                        format!("{prefix}: {summary}{constraint} ({})", db_error.message())
                    } else {
                        classify_db_message(prefix, db_error.message())
                    }
                }
            }
        }
        sqlx::Error::Io(_) => format!("{prefix}: database connection error ({error})"),
        sqlx::Error::PoolTimedOut => format!("{prefix}: database connection timed out"),
        sqlx::Error::PoolClosed => format!("{prefix}: database connection is closed"),
        sqlx::Error::RowNotFound => format!("{prefix}: no matching row found"),
        sqlx::Error::ColumnNotFound(column) => format!("{prefix}: column not found: {column}"),
        sqlx::Error::ColumnIndexOutOfBounds { index, len } => {
            format!("{prefix}: column index {index} out of bounds (columns: {len})")
        }
        sqlx::Error::Configuration(_) => {
            format!("{prefix}: invalid database configuration ({error})")
        }
        sqlx::Error::Tls(_) => format!("{prefix}: TLS connection failed ({error})"),
        _ => classify_db_message(prefix, &error.to_string()),
    }
}

pub(crate) fn classify_db_code(code: &str) -> Option<&'static str> {
    match code {
        // PostgreSQL SQLSTATE codes
        "23505" => Some("duplicate value violates a unique constraint"),
        "23503" => Some("foreign key constraint failed; check referenced rows first"),
        "23502" => Some("required field cannot be NULL"),
        "23514" => Some("value violates a check constraint"),
        "42P01" => Some("table or database object not found"),
        "42703" => Some("column not found"),
        "42601" => Some("SQL syntax error; check the query text"),
        "25006" => Some("permission denied or database is read-only"),
        "28000" | "28P01" => Some("could not connect to database server (authentication failed)"),

        // MySQL error codes
        "1062" => Some("duplicate value violates a unique constraint"),
        "1452" => Some("foreign key constraint failed; check referenced rows first"),
        "1048" => Some("required field cannot be NULL"),
        "1146" => Some("table or database object not found"),
        "1054" => Some("column not found"),
        "1064" => Some("SQL syntax error; check the query text"),
        "1045" => Some("could not connect to database server (authentication failed)"),

        // SQLite error codes
        "1555" | "2067" => Some("duplicate value violates a unique constraint"),
        "787" => Some("foreign key constraint failed; check referenced rows first"),
        "1299" => Some("required field cannot be NULL"),
        "1" => Some("SQL syntax error; check the query text"),
        "8" => Some("permission denied or database is read-only"),

        _ => None,
    }
}

const MESSAGE_PATTERNS: &[(&[&str], &str)] = &[
    (
        &["foreign key"],
        "foreign key constraint failed; check referenced rows first",
    ),
    (
        &["unique", "duplicate", "1062"],
        "duplicate value violates a unique constraint",
    ),
    (
        &["not null", "cannot be null"],
        "required field cannot be NULL",
    ),
    (
        &[
            "permission denied",
            "access denied",
            "readonly",
            "read-only",
            "attempt to write a readonly database",
        ],
        "permission denied or database is read-only",
    ),
    (
        &["syntax error", "sql syntax"],
        "SQL syntax error; check the query text",
    ),
    (
        &[
            "no such table",
            "doesn't exist",
            "does not exist",
            "unknown table",
        ],
        "table or database object not found",
    ),
    (&["no such column", "unknown column"], "column not found"),
    (
        &["connection refused", "could not connect"],
        "could not connect to database server",
    ),
];

fn classify_db_message(prefix: &str, message: &str) -> String {
    let lower = message.to_lowercase();
    let friendly = MESSAGE_PATTERNS.iter().find_map(|(patterns, summary)| {
        if patterns.iter().any(|p| lower.contains(p)) {
            Some(*summary)
        } else {
            None
        }
    });

    match friendly {
        Some(summary) => format!("{prefix}: {summary} ({message})"),
        None => format!("{prefix}: {message}"),
    }
}

pub(crate) fn normalize_sqlite_url(url: &str) -> String {
    if url == ":memory:" || url == "sqlite::memory:" {
        url.to_string()
    } else if let Some(stripped) = url.strip_prefix("sqlite:") {
        if stripped.contains("mode=") {
            url.to_string()
        } else if stripped.contains('?') {
            format!("{url}&mode=rwc")
        } else {
            format!("{url}?mode=rwc")
        }
    } else if url.contains("mode=") {
        format!("sqlite:{url}")
    } else if url.contains('?') {
        format!("sqlite:{url}&mode=rwc")
    } else {
        format!("sqlite:{url}?mode=rwc")
    }
}

pub fn parse_mssql_url(url: &str) -> Result<tiberius::Config, String> {
    let stripped = url
        .strip_prefix("mssql://")
        .or_else(|| url.strip_prefix("sqlserver://"))
        .unwrap_or(url);

    let (auth_host, database) = match stripped.split_once('/') {
        Some((ah, db)) => (ah, db),
        None => (stripped, "master"),
    };

    let (auth, host_port) = match auth_host.rsplit_once('@') {
        Some((a, hp)) => (Some(a), hp),
        None => (None, auth_host),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(1433)),
        None => (host_port, 1433),
    };

    let mut config = tiberius::Config::new();
    config.host(host);
    config.port(port);

    if let Some(auth_str) = auth {
        let (user, pass) = auth_str.split_once(':').unwrap_or((auth_str, ""));
        config.authentication(tiberius::AuthMethod::sql_server(user, pass));
    }

    if !database.is_empty() {
        config.database(database);
    }
    config.trust_cert();

    Ok(config)
}

fn decode_tiberius_cell(row: &tiberius::Row, idx: usize) -> String {
    if let Ok(Some(s)) = row.try_get::<&str, _>(idx) {
        return s.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<i32, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<i16, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<u8, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<f64, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<f32, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<bool, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<&[u8], _>(idx) {
        return format!("<blob {}b>", v.len());
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveDateTime, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveDate, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveTime, _>(idx) {
        return v.to_string();
    }
    NULL_DISPLAY.to_string()
}

impl DbClient {
    pub fn pool(&self) -> &AnyPool {
        match &self.backend {
            DbBackend::Sqlx(p) => p,
            _ => panic!("Expected Sqlx pool"),
        }
    }

    #[allow(dead_code)]
    pub async fn close(&self) {
        if let DbBackend::Sqlx(p) = &self.backend {
            p.close().await;
        }
    }

    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let db_type = DbType::from_url(url);
        match db_type {
            DbType::Mssql => {
                let config = parse_mssql_url(url).map_err(DbError::Mssql)?;
                let tcp = TcpStream::connect(config.get_addr()).await.map_err(|e| {
                    DbError::Mssql(format!(
                        "Failed to connect to MS SQL at {}: {e}",
                        config.get_addr()
                    ))
                })?;
                tcp.set_nodelay(true)
                    .map_err(|e| DbError::Mssql(e.to_string()))?;
                let client = tiberius::Client::connect(config.clone(), tcp.compat_write())
                    .await
                    .map_err(|e| DbError::Mssql(format!("MS SQL login failed: {e}")))?;
                let mssql_client = Arc::new(Mutex::new(client));
                Ok(Self {
                    backend: DbBackend::Mssql {
                        client: mssql_client,
                        config: Box::new(config),
                    },
                    db_type,
                })
            }
            _ => {
                sqlx::any::install_default_drivers();
                let normalized = match db_type {
                    DbType::Sqlite => {
                        if url != ":memory:" && url != "sqlite::memory:" && !url.contains("mode=ro")
                        {
                            let fs_path = url.strip_prefix("sqlite:").unwrap_or(url);
                            let fs_path = fs_path.split('?').next().unwrap_or(fs_path);
                            let path_obj = std::path::Path::new(fs_path);
                            if let Some(parent) = path_obj.parent()
                                && !parent.as_os_str().is_empty()
                            {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if !path_obj.exists() {
                                let _ = std::fs::File::create(path_obj);
                            }
                        }
                        normalize_sqlite_url(url)
                    }
                    _ => url.to_string(),
                };
                let pool = AnyPool::connect(&normalized).await?;
                Ok(Self {
                    backend: DbBackend::Sqlx(pool),
                    db_type,
                })
            }
        }
    }

    pub fn identifier_quote(&self) -> char {
        match self.db_type {
            DbType::Mysql => '`',
            DbType::Mssql => '[',
            _ => '"',
        }
    }

    pub fn quote_ident(&self, ident: &str) -> String {
        let quote = self.identifier_quote();
        if quote == '[' {
            format!("[{}]", ident.replace(']', "]]"))
        } else {
            let doubled = format!("{quote}{quote}");
            format!("{quote}{}{quote}", ident.replace(quote, &doubled))
        }
    }

    pub fn uses_numbered_placeholders(&self) -> bool {
        self.db_type == DbType::Postgres || self.db_type == DbType::Mssql
    }

    pub fn placeholder(&self, index: usize) -> String {
        match self.db_type {
            DbType::Postgres => format!("${index}"),
            DbType::Mssql => format!("@P{index}"),
            _ => "?".to_string(),
        }
    }

    pub async fn list_databases(&self) -> Result<Vec<String>, DbError> {
        match &self.backend {
            DbBackend::Sqlx(pool) => {
                let sql = match self.db_type {
                    DbType::Postgres => {
                        "SELECT datname::text FROM pg_database WHERE datistemplate = false ORDER BY datname"
                    }
                    DbType::Mysql => "SHOW DATABASES",
                    DbType::Sqlite => "SELECT 'main'",
                    _ => "SELECT 'main'",
                };
                let rows = sqlx::query(sql).fetch_all(pool).await?;
                Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
            }
            DbBackend::Mssql { .. } => {
                let sql = "SELECT name FROM sys.databases WHERE state = 0 AND name NOT IN ('master', 'tempdb', 'model', 'msdb') ORDER BY name";
                let res = self.execute_query(sql).await?;
                Ok(res.rows.into_iter().filter_map(|mut r| r.pop()).collect())
            }
        }
    }

    pub async fn list_schemas(&self) -> Result<Vec<String>, DbError> {
        match &self.backend {
            DbBackend::Sqlx(pool) => match self.db_type {
                DbType::Postgres => {
                    let sql = "SELECT schema_name::text FROM information_schema.schemata \
                                   WHERE schema_name NOT LIKE 'pg_%' AND schema_name != 'information_schema' \
                                   ORDER BY schema_name";
                    let rows = sqlx::query(sql).fetch_all(pool).await?;
                    Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
                }
                _ => Ok(vec![]),
            },
            DbBackend::Mssql { .. } => {
                let sql = "SELECT schema_name FROM information_schema.schemata \
                           WHERE schema_name NOT IN ('guest', 'INFORMATION_SCHEMA', 'sys', 'db_owner', 'db_accessadmin', 'db_securityadmin', 'db_ddladmin', 'db_backupoperator', 'db_datareader', 'db_datawriter', 'db_denydatareader', 'db_denydatawriter') \
                           ORDER BY schema_name";
                let res = self.execute_query(sql).await?;
                Ok(res.rows.into_iter().filter_map(|mut r| r.pop()).collect())
            }
        }
    }

    pub async fn list_tables(&self, schema: Option<&str>) -> Result<Vec<TableEntry>, DbError> {
        match &self.backend {
            DbBackend::Sqlx(pool) => {
                let sql = match self.db_type {
                    DbType::Sqlite => "SELECT name, type FROM sqlite_master \
                         WHERE type IN ('table', 'view') ORDER BY name"
                        .to_string(),
                    DbType::Postgres => {
                        let schema_name = schema.unwrap_or("public");
                        format!(
                            "SELECT name, is_view FROM ( \
                                 SELECT tablename::text AS name, false AS is_view \
                                 FROM pg_tables WHERE schemaname = '{schema_name}' \
                                 UNION ALL \
                                 SELECT viewname::text AS name, true AS is_view \
                                 FROM pg_views WHERE schemaname = '{schema_name}' \
                             ) t ORDER BY name"
                        )
                    }
                    DbType::Mysql => "SELECT table_name, IF(table_type = 'VIEW', 1, 0) \
                         FROM information_schema.tables WHERE table_schema = DATABASE() \
                         ORDER BY table_name"
                        .to_string(),
                    _ => return Ok(vec![]),
                };
                let rows = sqlx::query(&sql).fetch_all(pool).await?;
                Ok(rows
                    .iter()
                    .map(|r| {
                        let is_view = match self.db_type {
                            DbType::Sqlite => row_cell(r, 1) == "view",
                            _ => matches!(row_cell(r, 1).as_str(), "1" | "true" | "t"),
                        };
                        TableEntry {
                            name: row_cell(r, 0),
                            is_view,
                        }
                    })
                    .collect())
            }
            DbBackend::Mssql { .. } => {
                let schema_name = schema.unwrap_or("dbo");
                let sql = format!(
                    "SELECT table_name, table_type FROM information_schema.tables \
                     WHERE table_schema = '{}' ORDER BY table_name",
                    schema_name.replace('\'', "''")
                );
                let res = self.execute_query(&sql).await?;
                Ok(res
                    .rows
                    .into_iter()
                    .map(|r| {
                        let name = r.first().cloned().unwrap_or_default();
                        let is_view = r
                            .get(1)
                            .map(|t| t.to_lowercase().contains("view"))
                            .unwrap_or(false);
                        TableEntry { name, is_view }
                    })
                    .collect())
            }
        }
    }

    pub async fn query_table(
        &self,
        schema: Option<&str>,
        table: &str,
        limit: u32,
        offset: u32,
        sort: Option<(&str, bool)>,
    ) -> Result<QueryResult, DbError> {
        match self.db_type {
            DbType::Postgres => self.pg_text_query(schema, table, limit, offset, sort).await,
            DbType::Mssql => {
                let s = schema.unwrap_or("dbo");
                let order_by = sort
                    .map(|(col, desc)| {
                        format!(
                            " ORDER BY [{}] {}",
                            col.replace(']', "]]"),
                            if desc { "DESC" } else { "ASC" }
                        )
                    })
                    .unwrap_or_else(|| " ORDER BY (SELECT NULL)".to_string());
                let sql = format!(
                    "SELECT * FROM [{}].[{}] {} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                    s.replace(']', "]]"),
                    table.replace(']', "]]"),
                    order_by,
                    offset,
                    limit
                );
                self.execute_query(&sql).await
            }
            _ => {
                let order_by = sort
                    .map(|(col, desc)| {
                        format!(
                            " ORDER BY {} {}",
                            self.quote_ident(col),
                            if desc { "DESC" } else { "ASC" }
                        )
                    })
                    .unwrap_or_default();
                let sql = format!(
                    "SELECT * FROM {}{} LIMIT {} OFFSET {}",
                    self.quote_ident(table),
                    order_by,
                    limit,
                    offset
                );
                self.execute_query(&sql).await
            }
        }
    }

    // Postgres: cast every column to text to bypass sqlx::any type limitations
    async fn pg_text_query(
        &self,
        schema: Option<&str>,
        table: &str,
        limit: u32,
        offset: u32,
        sort: Option<(&str, bool)>,
    ) -> Result<QueryResult, DbError> {
        let pool = self.pool();
        let schema_name = schema.unwrap_or("public");
        let cols_sql = format!(
            "SELECT column_name::text FROM information_schema.columns \
             WHERE table_schema = '{}' AND table_name = '{}' \
             ORDER BY ordinal_position",
            schema_name, table
        );
        let col_rows = sqlx::query(&cols_sql).fetch_all(pool).await?;
        if col_rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: 0,
            });
        }
        let columns: Vec<String> = col_rows.iter().map(|r| row_cell(r, 0)).collect();
        let select = columns
            .iter()
            .map(|c| format!("{}::text", self.quote_ident(c)))
            .collect::<Vec<_>>()
            .join(", ");
        let order_by = sort
            .map(|(col, desc)| {
                format!(
                    " ORDER BY {} {}",
                    self.quote_ident(col),
                    if desc { "DESC" } else { "ASC" }
                )
            })
            .unwrap_or_default();
        let sql = format!(
            "SELECT {} FROM {}.{}{} LIMIT {} OFFSET {}",
            select,
            self.quote_ident(schema_name),
            self.quote_ident(table),
            order_by,
            limit,
            offset
        );
        let rows = sqlx::query(&sql).fetch_all(pool).await?;
        let data = rows.iter().map(row_to_strings).collect();
        Ok(QueryResult {
            columns,
            rows: data,
            rows_affected: 0,
        })
    }

    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, DbError> {
        match &self.backend {
            DbBackend::Sqlx(pool) => {
                if !Self::is_read_only_sql(sql) {
                    let result = sqlx::query(sql).execute(pool).await?;
                    return Ok(QueryResult {
                        columns: vec![],
                        rows: vec![],
                        rows_affected: result.rows_affected(),
                    });
                }

                let rows = sqlx::query(sql).fetch_all(pool).await?;
                self.rows_to_query_result(sql, rows, 0)
                    .await
                    .map_err(Into::into)
            }
            DbBackend::Mssql { client, .. } => {
                let mut c = client.lock().await;
                if !Self::is_read_only_sql(sql) {
                    let res = c
                        .execute(sql, &[])
                        .await
                        .map_err(|e| DbError::Mssql(e.to_string()))?;
                    return Ok(QueryResult {
                        columns: vec![],
                        rows: vec![],
                        rows_affected: res.total(),
                    });
                }

                let stream = c
                    .simple_query(sql)
                    .await
                    .map_err(|e| DbError::Mssql(e.to_string()))?;
                let rows = stream
                    .into_first_result()
                    .await
                    .map_err(|e| DbError::Mssql(e.to_string()))?;
                if rows.is_empty() {
                    return Ok(QueryResult {
                        columns: vec![],
                        rows: vec![],
                        rows_affected: 0,
                    });
                }
                let columns: Vec<String> = rows[0]
                    .columns()
                    .iter()
                    .map(|col| col.name().to_string())
                    .collect();
                let data: Vec<Vec<String>> = rows
                    .iter()
                    .map(|r| {
                        (0..r.columns().len())
                            .map(|i| decode_tiberius_cell(r, i))
                            .collect()
                    })
                    .collect();
                Ok(QueryResult {
                    columns,
                    rows: data,
                    rows_affected: 0,
                })
            }
        }
    }
    async fn rows_to_query_result(
        &self,
        sql: &str,
        rows: Vec<AnyRow>,
        rows_affected: u64,
    ) -> Result<QueryResult, sqlx::Error> {
        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected,
            });
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        if self.db_type == DbType::Postgres {
            // Try native decode; fall back to text-cast subquery on type error.
            if row_to_strings_checked(&rows[0]).is_ok() {
                let data = rows.iter().map(row_to_strings).collect();
                return Ok(QueryResult {
                    columns,
                    rows: data,
                    rows_affected,
                });
            }

            let select = columns
                .iter()
                .map(|c| format!("{}::text", self.quote_ident(c)))
                .collect::<Vec<_>>()
                .join(", ");
            let cast_sql = format!("SELECT {} FROM ({}) __db_eye_q", select, sql);
            let rows2 = sqlx::query(&cast_sql).fetch_all(self.pool()).await?;
            let data = rows2.iter().map(row_to_strings).collect();
            return Ok(QueryResult {
                columns,
                rows: data,
                rows_affected,
            });
        }

        let data = rows.iter().map(row_to_strings).collect();
        Ok(QueryResult {
            columns,
            rows: data,
            rows_affected,
        })
    }

    fn sanitize_sql(sql: &str) -> Option<String> {
        let chars: Vec<char> = sql.chars().collect();
        let len = chars.len();
        let mut out = String::with_capacity(len);
        let mut i = 0;

        enum State {
            Normal,
            SingleQuote,
            DoubleQuote,
            Backtick,
            LineComment,
            BlockComment(usize),
        }

        let mut state = State::Normal;

        while i < len {
            let c = chars[i];
            match state {
                State::Normal => {
                    if c == '-' && i + 1 < len && chars[i + 1] == '-' {
                        state = State::LineComment;
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                    } else if c == '/' && i + 1 < len && chars[i + 1] == '*' {
                        state = State::BlockComment(1);
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                    } else if c == '\'' {
                        state = State::SingleQuote;
                        out.push(' ');
                        i += 1;
                    } else if c == '"' {
                        state = State::DoubleQuote;
                        out.push(' ');
                        i += 1;
                    } else if c == '`' {
                        state = State::Backtick;
                        out.push(' ');
                        i += 1;
                    } else {
                        out.push(c);
                        i += 1;
                    }
                }
                State::SingleQuote => {
                    if c == '\\' && i + 1 < len {
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                    } else if c == '\'' {
                        if i + 1 < len && chars[i + 1] == '\'' {
                            out.push(' ');
                            out.push(' ');
                            i += 2;
                        } else {
                            state = State::Normal;
                            out.push(' ');
                            i += 1;
                        }
                    } else {
                        out.push(' ');
                        i += 1;
                    }
                }
                State::DoubleQuote => {
                    if c == '\\' && i + 1 < len {
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                    } else if c == '"' {
                        if i + 1 < len && chars[i + 1] == '"' {
                            out.push(' ');
                            out.push(' ');
                            i += 2;
                        } else {
                            state = State::Normal;
                            out.push(' ');
                            i += 1;
                        }
                    } else {
                        out.push(' ');
                        i += 1;
                    }
                }
                State::Backtick => {
                    if c == '`' {
                        state = State::Normal;
                        out.push(' ');
                        i += 1;
                    } else {
                        out.push(' ');
                        i += 1;
                    }
                }
                State::LineComment => {
                    if c == '\n' {
                        state = State::Normal;
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
                State::BlockComment(depth) => {
                    if c == '/' && i + 1 < len && chars[i + 1] == '*' {
                        state = State::BlockComment(depth + 1);
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                    } else if c == '*' && i + 1 < len && chars[i + 1] == '/' {
                        if depth <= 1 {
                            state = State::Normal;
                        } else {
                            state = State::BlockComment(depth - 1);
                        }
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                    } else {
                        out.push(' ');
                        i += 1;
                    }
                }
            }
        }

        match state {
            State::Normal | State::LineComment => Some(out),
            _ => None,
        }
    }

    pub fn is_read_only_sql(sql: &str) -> bool {
        let sanitized = match Self::sanitize_sql(sql) {
            Some(s) => s,
            None => return false,
        };

        let mut statements: Vec<&str> = sanitized.split(';').collect();
        if statements.is_empty() {
            return false;
        }
        if statements
            .last()
            .map(|s| s.trim().is_empty())
            .unwrap_or(false)
        {
            statements.pop();
        }
        if statements.is_empty() {
            return false;
        }

        const ALLOWED_START: &[&str] = &[
            "SELECT", "WITH", "EXPLAIN", "PRAGMA", "SHOW", "DESCRIBE", "DESC",
        ];
        const MUTATING_KEYWORDS: &[&str] = &[
            "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TRUNCATE", "REPLACE",
            "GRANT", "REVOKE", "VACUUM", "INTO",
        ];

        for stmt in statements {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                return false;
            }

            let words: Vec<String> = stmt
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|w| !w.is_empty())
                .map(|w| w.to_uppercase())
                .collect();

            if words.is_empty() {
                return false;
            }

            let first_word = &words[0];
            if !ALLOWED_START.contains(&first_word.as_str()) {
                return false;
            }

            if first_word == "PRAGMA" && stmt.contains('=') {
                return false;
            }

            for word in &words {
                if MUTATING_KEYWORDS.contains(&word.as_str()) {
                    return false;
                }
            }
        }

        true
    }

    pub async fn get_columns(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, DbError> {
        match self.db_type {
            DbType::Sqlite => {
                let sql = format!("PRAGMA table_info(\"{}\")", table);
                let rows = sqlx::query(&sql).fetch_all(self.pool()).await?;
                let mut cols: Vec<ColumnInfo> = rows
                    .iter()
                    .map(|r| {
                        let name = row_cell(r, 1);
                        let data_type = row_cell(r, 2);
                        let notnull = r.try_get::<i64, _>(3).unwrap_or(0);
                        let dflt_value = row_cell(r, 4);
                        let pk = r.try_get::<i64, _>(5).unwrap_or(0);
                        ColumnInfo {
                            name,
                            data_type,
                            is_pk: pk > 0,
                            pk_order: pk,
                            fk_table: None,
                            fk_column: None,
                            is_nullable: notnull == 0,
                            default_value: if dflt_value.is_empty() {
                                None
                            } else {
                                Some(dflt_value)
                            },
                        }
                    })
                    .collect();
                // PRAGMA foreign_key_list: id, seq, table, from, to, ...
                let fk_sql = format!("PRAGMA foreign_key_list(\"{}\")", table);
                let fk_rows = sqlx::query(&fk_sql)
                    .fetch_all(self.pool())
                    .await
                    .unwrap_or_default();
                for fk_row in &fk_rows {
                    let from_col = row_cell(fk_row, 3);
                    let to_table = row_cell(fk_row, 2);
                    let to_col = row_cell(fk_row, 4);
                    if let Some(col) = cols.iter_mut().find(|c| c.name == from_col) {
                        col.fk_table = Some(to_table);
                        col.fk_column = Some(if to_col.is_empty() {
                            "id".to_string()
                        } else {
                            to_col
                        });
                    }
                }
                Ok(cols)
            }
            DbType::Postgres => {
                let schema_name = schema.unwrap_or("public");
                let sql = format!(
                    "SELECT c.column_name::text, c.data_type::text, \
                     CASE WHEN pk.column_name IS NOT NULL THEN 1 ELSE 0 END, \
                     COALESCE(pk.ordinal_position, 0)::text, \
                     c.is_nullable::text, COALESCE(c.column_default::text, ''), \
                     COALESCE(c.character_maximum_length::text, ''), \
                     COALESCE(c.numeric_precision::text, ''), \
                     COALESCE(c.numeric_scale::text, ''), \
                     COALESCE(c.numeric_precision_radix::text, '') \
                     FROM information_schema.columns c \
                     LEFT JOIN ( \
                         SELECT kcu.column_name, kcu.ordinal_position \
                         FROM information_schema.table_constraints tc \
                         JOIN information_schema.key_column_usage kcu \
                           ON tc.constraint_name = kcu.constraint_name \
                          AND tc.table_name = kcu.table_name \
                          AND tc.table_schema = kcu.table_schema \
                         WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_name = '{}' \
                           AND tc.table_schema = '{}' \
                     ) pk ON c.column_name = pk.column_name \
                     WHERE c.table_name = '{}' AND c.table_schema = '{}' ORDER BY c.ordinal_position",
                    table, schema_name, table, schema_name
                );
                let rows = sqlx::query(&sql).fetch_all(self.pool()).await?;
                let mut cols: Vec<ColumnInfo> = rows
                    .iter()
                    .map(|r| {
                        let default_value = row_cell(r, 5);
                        ColumnInfo {
                            name: row_cell(r, 0),
                            data_type: format_precise_type(
                                &row_cell(r, 1),
                                &row_cell(r, 6),
                                &row_cell(r, 7),
                                &row_cell(r, 8),
                                &row_cell(r, 9),
                            ),
                            is_pk: row_cell(r, 2) == "1",
                            pk_order: row_cell(r, 3).parse::<i64>().unwrap_or(0),
                            fk_table: None,
                            fk_column: None,
                            is_nullable: row_cell(r, 4) == "YES",
                            default_value: if default_value.is_empty() {
                                None
                            } else {
                                Some(default_value)
                            },
                        }
                    })
                    .collect();
                let fk_sql = format!(
                    "SELECT kcu.column_name::text, ccu.table_name::text, ccu.column_name::text \
                     FROM information_schema.key_column_usage kcu \
                     JOIN information_schema.referential_constraints rc \
                       ON kcu.constraint_name = rc.constraint_name \
                      AND kcu.constraint_schema = rc.constraint_schema \
                     JOIN information_schema.constraint_column_usage ccu \
                       ON rc.unique_constraint_name = ccu.constraint_name \
                      AND rc.unique_constraint_schema = ccu.constraint_schema \
                     WHERE kcu.table_schema = '{}' AND kcu.table_name = '{}'",
                    schema_name, table
                );
                let fk_rows = sqlx::query(&fk_sql)
                    .fetch_all(self.pool())
                    .await
                    .unwrap_or_default();
                for fk_row in &fk_rows {
                    let from_col = row_cell(fk_row, 0);
                    let to_table = row_cell(fk_row, 1);
                    let to_col = row_cell(fk_row, 2);
                    if let Some(col) = cols.iter_mut().find(|c| c.name == from_col) {
                        col.fk_table = Some(to_table);
                        col.fk_column = Some(to_col);
                    }
                }
                Ok(cols)
            }
            DbType::Mysql => {
                let sql = format!(
                    "SELECT c.column_name, c.data_type, IF(kcu.column_name IS NULL,'0','1'), \
                     COALESCE(kcu.ordinal_position, 0), \
                     c.is_nullable, COALESCE(c.column_default, ''), \
                     COALESCE(c.character_maximum_length, ''), \
                     COALESCE(c.numeric_precision, ''), \
                     COALESCE(c.numeric_scale, ''), \
                     COALESCE(c.numeric_precision_radix, '') \
                     FROM information_schema.columns c \
                     LEFT JOIN information_schema.key_column_usage kcu \
                       ON c.table_schema = kcu.table_schema \
                      AND c.table_name = kcu.table_name \
                      AND c.column_name = kcu.column_name \
                      AND kcu.constraint_name = 'PRIMARY' \
                     WHERE c.table_schema = DATABASE() AND c.table_name = '{}' \
                     ORDER BY c.ordinal_position",
                    table
                );
                let rows = sqlx::query(&sql).fetch_all(self.pool()).await?;
                let mut cols: Vec<ColumnInfo> = rows
                    .iter()
                    .map(|r| {
                        let default_value = row_cell(r, 5);
                        ColumnInfo {
                            name: row_cell(r, 0),
                            data_type: format_precise_type(
                                &row_cell(r, 1),
                                &row_cell(r, 6),
                                &row_cell(r, 7),
                                &row_cell(r, 8),
                                &row_cell(r, 9),
                            ),
                            is_pk: row_cell(r, 2) == "1",
                            pk_order: row_cell(r, 3).parse::<i64>().unwrap_or(0),
                            fk_table: None,
                            fk_column: None,
                            is_nullable: row_cell(r, 4) == "YES",
                            default_value: if default_value.is_empty() {
                                None
                            } else {
                                Some(default_value)
                            },
                        }
                    })
                    .collect();
                let fk_sql = format!(
                    "SELECT column_name, referenced_table_name, referenced_column_name \
                     FROM information_schema.key_column_usage \
                     WHERE table_schema = DATABASE() AND table_name = '{}' \
                     AND referenced_table_name IS NOT NULL",
                    table
                );
                let fk_rows = sqlx::query(&fk_sql)
                    .fetch_all(self.pool())
                    .await
                    .unwrap_or_default();
                for fk_row in &fk_rows {
                    let from_col = row_cell(fk_row, 0);
                    let to_table = row_cell(fk_row, 1);
                    let to_col = row_cell(fk_row, 2);
                    if let Some(col) = cols.iter_mut().find(|c| c.name == from_col) {
                        col.fk_table = Some(to_table);
                        col.fk_column = Some(to_col);
                    }
                }
                Ok(cols)
            }
            DbType::Mssql => {
                let s = schema.unwrap_or("dbo");
                let sql = format!(
                    "SELECT c.column_name, c.data_type, \
                     CASE WHEN pk.column_name IS NOT NULL THEN '1' ELSE '0' END, \
                     COALESCE(CAST(pk.ordinal_position AS VARCHAR), '0'), \
                     c.is_nullable, COALESCE(c.column_default, ''), \
                     COALESCE(CAST(c.character_maximum_length AS VARCHAR), ''), \
                     COALESCE(CAST(c.numeric_precision AS VARCHAR), ''), \
                     COALESCE(CAST(c.numeric_scale AS VARCHAR), ''), \
                     COALESCE(CAST(c.numeric_precision_radix AS VARCHAR), '') \
                     FROM information_schema.columns c \
                     LEFT JOIN ( \
                         SELECT tc.table_schema, tc.table_name, kcu.column_name, kcu.ordinal_position \
                         FROM information_schema.table_constraints tc \
                         JOIN information_schema.key_column_usage kcu \
                           ON tc.constraint_name = kcu.constraint_name \
                          AND tc.table_schema = kcu.table_schema \
                          AND tc.table_name = kcu.table_name \
                         WHERE tc.constraint_type = 'PRIMARY KEY' \
                     ) pk ON c.table_schema = kcu.table_schema \
                          AND c.table_name = kcu.table_name \
                          AND c.column_name = kcu.column_name \
                     WHERE c.table_schema = '{s}' AND c.table_name = '{table}' \
                     ORDER BY c.ordinal_position"
                );
                let res = self.execute_query(&sql).await?;
                let mut cols: Vec<ColumnInfo> = res
                    .rows
                    .iter()
                    .map(|r| {
                        let name = r.first().cloned().unwrap_or_default();
                        let raw_type = r.get(1).cloned().unwrap_or_default();
                        let is_pk = r.get(2).map(|v| v == "1").unwrap_or(false);
                        let pk_order = r.get(3).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
                        let is_nullable = r.get(4).map(|v| v == "YES").unwrap_or(true);
                        let dflt = r
                            .get(5)
                            .cloned()
                            .filter(|v| !v.is_empty() && v != NULL_DISPLAY);
                        let char_len = r.get(6).cloned().unwrap_or_default();
                        let num_prec = r.get(7).cloned().unwrap_or_default();
                        let num_scale = r.get(8).cloned().unwrap_or_default();
                        let num_radix = r.get(9).cloned().unwrap_or_default();
                        ColumnInfo {
                            name,
                            data_type: format_precise_type(
                                &raw_type, &char_len, &num_prec, &num_scale, &num_radix,
                            ),
                            is_pk,
                            pk_order,
                            fk_table: None,
                            fk_column: None,
                            is_nullable,
                            default_value: dflt,
                        }
                    })
                    .collect();

                let fk_sql = format!(
                    "SELECT kcu.column_name, ccu.table_name, ccu.column_name \
                     FROM information_schema.key_column_usage kcu \
                     JOIN information_schema.referential_constraints rc \
                       ON kcu.constraint_name = rc.constraint_name \
                      AND kcu.constraint_schema = rc.constraint_schema \
                     JOIN information_schema.constraint_column_usage ccu \
                       ON rc.unique_constraint_name = ccu.constraint_name \
                      AND rc.unique_constraint_schema = ccu.constraint_schema \
                     WHERE kcu.table_schema = '{}' AND kcu.table_name = '{}'",
                    s.replace('\'', "''"),
                    table.replace('\'', "''")
                );
                if let Ok(fk_res) = self.execute_query(&fk_sql).await {
                    for r in fk_res.rows {
                        if let (Some(from_col), Some(to_table), Some(to_col)) =
                            (r.first(), r.get(1), r.get(2))
                            && let Some(col) = cols.iter_mut().find(|c| &c.name == from_col)
                        {
                            col.fk_table = Some(to_table.clone());
                            col.fk_column = Some(to_col.clone());
                        }
                    }
                }
                Ok(cols)
            }
        }
    }

    pub async fn get_fk_values(
        &self,
        schema: Option<&str>,
        ref_table: &str,
        ref_column: &str,
    ) -> Result<Vec<String>, DbError> {
        let sql = match self.db_type {
            DbType::Postgres => {
                let schema_name = schema.unwrap_or("public");
                format!(
                    "SELECT DISTINCT {}::text FROM {}.{} ORDER BY 1 LIMIT 50",
                    self.quote_ident(ref_column),
                    self.quote_ident(schema_name),
                    self.quote_ident(ref_table)
                )
            }
            DbType::Mssql => {
                let s = schema.unwrap_or("dbo");
                format!(
                    "SELECT TOP 50 [{}] FROM [{}].[{}] WHERE [{}] IS NOT NULL ORDER BY [{}]",
                    ref_column.replace(']', "]]"),
                    s.replace(']', "]]"),
                    ref_table.replace(']', "]]"),
                    ref_column.replace(']', "]]"),
                    ref_column.replace(']', "]]")
                )
            }
            _ => format!(
                "SELECT DISTINCT {} FROM {} ORDER BY 1 LIMIT 50",
                self.quote_ident(ref_column),
                self.quote_ident(ref_table)
            ),
        };
        match &self.backend {
            DbBackend::Sqlx(pool) => {
                let rows = sqlx::query(&sql).fetch_all(pool).await?;
                Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
            }
            DbBackend::Mssql { .. } => {
                let res = self.execute_query(&sql).await?;
                Ok(res.rows.into_iter().filter_map(|mut r| r.pop()).collect())
            }
        }
    }

    pub async fn execute_write_with_values(
        &self,
        sql: &str,
        values: &[Option<String>],
    ) -> Result<u64, DbError> {
        match &self.backend {
            DbBackend::Sqlx(pool) => {
                let mut query = sqlx::query(sql);
                for value in values {
                    query = query.bind(value.clone());
                }
                let result = query.execute(pool).await?;
                Ok(result.rows_affected())
            }
            DbBackend::Mssql { client, .. } => {
                let mut c = client.lock().await;
                let mut params: Vec<Box<dyn tiberius::ToSql>> = Vec::new();
                for val in values {
                    match val {
                        Some(v) => params.push(Box::new(v.clone())),
                        None => params.push(Box::new(Option::<String>::None)),
                    }
                }
                let param_refs: Vec<&dyn tiberius::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                let res = c
                    .execute(sql, &param_refs)
                    .await
                    .map_err(|e| DbError::Mssql(e.to_string()))?;
                Ok(res.total())
            }
        }
    }

    pub async fn table_stats(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> Result<TableStats, DbError> {
        match self.db_type {
            DbType::Sqlite => {
                let sql = format!("PRAGMA index_list(\"{}\")", table);
                let idx_rows = sqlx::query(&sql).fetch_all(self.pool()).await?;
                let mut indexes = Vec::new();
                for idx_row in &idx_rows {
                    let name = row_cell(idx_row, 1);
                    let unique = row_cell(idx_row, 2) == "1";
                    let cols_sql = format!("PRAGMA index_info(\"{}\")", name);
                    let col_rows = sqlx::query(&cols_sql)
                        .fetch_all(self.pool())
                        .await
                        .unwrap_or_default();
                    let cols: Vec<String> = col_rows.iter().map(|r| row_cell(r, 2)).collect();
                    let suffix = if unique { " UNIQUE" } else { "" };
                    indexes.push(format!("{} ({}){}", name, cols.join(", "), suffix));
                }
                Ok(TableStats {
                    indexes,
                    size_label: None,
                })
            }
            DbType::Postgres => {
                let schema_name = schema.unwrap_or("public");
                let sql = format!(
                    "SELECT indexname::text, indexdef::text FROM pg_indexes \
                     WHERE schemaname = '{schema_name}' AND tablename = '{table}'"
                );
                let idx_rows = sqlx::query(&sql).fetch_all(self.pool()).await?;
                let indexes = idx_rows
                    .iter()
                    .map(|r| {
                        let name = row_cell(r, 0);
                        let def = row_cell(r, 1);
                        if def.to_uppercase().contains("UNIQUE") {
                            format!("{name} UNIQUE")
                        } else {
                            name
                        }
                    })
                    .collect();
                let size_sql = format!(
                    "SELECT pg_size_pretty(pg_total_relation_size('{}.{}'))",
                    self.quote_ident(schema_name),
                    self.quote_ident(table)
                );
                let size_label = sqlx::query(&size_sql)
                    .fetch_one(self.pool())
                    .await
                    .ok()
                    .map(|r| row_cell(&r, 0));
                Ok(TableStats {
                    indexes,
                    size_label,
                })
            }
            DbType::Mysql => {
                let idx_sql = format!(
                    "SELECT index_name, non_unique, GROUP_CONCAT(column_name ORDER BY seq_in_index) \
                     FROM information_schema.statistics \
                     WHERE table_schema = DATABASE() AND table_name = '{table}' \
                     GROUP BY index_name, non_unique"
                );
                let idx_rows = sqlx::query(&idx_sql).fetch_all(self.pool()).await?;
                let indexes = idx_rows
                    .iter()
                    .map(|r| {
                        let name = row_cell(r, 0);
                        let non_unique = row_cell(r, 1) == "1";
                        let cols = row_cell(r, 2);
                        let suffix = if non_unique { "" } else { " UNIQUE" };
                        format!("{name} ({cols}){suffix}")
                    })
                    .collect();
                let size_sql = format!(
                    "SELECT data_length + index_length FROM information_schema.tables \
                     WHERE table_schema = DATABASE() AND table_name = '{table}'"
                );
                let size_label = sqlx::query(&size_sql)
                    .fetch_one(self.pool())
                    .await
                    .ok()
                    .and_then(|r| row_cell(&r, 0).parse::<i64>().ok())
                    .map(format_bytes);
                Ok(TableStats {
                    indexes,
                    size_label,
                })
            }
            DbType::Mssql => {
                let s = schema.unwrap_or("dbo");
                let idx_sql = format!(
                    "SELECT i.name, i.is_unique, c.name FROM sys.indexes i \
                     JOIN sys.index_columns ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id \
                     JOIN sys.columns c ON ic.object_id = c.object_id AND ic.column_id = c.column_id \
                     WHERE i.object_id = OBJECT_ID('[{s}].[{table}]') AND i.name IS NOT NULL \
                     ORDER BY i.name, ic.key_ordinal"
                );
                let idx_res = self.execute_query(&idx_sql).await.unwrap_or(QueryResult {
                    columns: vec![],
                    rows: vec![],
                    rows_affected: 0,
                });
                let mut indexes = Vec::new();
                for row in idx_res.rows {
                    if let (Some(name), Some(unique), Some(col)) =
                        (row.first(), row.get(1), row.get(2))
                    {
                        let uniq_str = if unique == "1" || unique == "true" {
                            "UNIQUE "
                        } else {
                            ""
                        };
                        indexes.push(format!("{uniq_str}{name} ({col})"));
                    }
                }
                Ok(TableStats {
                    indexes,
                    size_label: None,
                })
            }
        }
    }

    pub async fn count_rows(&self, schema: Option<&str>, table: &str) -> Result<i64, DbError> {
        let sql = match self.db_type {
            DbType::Postgres => {
                let schema_name = schema.unwrap_or("public");
                format!(
                    "SELECT COUNT(*)::text FROM {}.{}",
                    self.quote_ident(schema_name),
                    self.quote_ident(table)
                )
            }
            DbType::Mssql => {
                let s = schema.unwrap_or("dbo");
                format!(
                    "SELECT COUNT(*) FROM [{}].[{}]",
                    s.replace(']', "]]"),
                    table.replace(']', "]]")
                )
            }
            _ => format!("SELECT COUNT(*) FROM {}", self.quote_ident(table)),
        };
        match &self.backend {
            DbBackend::Sqlx(pool) => {
                let row = sqlx::query(&sql).fetch_one(pool).await?;
                let count = row
                    .try_get::<String, _>(0)
                    .map(|s| s.parse::<i64>().unwrap_or(0))
                    .or_else(|_| row.try_get::<i64, _>(0))
                    .or_else(|_| row.try_get::<i32, _>(0).map(|v| v as i64))
                    .unwrap_or(0);
                Ok(count)
            }
            DbBackend::Mssql { .. } => {
                let res = self.execute_query(&sql).await?;
                let count = res
                    .rows
                    .first()
                    .and_then(|r| r.first())
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                Ok(count)
            }
        }
    }
}

pub const NULL_DISPLAY: &str = "<NULL>";

pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub rows_affected: u64,
}

#[derive(Clone)]
pub struct TableEntry {
    pub name: String,
    pub is_view: bool,
}

pub struct TableStats {
    pub indexes: Vec<String>,
    pub size_label: Option<String>,
}

fn format_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[derive(Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_pk: bool,
    pub pk_order: i64,
    pub fk_table: Option<String>,
    pub fk_column: Option<String>,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

impl ColumnInfo {
    pub fn input_hint(&self) -> &'static str {
        let kind = self.normalized_type();
        if is_integer_type(&kind) {
            "integer | empty/\\null = NULL"
        } else if is_decimal_type(&kind) {
            "decimal (e.g. 12.34) | empty/\\null = NULL"
        } else if is_float_type(&kind) {
            "number | empty/\\null = NULL"
        } else if is_bool_type(&kind) {
            "true/false/1/0 | empty/\\null = NULL"
        } else if is_binary_type(&kind) {
            "binary/blob is read-only here"
        } else if is_datetime_type(&kind) {
            "date/time text | empty/\\null = NULL"
        } else {
            "text | empty/\\null = NULL"
        }
    }

    pub fn is_binary(&self) -> bool {
        is_binary_type(&self.normalized_type())
    }

    /// True when a value must be supplied on insert: not nullable, no default, and not a PK
    /// (PKs get their own read-only/auto-generated handling in the CRUD form).
    pub fn is_required(&self) -> bool {
        !self.is_nullable && self.default_value.is_none() && !self.is_pk
    }

    pub fn validate_input(&self, value: &str) -> Result<(), String> {
        if value.is_empty() || value.eq_ignore_ascii_case("\\null") || value == NULL_DISPLAY {
            return Ok(());
        }

        let kind = self.normalized_type();
        if self.is_binary() {
            return Err(format!(
                "{} is binary/blob and cannot be edited inline",
                self.name
            ));
        }
        if is_integer_type(&kind) && value.parse::<i64>().is_err() {
            return Err(format!("{} expects an integer", self.name));
        }
        if is_decimal_type(&kind) && !is_valid_decimal_format(value) {
            return Err(format!("{} expects a decimal number", self.name));
        }
        if is_float_type(&kind) && value.parse::<f64>().is_err() {
            return Err(format!("{} expects a number", self.name));
        }
        if is_bool_type(&kind)
            && !matches!(value.to_lowercase().as_str(), "true" | "false" | "1" | "0")
        {
            return Err(format!("{} expects true/false or 1/0", self.name));
        }
        Ok(())
    }

    fn normalized_type(&self) -> String {
        self.data_type.to_lowercase()
    }
}

fn base_type_name(kind: &str) -> String {
    kind.split('(')
        .next()
        .unwrap_or(kind)
        .trim()
        .to_ascii_lowercase()
}

fn is_valid_decimal_format(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let s = if let Some(stripped) = s.strip_prefix('+').or_else(|| s.strip_prefix('-')) {
        stripped
    } else {
        s
    };
    if s.is_empty() {
        return false;
    }
    let mut has_dot = false;
    let mut has_digit = false;
    for c in s.chars() {
        if c == '.' {
            if has_dot {
                return false;
            }
            has_dot = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
        } else {
            return false;
        }
    }
    has_digit
}

pub(crate) fn is_integer_type(kind: &str) -> bool {
    let base = base_type_name(kind);
    matches!(
        base.as_str(),
        "int"
            | "integer"
            | "tinyint"
            | "smallint"
            | "mediumint"
            | "bigint"
            | "int2"
            | "int4"
            | "int8"
            | "serial"
            | "smallserial"
            | "bigserial"
            | "year"
    )
}

pub(crate) fn is_decimal_type(kind: &str) -> bool {
    let base = base_type_name(kind);
    matches!(base.as_str(), "decimal" | "dec" | "numeric" | "fixed")
}

pub(crate) fn is_float_type(kind: &str) -> bool {
    let base = base_type_name(kind);
    matches!(
        base.as_str(),
        "real" | "float" | "double" | "double precision" | "float4" | "float8"
    )
}

fn is_bool_type(kind: &str) -> bool {
    kind.contains("bool") || kind == "bit"
}

fn is_binary_type(kind: &str) -> bool {
    kind.contains("blob") || kind.contains("binary") || kind.contains("bytea")
}

fn is_datetime_type(kind: &str) -> bool {
    kind.contains("date") || kind.contains("time")
}

/// Appends declared length/precision to a base type name, e.g. "varchar" -> "varchar(255)"
/// or "numeric" -> "numeric(10,2)". Only applies precision for base-10 (radix 10) numeric
/// types (decimal/numeric) so intrinsic binary-width types like integer/real/double aren't
/// misleadingly annotated with their bit width.
fn format_precise_type(
    base: &str,
    char_len: &str,
    num_precision: &str,
    num_scale: &str,
    num_precision_radix: &str,
) -> String {
    if !char_len.is_empty() {
        return format!("{base}({char_len})");
    }
    if num_precision_radix == "10" && !num_precision.is_empty() {
        if !num_scale.is_empty() && num_scale != "0" {
            return format!("{base}({num_precision},{num_scale})");
        }
        return format!("{base}({num_precision})");
    }
    base.to_string()
}

fn decode_any_cell(row: &AnyRow, col_idx: usize) -> Option<String> {
    if let Ok(v) = row.try_get::<String, _>(col_idx) {
        return Some(v);
    }
    if let Ok(v) = row.try_get::<i64, _>(col_idx) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<i32, _>(col_idx) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f64, _>(col_idx) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f32, _>(col_idx) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<bool, _>(col_idx) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(col_idx) {
        return Some(format!("<blob {}b>", v.len()));
    }
    None
}

fn row_cell(row: &AnyRow, i: usize) -> String {
    decode_any_cell(row, i).unwrap_or_default()
}

fn row_to_strings_checked(row: &AnyRow) -> Result<Vec<String>, ()> {
    let mut result = Vec::with_capacity(row.columns().len());
    for i in 0..row.columns().len() {
        match decode_any_cell(row, i) {
            Some(v) => result.push(v),
            None => return Err(()),
        }
    }
    Ok(result)
}

fn row_to_strings(row: &AnyRow) -> Vec<String> {
    (0..row.columns().len())
        .map(|i| decode_any_cell(row, i).unwrap_or_else(|| NULL_DISPLAY.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn execute_write_with_values_binds_sqlite_values() {
        let path = std::env::temp_dir().join(format!("db-eye-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::File::create(&path).unwrap();
        let db = DbClient::connect(path.to_str().unwrap()).await.unwrap();
        sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
            .execute(db.pool())
            .await
            .unwrap();

        let rows = db
            .execute_write_with_values(
                "INSERT INTO users (name, email) VALUES (?, ?)",
                &[Some("Ada's".to_string()), None],
            )
            .await
            .unwrap();
        assert_eq!(rows, 1);

        let result = db
            .execute_query("SELECT name, email FROM users WHERE id = 1")
            .await
            .unwrap();
        assert_eq!(
            result.rows,
            vec![vec!["Ada's".to_string(), NULL_DISPLAY.to_string()]]
        );
        db.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn classify_db_message_maps_common_constraint_errors() {
        let fk = classify_db_message("Delete failed", "FOREIGN KEY constraint failed");
        assert!(fk.contains("foreign key constraint failed"));

        let unique = classify_db_message("Insert failed", "UNIQUE constraint failed: users.email");
        assert!(unique.contains("duplicate value"));

        let not_null =
            classify_db_message("Insert failed", "NOT NULL constraint failed: users.name");
        assert!(not_null.contains("required field"));
    }

    #[test]
    fn classify_db_message_maps_permission_and_syntax_errors() {
        let readonly = classify_db_message("Update failed", "attempt to write a readonly database");
        assert!(readonly.contains("read-only"));

        let syntax = classify_db_message("Query failed", "near \"FROM\": syntax error");
        assert!(syntax.contains("SQL syntax error"));
    }

    #[tokio::test]
    async fn connect_auto_creates_missing_sqlite_file() {
        let path =
            std::env::temp_dir().join(format!("db-eye-autocreate-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(!path.exists());

        let db = DbClient::connect(path.to_str().unwrap()).await.unwrap();
        assert!(path.exists());
        db.close().await;
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_sqlite_url_relative_paths() {
        let res1 = DbClient::connect("./test_rel1.sqlite").await;
        println!("res1: {:?}", res1.err());
        let _ = std::fs::remove_file("./test_rel1.sqlite");
    }

    #[test]
    fn normalize_sqlite_url_handles_various_formats() {
        assert_eq!(normalize_sqlite_url(":memory:"), ":memory:");
        assert_eq!(normalize_sqlite_url("sqlite::memory:"), "sqlite::memory:");
        assert_eq!(normalize_sqlite_url("test.db"), "sqlite:test.db?mode=rwc");
        assert_eq!(
            normalize_sqlite_url("sqlite:test.db"),
            "sqlite:test.db?mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:test.db?cache=shared"),
            "sqlite:test.db?cache=shared&mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:test.db?mode=ro"),
            "sqlite:test.db?mode=ro"
        );
    }

    #[test]
    fn type_categorization_separates_integers_decimals_and_floats() {
        assert!(is_integer_type("int"));
        assert!(is_integer_type("INTEGER"));
        assert!(is_integer_type("int(11)"));
        assert!(is_integer_type("bigint"));
        assert!(is_integer_type("serial"));
        assert!(is_integer_type("year"));
        // Custom types and enums must NOT be recognized as integers
        assert!(!is_integer_type("point"));
        assert!(!is_integer_type("international_code"));
        assert!(!is_integer_type("interval"));

        assert!(is_decimal_type("decimal"));
        assert!(is_decimal_type("numeric"));
        assert!(is_decimal_type("decimal(10,2)"));
        assert!(is_decimal_type("numeric(8, 4)"));
        assert!(!is_decimal_type("float"));
        assert!(!is_decimal_type("int"));

        assert!(is_float_type("real"));
        assert!(is_float_type("float"));
        assert!(is_float_type("double precision"));
        // Decimal/numeric must NOT be recognized as floats
        assert!(!is_float_type("decimal"));
        assert!(!is_float_type("numeric"));
    }

    #[test]
    fn decimal_input_validation_and_hints() {
        let col = ColumnInfo {
            name: "price".to_string(),
            data_type: "decimal(10,2)".to_string(),
            is_pk: false,
            pk_order: 0,
            fk_table: None,
            fk_column: None,
            is_nullable: false,
            default_value: None,
        };
        assert_eq!(
            col.input_hint(),
            "decimal (e.g. 12.34) | empty/\\null = NULL"
        );
        assert!(col.validate_input("12.34").is_ok());
        assert!(col.validate_input("-99.999").is_ok());
        assert!(col.validate_input("100").is_ok());
        assert!(
            col.validate_input("12345678901234567890.1234567890")
                .is_ok()
        );
        assert!(col.validate_input("not-a-number").is_err());
        assert!(col.validate_input("12.34.56").is_err());
    }

    #[derive(Debug)]
    struct MockDbError {
        code: Option<&'static str>,
        message: &'static str,
    }
    impl std::fmt::Display for MockDbError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.message)
        }
    }
    impl std::error::Error for MockDbError {}
    impl sqlx::error::DatabaseError for MockDbError {
        fn message(&self) -> &str {
            self.message
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.code.map(std::borrow::Cow::Borrowed)
        }
    }

    #[test]
    fn format_db_error_inspects_error_codes() {
        let err = sqlx::Error::Database(Box::new(MockDbError {
            code: Some("23505"),
            message: "Key (id)=(1) already exists",
        }));
        let formatted = format_db_error("Insert", &err.into());
        assert!(formatted.contains("duplicate value violates a unique constraint"));

        let err_table = sqlx::Error::Database(Box::new(MockDbError {
            code: Some("42P01"),
            message: "relation \"users\" does not exist",
        }));
        let formatted_table = format_db_error("Query", &err_table.into());
        assert!(formatted_table.contains("table or database object not found"));

        let err_mysql = sqlx::Error::Database(Box::new(MockDbError {
            code: Some("1062"),
            message: "Duplicate entry '1' for key 'PRIMARY'",
        }));
        let formatted_mysql = format_db_error("Insert", &err_mysql.into());
        assert!(formatted_mysql.contains("duplicate value violates a unique constraint"));

        let err_sqlite_ro = sqlx::Error::Database(Box::new(MockDbError {
            code: Some("8"),
            message: "attempt to write a readonly database",
        }));
        let formatted_ro = format_db_error("Update", &err_sqlite_ro.into());
        assert!(formatted_ro.contains("permission denied or database is read-only"));
    }

    #[test]
    fn test_parse_mssql_url() {
        let conf = parse_mssql_url("mssql://sa:SecretPass123@localhost:1433/mydb").unwrap();
        assert_eq!(conf.get_addr(), "localhost:1433");

        let conf2 = parse_mssql_url("sqlserver://sa:SecretPass123@10.0.0.5:1433").unwrap();
        assert_eq!(conf2.get_addr(), "10.0.0.5:1433");

        let conf3 = parse_mssql_url("localhost:1433").unwrap();
        assert_eq!(conf3.get_addr(), "localhost:1433");
    }

    #[test]
    fn test_parse_mssql_url_with_at_in_password() {
        let conf = parse_mssql_url("mssql://sa:Password@123@localhost:1433/master").unwrap();
        assert_eq!(conf.get_addr(), "localhost:1433");
    }

    #[test]
    fn test_mssql_type_and_quoting() {
        assert_eq!(DbType::from_url("mssql://localhost"), DbType::Mssql);
        assert_eq!(DbType::from_url("sqlserver://localhost"), DbType::Mssql);
    }
}
