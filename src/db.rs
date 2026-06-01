use sqlx::{AnyPool, Column, Row, any::AnyRow};

#[derive(Clone, PartialEq)]
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

    pub async fn list_tables(&self) -> Result<Vec<String>, sqlx::Error> {
        let sql = match self.db_type {
            DbType::Sqlite => "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
            DbType::Postgres => {
                "SELECT tablename::text FROM pg_tables WHERE schemaname='public' ORDER BY tablename"
            }
            DbType::Mysql => {
                "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() ORDER BY table_name"
            }
        };
        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| row_cell(r, 0)).collect())
    }

    pub async fn query_table(
        &self,
        table: &str,
        limit: u32,
        offset: u32,
    ) -> Result<QueryResult, sqlx::Error> {
        match self.db_type {
            DbType::Postgres => self.pg_text_query(table, limit, offset).await,
            _ => {
                let sql = format!(
                    "SELECT * FROM {} LIMIT {} OFFSET {}",
                    self.quote_ident(table),
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
        table: &str,
        limit: u32,
        offset: u32,
    ) -> Result<QueryResult, sqlx::Error> {
        let cols_sql = format!(
            "SELECT column_name::text FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = '{}' \
             ORDER BY ordinal_position",
            table
        );
        let col_rows = sqlx::query(&cols_sql).fetch_all(&self.pool).await?;
        if col_rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
            });
        }
        let columns: Vec<String> = col_rows.iter().map(|r| row_cell(r, 0)).collect();
        let select = columns
            .iter()
            .map(|c| format!("{}::text", self.quote_ident(c)))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {} FROM {} LIMIT {} OFFSET {}",
            select,
            self.quote_ident(table),
            limit,
            offset
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let data = rows.iter().map(|r| row_to_strings(r)).collect();
        Ok(QueryResult {
            columns,
            rows: data,
        })
    }

    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, sqlx::Error> {
        if self.db_type == DbType::Postgres {
            let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
            if rows.is_empty() {
                return Ok(QueryResult {
                    columns: vec![],
                    rows: vec![],
                });
            }
            let columns: Vec<String> = rows[0]
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect();
            // Try native decode; fall back to text-cast subquery on type error
            if row_to_strings_checked(&rows[0]).is_ok() {
                let data = rows.iter().map(|r| row_to_strings(r)).collect();
                return Ok(QueryResult {
                    columns,
                    rows: data,
                });
            }
            let select = columns
                .iter()
                .map(|c| format!("{}::text", self.quote_ident(c)))
                .collect::<Vec<_>>()
                .join(", ");
            let cast_sql = format!("SELECT {} FROM ({}) __db_eye_q", select, sql);
            let rows2 = sqlx::query(&cast_sql).fetch_all(&self.pool).await?;
            let data = rows2.iter().map(|r| row_to_strings(r)).collect();
            return Ok(QueryResult {
                columns,
                rows: data,
            });
        }

        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        if rows.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
            });
        }
        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let data = rows.iter().map(|r| row_to_strings(r)).collect();
        Ok(QueryResult {
            columns,
            rows: data,
        })
    }

    pub async fn get_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, sqlx::Error> {
        match self.db_type {
            DbType::Sqlite => {
                let sql = format!("PRAGMA table_info(\"{}\")", table);
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
                let mut cols: Vec<ColumnInfo> = rows
                    .iter()
                    .map(|r| {
                        let name = row_cell(r, 1);
                        let data_type = row_cell(r, 2);
                        let pk = r.try_get::<i64, _>(5).unwrap_or(0);
                        ColumnInfo {
                            name,
                            data_type,
                            is_pk: pk > 0,
                            fk_table: None,
                            fk_column: None,
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
                let sql = format!(
                    "SELECT c.column_name::text, c.data_type::text, \
                     CASE WHEN pk.column_name IS NOT NULL THEN 1 ELSE 0 END \
                     FROM information_schema.columns c \
                     LEFT JOIN ( \
                         SELECT kcu.column_name \
                         FROM information_schema.table_constraints tc \
                         JOIN information_schema.key_column_usage kcu \
                           ON tc.constraint_name = kcu.constraint_name \
                          AND tc.table_name = kcu.table_name \
                         WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_name = '{}' \
                     ) pk ON c.column_name = pk.column_name \
                     WHERE c.table_name = '{}' ORDER BY c.ordinal_position",
                    table, table
                );
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
                let mut cols: Vec<ColumnInfo> = rows
                    .iter()
                    .map(|r| ColumnInfo {
                        name: row_cell(r, 0),
                        data_type: row_cell(r, 1),
                        is_pk: row_cell(r, 2) == "1",
                        fk_table: None,
                        fk_column: None,
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
                     WHERE kcu.table_schema = 'public' AND kcu.table_name = '{}'",
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
            DbType::Mysql => {
                let sql = format!(
                    "SELECT column_name, data_type, IF(column_key='PRI','1','0') \
                     FROM information_schema.columns \
                     WHERE table_schema = DATABASE() AND table_name = '{}' \
                     ORDER BY ordinal_position",
                    table
                );
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
                let mut cols: Vec<ColumnInfo> = rows
                    .iter()
                    .map(|r| ColumnInfo {
                        name: row_cell(r, 0),
                        data_type: row_cell(r, 1),
                        is_pk: row_cell(r, 2) == "1",
                        fk_table: None,
                        fk_column: None,
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
        ref_table: &str,
        ref_column: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let sql = match self.db_type {
            DbType::Postgres => format!(
                "SELECT DISTINCT {}::text FROM {} ORDER BY 1 LIMIT 50",
                self.quote_ident(ref_column),
                self.quote_ident(ref_table)
            ),
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

    pub async fn count_rows(&self, table: &str) -> Result<i64, sqlx::Error> {
        let sql = match self.db_type {
            DbType::Postgres => format!("SELECT COUNT(*)::text FROM {}", self.quote_ident(table)),
            _ => format!("SELECT COUNT(*) FROM {}", self.quote_ident(table)),
        };
        let row = sqlx::query(&sql).fetch_one(&self.pool).await?;
        row.try_get::<String, _>(0)
            .map(|s| s.parse::<i64>().unwrap_or(0))
            .or_else(|_| row.try_get::<i64, _>(0))
            .or_else(|_| row.try_get::<i32, _>(0).map(|v| v as i64))
    }
}

pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_pk: bool,
    pub fk_table: Option<String>,
    pub fk_column: Option<String>,
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
                .unwrap_or_else(|_| "NULL".to_string())
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
            vec![vec!["Ada's".to_string(), "NULL".to_string()]]
        );
        db.pool.close().await;
        let _ = std::fs::remove_file(path);
    }
}
