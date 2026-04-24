use sqlx::{any::AnyRow, AnyPool, Column, Row};

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
            DbType::Sqlite => {
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
            }
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
                let sql = format!("SELECT * FROM \"{}\" LIMIT {} OFFSET {}", table, limit, offset);
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
            return Ok(QueryResult { columns: vec![], rows: vec![] });
        }
        let columns: Vec<String> = col_rows.iter().map(|r| row_cell(r, 0)).collect();
        let select = columns
            .iter()
            .map(|c| format!("\"{}\"::text", c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {} FROM \"{}\" LIMIT {} OFFSET {}",
            select, table, limit, offset
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        let data = rows.iter().map(|r| row_to_strings(r)).collect();
        Ok(QueryResult { columns, rows: data })
    }

    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, sqlx::Error> {
        if self.db_type == DbType::Postgres {
            let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
            if rows.is_empty() {
                return Ok(QueryResult { columns: vec![], rows: vec![] });
            }
            let columns: Vec<String> =
                rows[0].columns().iter().map(|c| c.name().to_string()).collect();
            // Try native decode; fall back to text-cast subquery on type error
            if row_to_strings_checked(&rows[0]).is_ok() {
                let data = rows.iter().map(|r| row_to_strings(r)).collect();
                return Ok(QueryResult { columns, rows: data });
            }
            let select = columns
                .iter()
                .map(|c| format!("\"{}\"::text", c))
                .collect::<Vec<_>>()
                .join(", ");
            let cast_sql = format!("SELECT {} FROM ({}) __db_eye_q", select, sql);
            let rows2 = sqlx::query(&cast_sql).fetch_all(&self.pool).await?;
            let data = rows2.iter().map(|r| row_to_strings(r)).collect();
            return Ok(QueryResult { columns, rows: data });
        }

        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        if rows.is_empty() {
            return Ok(QueryResult { columns: vec![], rows: vec![] });
        }
        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let data = rows.iter().map(|r| row_to_strings(r)).collect();
        Ok(QueryResult { columns, rows: data })
    }

    pub async fn count_rows(&self, table: &str) -> Result<i64, sqlx::Error> {
        let sql = match self.db_type {
            DbType::Postgres => format!("SELECT COUNT(*)::text FROM \"{}\"", table),
            _ => format!("SELECT COUNT(*) FROM \"{}\"", table),
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

fn row_cell(row: &AnyRow, i: usize) -> String {
    row.try_get::<String, _>(i)
        .or_else(|_| row.try_get::<i64, _>(i).map(|v| v.to_string()))
        .unwrap_or_default()
}

fn row_to_strings_checked(row: &AnyRow) -> Result<Vec<String>, ()> {
    let mut result = Vec::with_capacity(row.columns().len());
    for i in 0..row.columns().len() {
        let val = row.try_get::<String, _>(i)
            .or_else(|_| row.try_get::<i64, _>(i).map(|v| v.to_string()))
            .or_else(|_| row.try_get::<f64, _>(i).map(|v| v.to_string()))
            .or_else(|_| row.try_get::<bool, _>(i).map(|v| v.to_string()))
            .or_else(|_| row.try_get::<Vec<u8>, _>(i).map(|v| format!("<blob {}b>", v.len())));
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
                .or_else(|_| row.try_get::<f64, _>(i).map(|v| format!("{:.4}", v).trim_end_matches('0').trim_end_matches('.').to_string()))
                .or_else(|_| row.try_get::<bool, _>(i).map(|v| v.to_string()))
                .or_else(|_| row.try_get::<Vec<u8>, _>(i).map(|v| format!("<blob {}b>", v.len())))
                .unwrap_or_else(|_| "NULL".to_string())
        })
        .collect()
}
