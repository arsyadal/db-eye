use sqlx::{AnyPool, Column, Row, any::AnyRow, error::ErrorKind};

#[derive(Clone, Copy, PartialEq)]
pub enum DbType {
    Sqlite,
    Postgres,
    Mysql,
}

impl DbType {
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("postgres") {
            DbType::Postgres
        } else if url.starts_with("mysql") {
            DbType::Mysql
        } else {
            DbType::Sqlite
        }
    }
}

pub struct DbClient {
    pub pool: AnyPool,
    pub db_type: DbType,
}

pub fn format_db_error(action: &str, error: &sqlx::Error) -> String {
    let prefix = if action.is_empty() {
        "Database error".to_string()
    } else {
        format!("{} failed", action)
    };

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
                ErrorKind::Other => classify_db_message(&prefix, db_error.message()),
                _ => classify_db_message(&prefix, db_error.message()),
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
        _ => classify_db_message(&prefix, &error.to_string()),
    }
}

fn classify_db_message(prefix: &str, message: &str) -> String {
    let lower = message.to_lowercase();
    let friendly = if lower.contains("foreign key") {
        Some("foreign key constraint failed; check referenced rows first")
    } else if lower.contains("unique") || lower.contains("duplicate") || lower.contains("1062") {
        Some("duplicate value violates a unique constraint")
    } else if lower.contains("not null") || lower.contains("cannot be null") {
        Some("required field cannot be NULL")
    } else if lower.contains("permission denied")
        || lower.contains("access denied")
        || lower.contains("readonly")
        || lower.contains("read-only")
        || lower.contains("attempt to write a readonly database")
    {
        Some("permission denied or database is read-only")
    } else if lower.contains("syntax error") || lower.contains("sql syntax") {
        Some("SQL syntax error; check the query text")
    } else if lower.contains("no such table")
        || lower.contains("doesn't exist")
        || lower.contains("does not exist")
        || lower.contains("unknown table")
    {
        Some("table or database object not found")
    } else if lower.contains("no such column") || lower.contains("unknown column") {
        Some("column not found")
    } else if lower.contains("connection refused") || lower.contains("could not connect") {
        Some("could not connect to database server")
    } else {
        None
    };

    match friendly {
        Some(summary) => format!("{prefix}: {summary} ({message})"),
        None => format!("{prefix}: {message}"),
    }
}

impl DbClient {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        sqlx::any::install_default_drivers();
        let normalized = match DbType::from_url(url) {
            DbType::Sqlite => {
                if url.starts_with("sqlite:") {
                    url.to_string()
                } else {
                    format!("sqlite:{}", url)
                }
            }
            _ => url.to_string(),
        };
        let db_type = DbType::from_url(&normalized);
        let pool = AnyPool::connect(&normalized).await?;
        Ok(Self { pool, db_type })
    }

    pub fn identifier_quote(&self) -> char {
        match self.db_type {
            DbType::Mysql => '`',
            _ => '"',
        }
    }

    pub fn quote_ident(&self, ident: &str) -> String {
        let quote = self.identifier_quote();
        let doubled = format!("{quote}{quote}");
        format!("{quote}{}{quote}", ident.replace(quote, &doubled))
    }

    pub fn uses_numbered_placeholders(&self) -> bool {
        self.db_type == DbType::Postgres
    }

    pub async fn list_databases(&self) -> Result<Vec<String>, sqlx::Error> {
        let sql = match self.db_type {
            DbType::Postgres => {
                "SELECT datname::text FROM pg_database WHERE datistemplate = false ORDER BY datname"
            }
            DbType::Mysql => "SHOW DATABASES",
            DbType::Sqlite => "SELECT 'main'",
        };
        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
    }

    pub async fn list_schemas(&self) -> Result<Vec<String>, sqlx::Error> {
        match self.db_type {
            DbType::Postgres => {
                let sql = "SELECT schema_name::text FROM information_schema.schemata \
                           WHERE schema_name NOT LIKE 'pg_%' AND schema_name != 'information_schema' \
                           ORDER BY schema_name";
                let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
                Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
            }
            _ => Ok(vec![]),
        }
    }

    pub async fn list_tables(&self, schema: Option<&str>) -> Result<Vec<TableEntry>, sqlx::Error> {
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
        };
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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

    pub async fn query_table(
        &self,
        schema: Option<&str>,
        table: &str,
        limit: u32,
        offset: u32,
        sort: Option<(&str, bool)>,
    ) -> Result<QueryResult, sqlx::Error> {
        match self.db_type {
            DbType::Postgres => self.pg_text_query(schema, table, limit, offset, sort).await,
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
    ) -> Result<QueryResult, sqlx::Error> {
        let schema_name = schema.unwrap_or("public");
        let cols_sql = format!(
            "SELECT column_name::text FROM information_schema.columns \
             WHERE table_schema = '{}' AND table_name = '{}' \
             ORDER BY ordinal_position",
            schema_name, table
        );
        let col_rows = sqlx::query(&cols_sql).fetch_all(&self.pool).await?;
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
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let data = rows.iter().map(row_to_strings).collect();
        Ok(QueryResult {
            columns,
            rows: data,
            rows_affected: 0,
        })
    }

    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, sqlx::Error> {
        if !Self::is_read_only_sql(sql) {
            let result = sqlx::query(sql).execute(&self.pool).await?;
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: result.rows_affected(),
            });
        }

        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        self.rows_to_query_result(sql, rows, 0).await
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
            let rows2 = sqlx::query(&cast_sql).fetch_all(&self.pool).await?;
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

    pub fn is_read_only_sql(sql: &str) -> bool {
        let sql = sql.trim_start();
        if sql.is_empty() {
            return true;
        }
        let sql = sql
            .trim_start_matches(|c: char| c == '(' || c == ';' || c.is_whitespace())
            .to_lowercase();
        matches!(
            sql.split_whitespace().next(),
            Some("select" | "with" | "show" | "describe" | "desc" | "explain")
        )
    }

    pub async fn get_columns(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, sqlx::Error> {
        match self.db_type {
            DbType::Sqlite => {
                let sql = format!("PRAGMA table_info(\"{}\")", table);
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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
                    .fetch_all(&self.pool)
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
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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
                    .fetch_all(&self.pool)
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
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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
                    .fetch_all(&self.pool)
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
        }
    }

    pub async fn get_fk_values(
        &self,
        schema: Option<&str>,
        ref_table: &str,
        ref_column: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
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
            _ => format!(
                "SELECT DISTINCT {} FROM {} ORDER BY 1 LIMIT 50",
                self.quote_ident(ref_column),
                self.quote_ident(ref_table)
            ),
        };
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
    }

    pub async fn execute_write_with_values(
        &self,
        sql: &str,
        values: &[Option<String>],
    ) -> Result<u64, sqlx::Error> {
        let mut query = sqlx::query(sql);
        for value in values {
            query = query.bind(value.clone());
        }
        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn table_stats(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> Result<TableStats, sqlx::Error> {
        match self.db_type {
            DbType::Sqlite => {
                let sql = format!("PRAGMA index_list(\"{}\")", table);
                let idx_rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
                let mut indexes = Vec::new();
                for idx_row in &idx_rows {
                    let name = row_cell(idx_row, 1);
                    let unique = row_cell(idx_row, 2) == "1";
                    let cols_sql = format!("PRAGMA index_info(\"{}\")", name);
                    let col_rows = sqlx::query(&cols_sql)
                        .fetch_all(&self.pool)
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
                let idx_rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
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
                    .fetch_one(&self.pool)
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
                let idx_rows = sqlx::query(&idx_sql).fetch_all(&self.pool).await?;
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
                    .fetch_one(&self.pool)
                    .await
                    .ok()
                    .and_then(|r| row_cell(&r, 0).parse::<i64>().ok())
                    .map(format_bytes);
                Ok(TableStats {
                    indexes,
                    size_label,
                })
            }
        }
    }

    pub async fn count_rows(&self, schema: Option<&str>, table: &str) -> Result<i64, sqlx::Error> {
        let sql = match self.db_type {
            DbType::Postgres => {
                let schema_name = schema.unwrap_or("public");
                format!(
                    "SELECT COUNT(*)::text FROM {}.{}",
                    self.quote_ident(schema_name),
                    self.quote_ident(table)
                )
            }
            _ => format!("SELECT COUNT(*) FROM {}", self.quote_ident(table)),
        };
        let row = sqlx::query(&sql).fetch_one(&self.pool).await?;
        row.try_get::<String, _>(0)
            .map(|s| s.parse::<i64>().unwrap_or(0))
            .or_else(|_| row.try_get::<i64, _>(0))
            .or_else(|_| row.try_get::<i32, _>(0).map(|v| v as i64))
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

fn is_integer_type(kind: &str) -> bool {
    kind.contains("int") || matches!(kind, "serial" | "bigserial" | "smallserial")
}

fn is_float_type(kind: &str) -> bool {
    kind.contains("real")
        || kind.contains("double")
        || kind.contains("float")
        || kind.contains("numeric")
        || kind.contains("decimal")
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

fn row_cell(row: &AnyRow, i: usize) -> String {
    row.try_get::<String, _>(i)
        .or_else(|_| row.try_get::<i64, _>(i).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<i32, _>(i).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<i16, _>(i).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<f64, _>(i).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<f32, _>(i).map(|v| v.to_string()))
        .or_else(|_| row.try_get::<bool, _>(i).map(|v| v.to_string()))
        .or_else(|_| {
            row.try_get::<Vec<u8>, _>(i)
                .map(|v| format!("<blob {}b>", v.len()))
        })
        .unwrap_or_default()
}

fn row_to_strings_checked(row: &AnyRow) -> Result<Vec<String>, ()> {
    let mut result = Vec::with_capacity(row.columns().len());
    for i in 0..row.columns().len() {
        let val = row
            .try_get::<String, _>(i)
            .or_else(|_| row.try_get::<i64, _>(i).map(|v| v.to_string()))
            .or_else(|_| row.try_get::<f64, _>(i).map(|v| v.to_string()))
            .or_else(|_| row.try_get::<bool, _>(i).map(|v| v.to_string()))
            .or_else(|_| {
                row.try_get::<Vec<u8>, _>(i)
                    .map(|v| format!("<blob {}b>", v.len()))
            });
        match val {
            Ok(v) => result.push(v),
            Err(_) => return Err(()),
        }
    }
    Ok(result)
}

fn row_to_strings(row: &AnyRow) -> Vec<String> {
    (0..row.columns().len())
        .map(|i| {
            row.try_get::<String, _>(i)
                .or_else(|_| row.try_get::<i64, _>(i).map(|v| v.to_string()))
                .or_else(|_| {
                    row.try_get::<f64, _>(i).map(|v| {
                        format!("{:.4}", v)
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_string()
                    })
                })
                .or_else(|_| row.try_get::<bool, _>(i).map(|v| v.to_string()))
                .or_else(|_| {
                    row.try_get::<Vec<u8>, _>(i)
                        .map(|v| format!("<blob {}b>", v.len()))
                })
                .unwrap_or_else(|_| NULL_DISPLAY.to_string())
        })
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
            .execute(&db.pool)
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
        db.pool.close().await;
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
}
