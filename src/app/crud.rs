use crate::db::{ColumnInfo, NULL_DISPLAY};

#[derive(Clone, PartialEq)]
pub enum CrudMode {
    Insert,
    Update,
}

pub struct CrudForm {
    pub table: String,
    pub schema: Option<String>,
    pub columns: Vec<ColumnInfo>,
    pub values: Vec<String>,
    pub fk_hints: Vec<Vec<String>>,
    pub fk_hint_index: usize,
    pub pk_values: Vec<(String, String)>,
    pub active_field: usize,
    pub mode: CrudMode,
    pub ident_quote: char,
    pub numbered_placeholders: bool,
}

pub struct CrudStatement {
    pub sql: String,
    pub values: Vec<Option<String>>,
}

impl CrudForm {
    pub fn build_sql(&self) -> String {
        let table_ident = if let Some(schema) = &self.schema {
            format!(
                "{}.{}",
                self.quote_ident(schema),
                self.quote_ident(&self.table)
            )
        } else {
            self.quote_ident(&self.table)
        };

        match self.mode {
            CrudMode::Insert => {
                let mut cols = Vec::new();
                let mut vals = Vec::new();
                for (col, value) in self.columns.iter().zip(self.values.iter()) {
                    if !col.is_binary() && (!col.is_pk || !value.is_empty()) {
                        cols.push(self.quote_ident(&col.name));
                        vals.push(Self::sql_value(value));
                    }
                }
                if cols.is_empty() {
                    format!("INSERT INTO {} DEFAULT VALUES", table_ident)
                } else {
                    format!(
                        "INSERT INTO {} ({}) VALUES ({})",
                        table_ident,
                        cols.join(", "),
                        vals.join(", ")
                    )
                }
            }
            CrudMode::Update => {
                let sets: Vec<String> = self
                    .columns
                    .iter()
                    .zip(self.values.iter())
                    .filter(|(c, _)| !c.is_pk && !c.is_binary())
                    .map(|(c, v)| format!("{} = {}", self.quote_ident(&c.name), Self::sql_value(v)))
                    .collect();
                format!(
                    "UPDATE {} SET {} WHERE {}",
                    table_ident,
                    sets.join(", "),
                    self.pk_literal_conditions()
                )
            }
        }
    }

    pub fn build_statement(&self) -> CrudStatement {
        let table_ident = if let Some(schema) = &self.schema {
            format!(
                "{}.{}",
                self.quote_ident(schema),
                self.quote_ident(&self.table)
            )
        } else {
            self.quote_ident(&self.table)
        };

        match self.mode {
            CrudMode::Insert => {
                let mut cols = Vec::new();
                let mut placeholders = Vec::new();
                let mut values = Vec::new();
                for (col, value) in self.columns.iter().zip(self.values.iter()) {
                    if !col.is_binary() && (!col.is_pk || !value.is_empty()) {
                        cols.push(self.quote_ident(&col.name));
                        values.push(Self::form_value(value));
                        placeholders.push(self.placeholder(values.len()));
                    }
                }
                let sql = if cols.is_empty() {
                    format!("INSERT INTO {} DEFAULT VALUES", table_ident)
                } else {
                    format!(
                        "INSERT INTO {} ({}) VALUES ({})",
                        table_ident,
                        cols.join(", "),
                        placeholders.join(", ")
                    )
                };
                CrudStatement { sql, values }
            }
            CrudMode::Update => {
                let mut sets = Vec::new();
                let mut values = Vec::new();
                for (col, value) in self.columns.iter().zip(self.values.iter()) {
                    if !col.is_pk && !col.is_binary() {
                        values.push(Self::form_value(value));
                        sets.push(format!(
                            "{} = {}",
                            self.quote_ident(&col.name),
                            self.placeholder(values.len())
                        ));
                    }
                }
                let where_clause = self.pk_placeholder_conditions(&mut values);
                let sql = format!(
                    "UPDATE {} SET {} WHERE {}",
                    table_ident,
                    sets.join(", "),
                    where_clause
                );
                CrudStatement { sql, values }
            }
        }
    }

    fn pk_literal_conditions(&self) -> String {
        self.pk_values
            .iter()
            .map(|(name, value)| format!("{} = {}", self.quote_ident(name), Self::sql_value(value)))
            .collect::<Vec<_>>()
            .join(" AND ")
    }

    fn pk_placeholder_conditions(&self, values: &mut Vec<Option<String>>) -> String {
        self.pk_values
            .iter()
            .map(|(name, value)| {
                values.push(Self::form_value(value));
                format!(
                    "{} = {}",
                    self.quote_ident(name),
                    self.placeholder(values.len())
                )
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    }

    fn quote_ident(&self, ident: &str) -> String {
        let doubled = format!("{}{}", self.ident_quote, self.ident_quote);
        format!(
            "{}{}{}",
            self.ident_quote,
            ident.replace(self.ident_quote, &doubled),
            self.ident_quote
        )
    }

    fn placeholder(&self, index: usize) -> String {
        if self.numbered_placeholders {
            format!("${index}")
        } else {
            "?".to_string()
        }
    }

    pub(super) fn form_value(value: &str) -> Option<String> {
        if value.is_empty() || value.eq_ignore_ascii_case("\\null") || value == NULL_DISPLAY {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn sql_value(value: &str) -> String {
        if value.is_empty() || value.eq_ignore_ascii_case("\\null") || value == NULL_DISPLAY {
            "NULL".to_string()
        } else {
            format!("'{}'", value.replace('\'', "''"))
        }
    }

    pub fn validate_values(&self) -> Result<(), String> {
        for (index, (col, value)) in self.columns.iter().zip(self.values.iter()).enumerate() {
            if self.mode == CrudMode::Update && col.is_pk {
                continue;
            }
            if self.mode == CrudMode::Insert && col.is_pk && value.is_empty() {
                continue;
            }
            col.validate_input(value)
                .map_err(|msg| format!("{}: {}", index + 1, msg))?;
        }
        Ok(())
    }

    pub fn editable_indices(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_binary() && (!c.is_pk || self.mode == CrudMode::Insert))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn next_field(&mut self) {
        let editable = self.editable_indices();
        if let Some(pos) = editable.iter().position(|&i| i == self.active_field) {
            self.active_field = editable[(pos + 1) % editable.len()];
        } else if !editable.is_empty() {
            self.active_field = editable[0];
        }
        self.fk_hint_index = 0;
    }

    pub fn prev_field(&mut self) {
        let editable = self.editable_indices();
        if let Some(pos) = editable.iter().position(|&i| i == self.active_field) {
            self.active_field = editable[if pos == 0 {
                editable.len() - 1
            } else {
                pos - 1
            }];
        } else if !editable.is_empty() {
            self.active_field = editable[0];
        }
        self.fk_hint_index = 0;
    }

    /// Cycles the FK dropdown for the active field and writes the selected value into it.
    /// No-op if the active field isn't a FK column or has no known values to pick from.
    pub fn cycle_fk_hint(&mut self, forward: bool) {
        let hints = match self.fk_hints.get(self.active_field) {
            Some(h) if !h.is_empty() => h,
            _ => return,
        };
        self.fk_hint_index = if forward {
            (self.fk_hint_index + 1) % hints.len()
        } else if self.fk_hint_index == 0 {
            hints.len() - 1
        } else {
            self.fk_hint_index - 1
        };
        if let Some(value) = hints.get(self.fk_hint_index)
            && let Some(slot) = self.values.get_mut(self.active_field)
        {
            *slot = value.clone();
        }
    }
}

pub struct DeleteConfirm {
    pub sql: String,
    pub values: Vec<Option<String>>,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbClient;

    fn col(name: &str, is_pk: bool) -> ColumnInfo {
        let pk_order = if is_pk { 1 } else { 0 };
        ColumnInfo {
            name: name.to_string(),
            data_type: "text".to_string(),
            is_pk,
            pk_order,
            fk_table: None,
            fk_column: None,
            is_nullable: true,
            default_value: None,
        }
    }

    fn form(mode: CrudMode) -> CrudForm {
        CrudForm {
            table: "users".to_string(),
            schema: None,
            columns: vec![col("id", true), col("name", false), col("email", false)],
            values: vec!["1".to_string(), "Ada".to_string(), "\\null".to_string()],
            fk_hints: vec![vec![], vec![], vec![]],
            fk_hint_index: 0,
            pk_values: vec![("id".to_string(), "1".to_string())],
            active_field: 1,
            mode,
            ident_quote: '"',
            numbered_placeholders: false,
        }
    }

    #[test]
    fn insert_statement_uses_bind_placeholders() {
        let stmt = form(CrudMode::Insert).build_statement();
        assert_eq!(
            stmt.sql,
            "INSERT INTO \"users\" (\"id\", \"name\", \"email\") VALUES (?, ?, ?)"
        );
        assert_eq!(
            stmt.values,
            vec![Some("1".to_string()), Some("Ada".to_string()), None]
        );
    }

    #[test]
    fn update_statement_uses_bind_placeholders_for_values_and_pk() {
        let stmt = form(CrudMode::Update).build_statement();
        assert_eq!(
            stmt.sql,
            "UPDATE \"users\" SET \"name\" = ?, \"email\" = ? WHERE \"id\" = ?"
        );
        assert_eq!(
            stmt.values,
            vec![Some("Ada".to_string()), None, Some("1".to_string())]
        );
    }

    #[test]
    fn literal_null_string_is_not_database_null() {
        assert_eq!(CrudForm::form_value("NULL"), Some("NULL".to_string()));
        assert_eq!(CrudForm::form_value("\\null"), None);
        assert_eq!(CrudForm::form_value(NULL_DISPLAY), None);
    }

    #[test]
    fn validates_numeric_and_boolean_inputs() {
        let mut form = form(CrudMode::Insert);
        form.columns[1].data_type = "integer".to_string();
        form.values[1] = "not-a-number".to_string();
        assert!(form.validate_values().is_err());

        form.columns[1].data_type = "boolean".to_string();
        form.values[1] = "true".to_string();
        assert!(form.validate_values().is_ok());
    }

    #[test]
    fn postgres_statement_uses_numbered_placeholders() {
        let mut form = form(CrudMode::Update);
        form.numbered_placeholders = true;
        let stmt = form.build_statement();
        assert_eq!(
            stmt.sql,
            "UPDATE \"users\" SET \"name\" = $1, \"email\" = $2 WHERE \"id\" = $3"
        );
    }

    #[test]
    fn composite_pk_update_statement_uses_all_pk_columns() {
        let form = CrudForm {
            table: "memberships".to_string(),
            schema: None,
            columns: vec![
                ColumnInfo {
                    name: "tenant_id".to_string(),
                    data_type: "text".to_string(),
                    is_pk: true,
                    pk_order: 1,
                    fk_table: None,
                    fk_column: None,
                    is_nullable: false,
                    default_value: None,
                },
                ColumnInfo {
                    name: "user_id".to_string(),
                    data_type: "text".to_string(),
                    is_pk: true,
                    pk_order: 2,
                    fk_table: None,
                    fk_column: None,
                    is_nullable: false,
                    default_value: None,
                },
                col("name", false),
            ],
            values: vec!["t1".to_string(), "u1".to_string(), "Ada".to_string()],
            fk_hints: vec![vec![], vec![], vec![]],
            fk_hint_index: 0,
            pk_values: vec![
                ("tenant_id".to_string(), "t1".to_string()),
                ("user_id".to_string(), "u1".to_string()),
            ],
            active_field: 2,
            mode: CrudMode::Update,
            ident_quote: '"',
            numbered_placeholders: false,
        };
        let stmt = form.build_statement();
        assert_eq!(
            stmt.sql,
            "UPDATE \"memberships\" SET \"name\" = ? WHERE \"tenant_id\" = ? AND \"user_id\" = ?"
        );
        assert_eq!(
            stmt.values,
            vec![
                Some("Ada".to_string()),
                Some("t1".to_string()),
                Some("u1".to_string())
            ]
        );
    }

    #[tokio::test]
    async fn sqlite_crud_statements_execute_end_to_end() {
        let path =
            std::env::temp_dir().join(format!("db-eye-crud-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::File::create(&path).unwrap();
        let db = DbClient::connect(path.to_str().unwrap()).await.unwrap();
        sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT)")
            .execute(&db.pool)
            .await
            .unwrap();

        let insert = form(CrudMode::Insert).build_statement();
        assert_eq!(
            db.execute_write_with_values(&insert.sql, &insert.values)
                .await
                .unwrap(),
            1
        );

        let mut update_form = form(CrudMode::Update);
        update_form.values = vec![
            "1".to_string(),
            "Grace".to_string(),
            "g@example.test".to_string(),
        ];
        let update = update_form.build_statement();
        assert_eq!(
            db.execute_write_with_values(&update.sql, &update.values)
                .await
                .unwrap(),
            1
        );

        let result = db
            .execute_query("SELECT id, name, email FROM users WHERE id = 1")
            .await
            .unwrap();
        assert_eq!(
            result.rows,
            vec![vec![
                "1".to_string(),
                "Grace".to_string(),
                "g@example.test".to_string()
            ]]
        );

        assert_eq!(
            db.execute_write_with_values(
                "DELETE FROM users WHERE id = ?",
                &[Some("1".to_string())]
            )
            .await
            .unwrap(),
            1
        );
        let result = db.execute_query("SELECT id FROM users").await.unwrap();
        assert!(result.rows.is_empty());

        db.pool.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn sqlite_composite_pk_crud_executes_end_to_end() {
        let path = std::env::temp_dir().join(format!(
            "db-eye-composite-crud-test-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        std::fs::File::create(&path).unwrap();
        let db = DbClient::connect(path.to_str().unwrap()).await.unwrap();
        sqlx::query(
            "CREATE TABLE memberships (tenant_id TEXT, user_id TEXT, name TEXT, PRIMARY KEY (tenant_id, user_id))",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        let columns = db.get_columns(None, "memberships").await.unwrap();
        let pk_orders: Vec<(String, i64)> = columns
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| (c.name.clone(), c.pk_order))
            .collect();
        assert_eq!(
            pk_orders,
            vec![("tenant_id".to_string(), 1), ("user_id".to_string(), 2)]
        );

        let insert_form = CrudForm {
            table: "memberships".to_string(),
            schema: None,
            columns: columns.clone(),
            values: vec!["t1".to_string(), "u1".to_string(), "Ada".to_string()],
            fk_hints: vec![vec![], vec![], vec![]],
            fk_hint_index: 0,
            pk_values: vec![],
            active_field: 2,
            mode: CrudMode::Insert,
            ident_quote: '"',
            numbered_placeholders: false,
        };
        let insert = insert_form.build_statement();
        assert_eq!(
            db.execute_write_with_values(&insert.sql, &insert.values)
                .await
                .unwrap(),
            1
        );

        let update_form = CrudForm {
            table: "memberships".to_string(),
            schema: None,
            columns,
            values: vec!["t1".to_string(), "u1".to_string(), "Grace".to_string()],
            fk_hints: vec![vec![], vec![], vec![]],
            fk_hint_index: 0,
            pk_values: vec![
                ("tenant_id".to_string(), "t1".to_string()),
                ("user_id".to_string(), "u1".to_string()),
            ],
            active_field: 2,
            mode: CrudMode::Update,
            ident_quote: '"',
            numbered_placeholders: false,
        };
        let update = update_form.build_statement();
        assert_eq!(
            db.execute_write_with_values(&update.sql, &update.values)
                .await
                .unwrap(),
            1
        );

        let result = db
            .execute_query("SELECT name FROM memberships WHERE tenant_id = 't1' AND user_id = 'u1'")
            .await
            .unwrap();
        assert_eq!(result.rows, vec![vec!["Grace".to_string()]]);

        assert_eq!(
            db.execute_write_with_values(
                "DELETE FROM memberships WHERE tenant_id = ? AND user_id = ?",
                &[Some("t1".to_string()), Some("u1".to_string())],
            )
            .await
            .unwrap(),
            1
        );
        let result = db
            .execute_query("SELECT name FROM memberships")
            .await
            .unwrap();
        assert!(result.rows.is_empty());

        db.pool.close().await;
        let _ = std::fs::remove_file(path);
    }
}
