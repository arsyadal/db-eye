use super::{App, Screen};
use crate::db::{DbClient, DbType, format_db_error};
use crossterm::event::{KeyCode, KeyModifiers};
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
    pub(super) async fn handle_query(&mut self, key: KeyCode, modifiers: KeyModifiers) {
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
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                let sql = self.query_input.trim().to_string();
                if !sql.is_empty() && let Some(tab) = self.current_tab() {
                    let prefix = match tab.db.db_type {
                        DbType::Sqlite => "EXPLAIN QUERY PLAN ",
                        DbType::Postgres | DbType::Mysql => "EXPLAIN ",
                    };
                    let explain_sql = format!("{}{}", prefix, sql);
                    self.execute_current_query(explain_sql).await;
                    self.screen = Screen::Main;
                }
            }
            KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
                let sql = self.query_input.trim().to_string();
                if !sql.is_empty() {
                    self.save_query_input.clear();
                    self.screen = Screen::SaveQueryPrompt;
                }
            }
            KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.load_named_queries();
                self.named_query_index = 0;
                self.screen = Screen::NamedQueriesList;
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct NamedQuery {
    pub name: String,
    pub sql: String,
}

impl App {
    pub fn queries_config_path(&self) -> Option<std::path::PathBuf> {
        #[cfg(target_os = "windows")]
        let base = std::env::var("APPDATA").ok().map(std::path::PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".config"));

        base.map(|b| b.join("db-eye").join("queries.json"))
    }

    pub fn load_named_queries(&mut self) {
        if let Some(path) = self.queries_config_path()
            && path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(queries) = serde_json::from_str::<Vec<NamedQuery>>(&content)
        {
            self.named_queries = queries;
            return;
        }
        self.named_queries = vec![];
    }

    pub fn save_named_queries(&self) {
        if let Some(path) = self.queries_config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(&self.named_queries) {
                let _ = std::fs::write(&path, content);
            }
        }
    }

    pub(super) fn handle_save_query_prompt(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                self.save_query_input.push(c);
            }
            KeyCode::Backspace => {
                self.save_query_input.pop();
            }
            KeyCode::Enter => {
                let name = self.save_query_input.trim().to_string();
                if !name.is_empty() {
                    let sql = self.query_input.trim().to_string();
                    if let Some(pos) = self.named_queries.iter().position(|q| q.name == name) {
                        self.named_queries[pos].sql = sql;
                    } else {
                        self.named_queries.push(NamedQuery { name: name.clone(), sql });
                    }
                    self.save_named_queries();
                    self.status = format!("Query saved as '{}'", name);
                }
                self.screen = Screen::Query;
            }
            KeyCode::Esc => {
                self.screen = Screen::Query;
            }
            _ => {}
        }
    }

    pub(super) fn handle_named_queries_list(&mut self, key: KeyCode) {
        if self.named_queries.is_empty() {
            match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.screen = Screen::Query;
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.named_query_index + 1 < self.named_queries.len() {
                    self.named_query_index += 1;
                } else {
                    self.named_query_index = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.named_query_index > 0 {
                    self.named_query_index -= 1;
                } else {
                    self.named_query_index = self.named_queries.len().saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                if let Some(q) = self.named_queries.get(self.named_query_index) {
                    self.query_input = q.sql.clone();
                    self.screen = Screen::Query;
                    self.status = format!("Loaded named query '{}'", q.name);
                }
            }
            KeyCode::Delete | KeyCode::Backspace if self.named_query_index < self.named_queries.len() => {
                let removed = self.named_queries.remove(self.named_query_index);
                self.save_named_queries();
                self.status = format!("Deleted named query '{}'", removed.name);
                if self.named_query_index >= self.named_queries.len() {
                    self.named_query_index = self.named_queries.len().saturating_sub(1);
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Query;
            }
            _ => {}
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
    use crate::app::Tab;

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

    #[tokio::test]
    async fn test_handle_query_explain() {
        let mut app = App::new(false);
        app.screen = Screen::Query;
        app.query_input = "SELECT * FROM users".to_string();
        
        let db = DbClient::connect("sqlite::memory:").await.unwrap();
        app.tabs.push(Tab::new("sqlite::memory:".to_string(), "sqlite::memory:".to_string(), db));
        app.active_tab = 0;
        
        // Execute Ctrl+E
        app.handle_query(KeyCode::Char('e'), KeyModifiers::CONTROL).await;
        
        // It should try to execute "EXPLAIN QUERY PLAN SELECT * FROM users"
        // and fail with "no such table: users" or similar, proving it prepended the explain prefix
        assert!(app.query_history.iter().any(|h| h.sql.starts_with("EXPLAIN QUERY PLAN")));
    }

    #[test]
    fn test_named_queries_saving_loading_and_actions() {
        let mut app = App::new(false);
        app.query_input = "SELECT 1".to_string();
        app.save_query_input = "test_q".to_string();

        // Test save query prompt Enter key
        app.handle_save_query_prompt(KeyCode::Enter);
        assert_eq!(app.named_queries.len(), 1);
        assert_eq!(app.named_queries[0].name, "test_q");
        assert_eq!(app.named_queries[0].sql, "SELECT 1");
        assert_eq!(app.screen, Screen::Query);

        // Clear query input and load named query
        app.query_input.clear();
        app.screen = Screen::NamedQueriesList;
        app.named_query_index = 0;
        app.handle_named_queries_list(KeyCode::Enter);
        assert_eq!(app.query_input, "SELECT 1");
        assert_eq!(app.screen, Screen::Query);

        // Test deletion
        app.screen = Screen::NamedQueriesList;
        app.named_query_index = 0;
        app.handle_named_queries_list(KeyCode::Delete);
        assert_eq!(app.named_queries.len(), 0);
    }
}
