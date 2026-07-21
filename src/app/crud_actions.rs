use super::{App, CrudForm, CrudMode, DeleteConfirm, Screen};
use crate::db::{ColumnInfo, format_db_error};
use crossterm::event::KeyCode;

impl App {
    pub(super) async fn open_insert_form(&mut self) {
        if self.read_only {
            self.read_only_block();
            return;
        }
        let table = match self.current_tab().and_then(|t| t.tables.get(t.table_index)) {
            Some(t) => t.name.clone(),
            None => return,
        };
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let schema = tab.current_schema().map(|s| s.to_string());
            match tab.db.get_columns(schema.as_deref(), &table).await {
                Ok(columns) => {
                    let mut fk_hints: Vec<Vec<String>> = Vec::new();
                    for col in &columns {
                        let hints = if let (Some(ref_table), Some(ref_col)) =
                            (&col.fk_table, &col.fk_column)
                        {
                            tab.db
                                .get_fk_values(schema.as_deref(), ref_table, ref_col)
                                .await
                                .unwrap_or_default()
                        } else {
                            vec![]
                        };
                        fk_hints.push(hints);
                    }
                    let values = columns.iter().map(|_| String::new()).collect();
                    self.crud_form = Some(CrudForm {
                        table,
                        schema,
                        active_field: columns
                            .iter()
                            .position(|c| !c.is_pk && !c.is_binary())
                            .unwrap_or(0),
                        columns,
                        values,
                        fk_hints,
                        pk_values: vec![],
                        mode: CrudMode::Insert,
                        ident_quote: tab.db.identifier_quote(),
                        numbered_placeholders: tab.db.uses_numbered_placeholders(),
                    });
                    self.screen = Screen::CrudForm;
                    self.status = "Tab/↑↓: fields  Enter: save  Esc: cancel".into();
                }
                Err(e) => self.status = format_db_error("Loading columns", &e),
            }
        }
    }

    pub(super) async fn open_update_form(&mut self) {
        if self.read_only {
            self.read_only_block();
            return;
        }
        let (table, row_data) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.name.clone(),
                None => return,
            };
            let row = match tab.display_rows().get(tab.selected_row) {
                Some(r) => r.clone(),
                None => {
                    self.status = "No row selected".into();
                    return;
                }
            };
            (table, row)
        };
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let schema = tab.current_schema().map(|s| s.to_string());
            match tab.db.get_columns(schema.as_deref(), &table).await {
                Ok(columns) => {
                    if !columns.iter().any(|c| c.is_pk) {
                        self.status = "Update requires a primary key".into();
                        return;
                    }
                    if !columns.iter().any(|c| !c.is_pk && !c.is_binary()) {
                        self.status = "No editable columns".into();
                        return;
                    }
                    let mut fk_hints: Vec<Vec<String>> = Vec::new();
                    for col in &columns {
                        let hints = if let (Some(ref_table), Some(ref_col)) =
                            (&col.fk_table, &col.fk_column)
                        {
                            tab.db
                                .get_fk_values(schema.as_deref(), ref_table, ref_col)
                                .await
                                .unwrap_or_default()
                        } else {
                            vec![]
                        };
                        fk_hints.push(hints);
                    }
                    let result = tab.result.as_ref().unwrap();
                    let values: Vec<String> = columns
                        .iter()
                        .map(|col| {
                            result
                                .columns
                                .iter()
                                .position(|c| c == &col.name)
                                .and_then(|i| row_data.get(i))
                                .cloned()
                                .unwrap_or_default()
                        })
                        .collect();
                    let mut pk_columns: Vec<ColumnInfo> =
                        columns.iter().filter(|c| c.is_pk).cloned().collect();
                    pk_columns.sort_by_key(|c| c.pk_order);
                    let pk_values: Vec<(String, String)> = pk_columns
                        .iter()
                        .map(|pk| {
                            let val = result
                                .columns
                                .iter()
                                .position(|c| c == &pk.name)
                                .and_then(|i| row_data.get(i))
                                .cloned()
                                .unwrap_or_default();
                            (pk.name.clone(), val)
                        })
                        .collect();
                    self.crud_form = Some(CrudForm {
                        active_field: columns
                            .iter()
                            .position(|c| !c.is_pk && !c.is_binary())
                            .unwrap_or(0),
                        table,
                        schema,
                        columns,
                        values,
                        fk_hints,
                        pk_values,
                        mode: CrudMode::Update,
                        ident_quote: tab.db.identifier_quote(),
                        numbered_placeholders: tab.db.uses_numbered_placeholders(),
                    });
                    self.screen = Screen::CrudForm;
                    self.status = "Tab/↑↓: fields  Enter: save  Esc: cancel".into();
                }
                Err(e) => self.status = format_db_error("Loading columns", &e),
            }
        }
    }

    pub(super) async fn open_delete_confirm(&mut self) {
        if self.read_only {
            self.read_only_block();
            return;
        }
        let (table, row_data, columns) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.name.clone(),
                None => return,
            };
            let row = match tab.display_rows().get(tab.selected_row) {
                Some(r) => r.clone(),
                None => {
                    self.status = "No row selected".into();
                    return;
                }
            };
            let cols = tab
                .result
                .as_ref()
                .map(|r| r.columns.clone())
                .unwrap_or_default();
            (table, row, cols)
        };
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let schema = tab.current_schema().map(|s| s.to_string());
            match tab.db.get_columns(schema.as_deref(), &table).await {
                Ok(col_info) => {
                    let mut pk_columns: Vec<ColumnInfo> =
                        col_info.iter().filter(|c| c.is_pk).cloned().collect();
                    if pk_columns.is_empty() {
                        self.status = "Delete requires a primary key".into();
                        return;
                    }
                    pk_columns.sort_by_key(|c| c.pk_order);
                    let quote = tab.db.identifier_quote();
                    let quote_ident = |ident: &str| {
                        let doubled = format!("{quote}{quote}");
                        format!("{quote}{}{quote}", ident.replace(quote, &doubled))
                    };
                    let numbered_placeholders = tab.db.uses_numbered_placeholders();
                    let mut values = Vec::new();
                    let conditions: Vec<String> = pk_columns
                        .iter()
                        .enumerate()
                        .map(|(i, pk)| {
                            let pk_val = columns
                                .iter()
                                .position(|c| c == &pk.name)
                                .and_then(|idx| row_data.get(idx))
                                .cloned()
                                .unwrap_or_default();
                            values.push(CrudForm::form_value(&pk_val));
                            let placeholder = if numbered_placeholders {
                                format!("${}", i + 1)
                            } else {
                                "?".to_string()
                            };
                            format!("{} = {}", quote_ident(&pk.name), placeholder)
                        })
                        .collect();
                    let table_ident = if let Some(s) = &schema {
                        format!("{}.{}", quote_ident(s), quote_ident(&table))
                    } else {
                        quote_ident(&table)
                    };
                    let sql = format!(
                        "DELETE FROM {} WHERE {}",
                        table_ident,
                        conditions.join(" AND ")
                    );
                    let preview = row_data
                        .iter()
                        .take(3)
                        .map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.delete_confirm = Some(DeleteConfirm {
                        description: format!("Row: {}...", preview),
                        sql,
                        values,
                    });
                    self.screen = Screen::ConfirmDelete;
                }
                Err(e) => self.status = format_db_error("Loading columns", &e),
            }
        }
    }

    pub(super) async fn handle_crud_form(&mut self, key: KeyCode) {
        if self.read_only {
            self.crud_form = None;
            self.screen = Screen::Main;
            self.read_only_block();
            return;
        }
        match key {
            KeyCode::Tab | KeyCode::Down => {
                if let Some(ref mut form) = self.crud_form {
                    form.next_field();
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if let Some(ref mut form) = self.crud_form {
                    form.prev_field();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut form) = self.crud_form
                    && let Some(value) = form.values.get_mut(form.active_field)
                {
                    value.push(c);
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut form) = self.crud_form
                    && let Some(value) = form.values.get_mut(form.active_field)
                {
                    value.pop();
                }
            }
            KeyCode::Enter => {
                if let Some(error) = self
                    .crud_form
                    .as_ref()
                    .and_then(|f| f.validate_values().err())
                {
                    self.status = format!("Validation: {error}");
                    return;
                }
                let statement = self.crud_form.as_ref().map(|f| f.build_statement());
                if let Some(statement) = statement
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    match tab
                        .db
                        .execute_write_with_values(&statement.sql, &statement.values)
                        .await
                    {
                        Ok(rows) => {
                            let mode = self.crud_form.as_ref().map(|f| f.mode.clone());
                            let msg = match mode {
                                Some(CrudMode::Insert) => format!("{} row inserted", rows),
                                _ => format!("{} row updated", rows),
                            };
                            self.crud_form = None;
                            self.screen = Screen::Main;
                            self.load_table_data().await;
                            self.status = msg;
                        }
                        Err(e) => {
                            let action = match self.crud_form.as_ref().map(|f| f.mode.clone()) {
                                Some(CrudMode::Insert) => "Insert",
                                _ => "Update",
                            };
                            self.status = format_db_error(action, &e);
                        }
                    }
                }
            }
            KeyCode::Esc => {
                self.crud_form = None;
                self.screen = Screen::Main;
                self.status = "Cancelled".into();
            }
            _ => {}
        }
    }

    pub(super) async fn handle_confirm_delete(&mut self, key: KeyCode) {
        if self.read_only {
            self.delete_confirm = None;
            self.screen = Screen::Main;
            self.read_only_block();
            return;
        }
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let statement = self
                    .delete_confirm
                    .as_ref()
                    .map(|d| (d.sql.clone(), d.values.clone()));
                if let Some((sql, values)) = statement
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    match tab.db.execute_write_with_values(&sql, &values).await {
                        Ok(rows) => {
                            let msg = format!("{} row deleted", rows);
                            self.delete_confirm = None;
                            self.screen = Screen::Main;
                            self.load_table_data().await;
                            self.status = msg;
                        }
                        Err(e) => {
                            self.status = format_db_error("Delete", &e);
                            self.delete_confirm = None;
                            self.screen = Screen::Main;
                        }
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.delete_confirm = None;
                self.screen = Screen::Main;
                self.status = "Cancelled".into();
            }
            _ => {}
        }
    }
}
