use super::{App, Screen};
use crate::db::{DbClient, format_db_error};
use crossterm::event::KeyCode;
use std::time::Instant;

#[derive(Clone)]
pub struct QueryHistoryEntry {
    pub sql: String,
    pub rows: Option<usize>,
    pub duration_ms: u128,
    pub success: bool,
    pub error: Option<String>,
}

impl QueryHistoryEntry {
    pub fn status_label(&self) -> String {
        if self.success {
            match self.rows {
                Some(rows) => format!("OK ({} rows)", rows),
                None => "OK".to_string(),
            }
        } else {
            "ERROR".to_string()
        }
    }
}

impl App {
    pub(super) async fn handle_query(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => {
                if let Some(tab) = self.current_tab_mut()
                    && !tab.query_history.is_empty()
                {
                    let new_index = match tab.history_index {
                        Some(idx) => {
                            if idx > 0 {
                                idx - 1
                            } else {
                                0
                            }
                        }
                        None => tab.query_history.len().saturating_sub(1),
                    };
                    tab.history_index = Some(new_index);
                    if let Some(history_sql) = tab.query_history.get(new_index) {
                        self.query_input = history_sql.clone();
                    }
                }
            }
            KeyCode::Down => {
                if let Some(tab) = self.current_tab_mut()
                    && let Some(idx) = tab.history_index
                {
                    if idx + 1 < tab.query_history.len() {
                        let new_index = idx + 1;
                        tab.history_index = Some(new_index);
                        self.query_input = tab.query_history[new_index].clone();
                    } else {
                        tab.history_index = None;
                        self.query_input.clear();
                    }
                }
            }
            KeyCode::Char(c) => {
                self.query_input.push(c);
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
            }
            KeyCode::Backspace => {
                self.query_input.pop();
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
            }
            KeyCode::Enter => {
                let sql = self.query_input.trim().to_string();
                if sql.is_empty() {
                    self.screen = Screen::Main;
                    return;
                }
                self.execute_current_query(sql).await;
                self.screen = Screen::Main;
            }
            KeyCode::Esc => {
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    async fn execute_current_query(&mut self, sql: String) {
        if self.read_only && !DbClient::is_read_only_sql(&sql) {
            self.read_only_block();
            return;
        }

        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.query_history.last() != Some(&sql) {
                tab.query_history.push(sql.clone());
            }
            tab.history_index = None;

            let started = Instant::now();
            let mut history = QueryHistoryEntry {
                sql,
                rows: None,
                duration_ms: 0,
                success: false,
                error: None,
            };

            match tab.db.execute_query(&history.sql).await {
                Ok(result) => {
                    let duration_ms = started.elapsed().as_millis();
                    let count = result.rows.len();
                    tab.total_rows = count as i64;
                    let msg = if count > 0 {
                        format!("{} rows returned", count)
                    } else {
                        format!("{} rows affected", result.rows_affected)
                    };
                    tab.result = Some(result);
                    tab.search_query.clear();
                    tab.update_filter();
                    tab.selected_row = 0;
                    tab.row_offset = 0;
                    tab.col_offset = 0;

                    history.rows = Some(count);
                    history.duration_ms = duration_ms;
                    history.success = true;
                    self.status = format!("{} in {}", msg, format_duration_ms(duration_ms));
                }
                Err(e) => {
                    let duration_ms = started.elapsed().as_millis();
                    let error = format_db_error("Query", &e);
                    history.duration_ms = duration_ms;
                    history.error = Some(error.clone());
                    self.status = error;
                }
            }
            self.query_history.insert(0, history);
            if self.query_history.len() > 100 {
                self.query_history.truncate(100);
            }
        }
    }

    pub(super) async fn handle_query_history(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down
                if self.history_index + 1 < self.query_history.len() =>
            {
                self.history_index += 1;
            }
            KeyCode::Char('k') | KeyCode::Up if self.history_index > 0 => {
                self.history_index -= 1;
            }
            KeyCode::Enter => {
                if let Some(entry) = self.query_history.get(self.history_index) {
                    self.query_input = entry.sql.clone();
                    self.screen = Screen::Query;
                    self.status = "Editing query from history".into();
                }
            }
            KeyCode::Char('r') => {
                if let Some(entry) = self.query_history.get(self.history_index).cloned() {
                    self.screen = Screen::Main;
                    self.execute_current_query(entry.sql).await;
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Main;
                self.status = self.table_help().into();
            }
            _ => {}
        }
    }

    pub(super) fn handle_search(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                self.search_input.push(c);
                self.apply_search();
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.apply_search();
            }
            KeyCode::Enter | KeyCode::Esc => {
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    fn apply_search(&mut self) {
        let q = self.search_input.clone();
        if let Some(tab) = self.current_tab_mut() {
            tab.search_query = q;
            tab.update_filter();
        }
        if let Some(tab) = self.current_tab() {
            let page_len = tab.result.as_ref().map(|r| r.rows.len()).unwrap_or(0);
            self.status = format!(
                "{} of {} rows on this page match",
                tab.filtered_rows.len(),
                page_len
            );
        }
    }
}

pub fn format_duration_ms(ms: u128) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else {
        format!("{:.2}s", ms as f64 / 1_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_sql_allows_selects_and_explain() {
        assert!(DbClient::is_read_only_sql("select * from users"));
        assert!(DbClient::is_read_only_sql(
            "WITH recent AS (SELECT 1) SELECT * FROM recent"
        ));
        assert!(DbClient::is_read_only_sql("EXPLAIN SELECT * FROM users"));
        assert!(DbClient::is_read_only_sql("SHOW TABLES"));
    }

    #[test]
    fn read_only_sql_blocks_writes() {
        assert!(!DbClient::is_read_only_sql("insert into users values (1)"));
        assert!(!DbClient::is_read_only_sql("UPDATE users SET name = 'Ada'"));
        assert!(!DbClient::is_read_only_sql("delete from users"));
        assert!(!DbClient::is_read_only_sql("drop table users"));
    }
}
