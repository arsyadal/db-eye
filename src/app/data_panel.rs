use super::{App, CrudForm, ExportForm, ImportForm, Focus, Screen};
use crate::db::{ColumnInfo, format_db_error};
use crossterm::event::KeyCode;

impl App {
    pub(super) async fn handle_data_focus(&mut self, key: KeyCode) {
        let editing = if let Some(tab) = self.current_tab() {
            tab.editing_cell.is_some()
        } else {
            false
        };

        if editing {
            match key {
                KeyCode::Enter => {
                    self.save_inline_edit().await;
                }
                KeyCode::Esc => {
                    if let Some(tab) = self.current_tab_mut() {
                        tab.editing_cell = None;
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(tab) = self.current_tab_mut() {
                        tab.edit_buffer.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(tab) = self.current_tab_mut() {
                        tab.edit_buffer.pop();
                    }
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                let page_size = self.page_size;
                let should_next = self
                    .current_tab()
                    .map(|t| {
                        t.selected_row + 1 >= t.display_rows().len()
                            && (t.row_offset + page_size) as i64 <= t.total_rows
                    })
                    .unwrap_or(false);
                if should_next {
                    if let Some(t) = self.current_tab_mut() {
                        t.row_offset += page_size;
                        t.selected_row = 0;
                    }
                    self.load_table_data().await;
                } else if let Some(t) = self.current_tab_mut() {
                    let count = t.display_rows().len();
                    if t.selected_row + 1 < count {
                        t.selected_row += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let page_size = self.page_size;
                let should_prev = self
                    .current_tab()
                    .map(|t| t.selected_row == 0 && t.row_offset > 0)
                    .unwrap_or(false);
                if should_prev {
                    if let Some(t) = self.current_tab_mut() {
                        t.row_offset = t.row_offset.saturating_sub(page_size);
                    }
                    self.load_table_data().await;
                    if let Some(t) = self.current_tab_mut() {
                        t.selected_row = t.display_rows().len().saturating_sub(1);
                    }
                } else if let Some(t) = self.current_tab_mut()
                    && t.selected_row > 0
                {
                    t.selected_row -= 1;
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(t) = self.current_tab_mut() {
                    t.col_offset += 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(t) = self.current_tab_mut()
                    && t.col_offset > 0
                {
                    t.col_offset -= 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if self.read_only {
                    self.read_only_block();
                    return;
                }
                let cell_data = if let Some(tab) = self.current_tab() {
                    let row = tab.selected_row;
                    let col = tab.col_offset;
                    tab.display_rows()
                        .get(row)
                        .and_then(|r| r.get(col))
                        .map(|v| (row, col, v.clone()))
                } else {
                    None
                };

                if let Some((row, col, val)) = cell_data
                    && let Some(tab) = self.current_tab_mut()
                {
                    tab.editing_cell = Some((row, col));
                    tab.edit_buffer = val;
                }
            }
            KeyCode::Char(':') => {
                self.query_input.clear();
                self.screen = Screen::Query;
            }
            KeyCode::Char('/') => {
                self.search_input = self
                    .current_tab()
                    .map(|t| t.search_query.clone())
                    .unwrap_or_default();
                self.screen = Screen::Search;
            }
            KeyCode::Char('v') => {
                if let Some(tab) = self.current_tab() {
                    let table_name = tab.path.split('/').next_back().unwrap_or("table");
                    self.export_form = Some(ExportForm::new(table_name));
                    self.screen = Screen::ExportForm;
                }
            }
            KeyCode::Char('I') => {
                if self.read_only {
                    self.read_only_block();
                    return;
                }
                if let Some(tab) = self.current_tab() {
                    let db_cols = tab.result.as_ref().map(|r| r.columns.clone()).unwrap_or_default();
                    self.import_form = Some(ImportForm::new(db_cols));
                    self.screen = Screen::ImportForm;
                }
            }
            KeyCode::Char('s') => {
                self.open_table_stats().await;
            }
            KeyCode::Char('y') => {
                if let Some(tab) = self.current_tab()
                    && let Some(row_data) = tab.display_rows().get(tab.selected_row)
                    && let Some(cell_val) = row_data.get(tab.col_offset)
                {
                    match copy_to_clipboard(cell_val) {
                        Ok(()) => self.status = "Cell copied to clipboard".to_string(),
                        Err(e) => self.status = format!("Copy failed: {}", e),
                    }
                }
            }
            KeyCode::Char('Y') => {
                if let Some(tab) = self.current_tab()
                    && let Some(row_data) = tab.display_rows().get(tab.selected_row)
                {
                    let joined = row_data.join("\t");
                    match copy_to_clipboard(&joined) {
                        Ok(()) => self.status = "Row copied to clipboard".to_string(),
                        Err(e) => self.status = format!("Copy failed: {}", e),
                    }
                }
            }
            KeyCode::Char('C') => {
                if let Some(tab) = self.current_tab() {
                    let col_idx = tab.col_offset;
                    let col_cells: Vec<String> = tab.display_rows()
                        .iter()
                        .filter_map(|r| r.get(col_idx).cloned())
                        .collect();
                    let joined = col_cells.join("\n");
                    match copy_to_clipboard(&joined) {
                        Ok(()) => self.status = "Column copied to clipboard".to_string(),
                        Err(e) => self.status = format!("Copy failed: {}", e),
                    }
                }
            }
            KeyCode::Char('o') => {
                self.toggle_sort_column().await;
            }
            KeyCode::PageDown => {
                let page_size = self.page_size;
                let can_advance = self
                    .current_tab()
                    .map(|t| (t.row_offset + page_size) as i64 <= t.total_rows)
                    .unwrap_or(false);
                if can_advance {
                    if let Some(t) = self.current_tab_mut() {
                        t.row_offset += page_size;
                        t.selected_row = 0;
                    }
                    self.load_table_data().await;
                }
            }
            KeyCode::PageUp => {
                let page_size = self.page_size;
                if let Some(t) = self.current_tab_mut() {
                    t.row_offset = t.row_offset.saturating_sub(page_size);
                    t.selected_row = 0;
                }
                self.load_table_data().await;
            }
            KeyCode::Char('g') => {
                self.jump_input.clear();
                self.screen = Screen::Jump;
            }
            KeyCode::Char('i') => {
                if self.read_only {
                    self.read_only_block();
                } else {
                    self.open_insert_form().await;
                }
            }
            KeyCode::Char('u') => {
                if self.read_only {
                    self.read_only_block();
                } else {
                    self.open_update_form().await;
                }
            }
            KeyCode::Char('d') => {
                if self.read_only {
                    self.read_only_block();
                } else {
                    self.open_delete_confirm().await;
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if let Some(t) = self.current_tab_mut() {
                    t.result = None;
                    t.filtered_rows.clear();
                    t.selected_row = 0;
                    t.row_offset = 0;
                    t.col_offset = 0;
                    t.search_query.clear();
                }
                self.focus = Focus::Tables;
                self.status = self.table_help().into();
            }
            _ => {}
        }
    }

    async fn open_table_stats(&mut self) {
        let (table, schema) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.name.clone(),
                None => return,
            };
            (table, tab.current_schema().map(|s| s.to_string()))
        };
        if let Some(tab) = self.current_tab() {
            match tab.db.table_stats(schema.as_deref(), &table).await {
                Ok(stats) => {
                    self.table_stats = Some(stats);
                    self.screen = Screen::TableStats;
                }
                Err(e) => self.status = format_db_error("Loading table stats", &e),
            }
        }
    }

    async fn save_inline_edit(&mut self) {
        let (table, schema, col_idx, new_val, row_data, columns) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let (row_idx, col_idx) = match tab.editing_cell {
                Some(c) => c,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.name.clone(),
                None => return,
            };
            let row = match tab.display_rows().get(row_idx) {
                Some(r) => r.clone(),
                None => return,
            };
            let cols = tab
                .result
                .as_ref()
                .map(|r| r.columns.clone())
                .unwrap_or_default();
            (
                table,
                tab.current_schema().map(|s| s.to_string()),
                col_idx,
                tab.edit_buffer.clone(),
                row,
                cols,
            )
        };

        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            match tab.db.get_columns(schema.as_deref(), &table).await {
                Ok(col_info) => {
                    let col_name = match columns.get(col_idx) {
                        Some(c) => c,
                        None => return,
                    };
                    let mut pk_columns: Vec<ColumnInfo> =
                        col_info.iter().filter(|c| c.is_pk).cloned().collect();
                    if pk_columns.is_empty() {
                        self.status = "Update requires a primary key".into();
                        tab.editing_cell = None;
                        return;
                    }
                    pk_columns.sort_by_key(|c| c.pk_order);

                    let quote = tab.db.identifier_quote();
                    let quote_ident = |ident: &str| {
                        let doubled = format!("{quote}{quote}");
                        format!("{quote}{}{quote}", ident.replace(quote, &doubled))
                    };
                    let numbered_placeholders = tab.db.uses_numbered_placeholders();

                    if let Some(col) = col_info.iter().find(|c| c.name == col_name.as_str())
                        && let Err(msg) = col.validate_input(&new_val)
                    {
                        self.status = msg;
                        tab.editing_cell = None;
                        return;
                    }

                    let mut values = vec![CrudForm::form_value(&new_val)];
                    let mut conditions = Vec::new();

                    for (i, pk) in pk_columns.iter().enumerate() {
                        let pk_val = columns
                            .iter()
                            .position(|c| c == &pk.name)
                            .and_then(|idx| row_data.get(idx))
                            .cloned()
                            .unwrap_or_default();
                        values.push(CrudForm::form_value(&pk_val));
                        let placeholder = if numbered_placeholders {
                            format!("${}", i + 2)
                        } else {
                            "?".to_string()
                        };
                        conditions.push(format!("{} = {}", quote_ident(&pk.name), placeholder));
                    }

                    let table_ident = if let Some(s) = &schema {
                        format!("{}.{}", quote_ident(s), quote_ident(&table))
                    } else {
                        quote_ident(&table)
                    };
                    let sql = format!(
                        "UPDATE {} SET {} = {} WHERE {}",
                        table_ident,
                        quote_ident(col_name),
                        if numbered_placeholders {
                            "$1".to_string()
                        } else {
                            "?".to_string()
                        },
                        conditions.join(" AND ")
                    );

                    match tab.db.execute_write_with_values(&sql, &values).await {
                        Ok(_) => {
                            if let Some(res) = tab.result.as_mut() {
                                // Find actual row in result by primary key (since display_rows might be filtered)
                                let row_pk_values: Vec<String> = pk_columns
                                    .iter()
                                    .map(|pk| {
                                        columns
                                            .iter()
                                            .position(|c| c == &pk.name)
                                            .and_then(|idx| row_data.get(idx))
                                            .cloned()
                                            .unwrap_or_default()
                                    })
                                    .collect();

                                if let Some(target_row) = res.rows.iter_mut().find(|r| {
                                    pk_columns.iter().all(|pk| {
                                        let idx = res
                                            .columns
                                            .iter()
                                            .position(|c| c == &pk.name)
                                            .unwrap_or(0);
                                        r.get(idx)
                                            == row_pk_values.get(
                                                pk_columns
                                                    .iter()
                                                    .position(|p| p.name == pk.name)
                                                    .unwrap_or(0),
                                            )
                                    })
                                }) && let Some(cell) = target_row.get_mut(col_idx)
                                {
                                    *cell = new_val;
                                }
                            }
                            tab.update_filter();
                            tab.editing_cell = None;
                            self.status = "Cell updated".into();
                        }
                        Err(e) => {
                            self.status = format_db_error("Inline update", &e);
                            tab.editing_cell = None;
                        }
                    }
                }
                Err(e) => {
                    self.status = format_db_error("Loading columns", &e);
                    tab.editing_cell = None;
                }
            }
        }
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), std::io::Error> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[cfg(target_os = "macos")]
    let cmd: (&str, Vec<&str>) = ("pbcopy", vec![]);

    #[cfg(target_os = "windows")]
    let cmd: (&str, Vec<&str>) = ("clip", vec![]);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let cmd: (&str, Vec<&str>) = if std::env::var("WAYLAND_DISPLAY").is_ok() {
        ("wl-copy", vec![])
    } else {
        ("xclip", vec!["-selection", "clipboard"])
    };

    let mut child = Command::new(cmd.0)
        .args(cmd.1)
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let _ = child.wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Tab};
    use crate::db::{DbClient, QueryResult};

    #[tokio::test]
    async fn test_copy_shortcuts() {
        let mut app = App::new(false);
        let db = DbClient::connect("sqlite::memory:").await.unwrap();
        let mut tab = Tab::new("test".to_string(), "test".to_string(), db);

        let result = QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
            ],
            rows_affected: 0,
        };
        tab.result = Some(result);
        tab.update_filter();

        tab.selected_row = 1;
        tab.col_offset = 1;
        app.tabs.push(tab);
        app.active_tab = 0;

        app.handle_data_focus(KeyCode::Char('y')).await;
        assert!(app.status.contains("copied to clipboard") || app.status.contains("Copy failed"));

        app.status.clear();
        app.handle_data_focus(KeyCode::Char('Y')).await;
        assert!(app.status.contains("copied to clipboard") || app.status.contains("Copy failed"));

        app.status.clear();
        app.handle_data_focus(KeyCode::Char('C')).await;
        assert!(app.status.contains("copied to clipboard") || app.status.contains("Copy failed"));
    }
}
