use super::{App, Screen};
use crate::db::{DbClient, DbType, format_db_error};
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Instant;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
            KeyCode::Left => {
                self.query_cursor = self.query_cursor.saturating_sub(1);
                self.update_query_suggestions();
            }
            KeyCode::Right => {
                self.query_cursor = (self.query_cursor + 1).min(self.query_input.len());
                self.update_query_suggestions();
            }
            KeyCode::Home => {
                self.query_cursor = 0;
                self.update_query_suggestions();
            }
            KeyCode::End => {
                self.query_cursor = self.query_input.len();
                self.update_query_suggestions();
            }
            KeyCode::Up => {
                if let Some(tab) = self.current_tab_mut()
                    && !tab.query_history.is_empty()
                {
                    let new_index = match tab.history_index {
                        Some(idx) => idx.saturating_sub(1),
                        None => tab.query_history.len().saturating_sub(1),
                    };
                    tab.history_index = Some(new_index);
                    if let Some(history_sql) = tab.query_history.get(new_index) {
                        self.query_input = history_sql.clone();
                        self.query_cursor = self.query_input.len();
                        self.update_query_suggestions();
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
                        self.query_cursor = self.query_input.len();
                    } else {
                        tab.history_index = None;
                        self.query_input.clear();
                        self.query_cursor = 0;
                    }
                    self.update_query_suggestions();
                }
            }
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                let sql = self.query_input.trim().to_string();
                if !sql.is_empty()
                    && let Some(tab) = self.current_tab()
                {
                    let prefix = match tab.db.db_type {
                        DbType::Sqlite => "EXPLAIN QUERY PLAN ",
                        DbType::Postgres | DbType::Mysql | DbType::Mssql => "EXPLAIN ",
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
            KeyCode::Tab if !self.query_suggestions.is_empty() => {
                let sug = self.query_suggestions[self.query_suggestion_index].clone();
                self.apply_query_suggestion(&sug);
            }
            KeyCode::BackTab if !self.query_suggestions.is_empty() => {
                self.query_suggestion_index = self
                    .query_suggestion_index
                    .checked_sub(1)
                    .unwrap_or(self.query_suggestions.len().saturating_sub(1));
            }
            KeyCode::Char(c) => {
                self.query_input.insert(self.query_cursor, c);
                self.query_cursor += 1;
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
                self.update_query_suggestions();
            }
            KeyCode::Backspace => {
                if self.query_cursor > 0 {
                    self.query_cursor -= 1;
                    self.query_input.remove(self.query_cursor);
                }
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
                self.update_query_suggestions();
            }
            KeyCode::Delete => {
                if self.query_cursor < self.query_input.len() {
                    self.query_input.remove(self.query_cursor);
                }
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
                self.update_query_suggestions();
            }
            KeyCode::Enter => {
                self.query_suggestions.clear();
                let sql = self.query_input.trim().to_string();
                if sql.is_empty() {
                    self.screen = Screen::Main;
                    return;
                }
                self.execute_current_query(sql).await;
                self.screen = Screen::Main;
            }
            KeyCode::Esc => {
                self.query_suggestions.clear();
                if let Some(tab) = self.current_tab_mut() {
                    tab.history_index = None;
                }
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    pub fn update_query_suggestions(&mut self) {
        if self.query_input.is_empty() {
            self.query_suggestions.clear();
            self.query_suggestion_index = 0;
            return;
        }

        let cursor = self.query_cursor.min(self.query_input.len());
        let before_cursor = &self.query_input[..cursor];

        let mut word_start = before_cursor.len();
        let bytes = before_cursor.as_bytes();
        while word_start > 0 {
            let b = bytes[word_start - 1];
            if b.is_ascii_alphanumeric() || b == b'_' {
                word_start -= 1;
            } else {
                break;
            }
        }

        let prefix = &before_cursor[word_start..];
        if prefix.is_empty() {
            self.query_suggestions.clear();
            self.query_suggestion_index = 0;
            return;
        }

        let prefix_lower = prefix.to_lowercase();
        let mut candidates = Vec::new();

        // 1. Table and view names from current tab
        if let Some(tab) = self.current_tab() {
            for t in &tab.tables {
                if t.name.to_lowercase().starts_with(&prefix_lower)
                    && !candidates
                        .iter()
                        .any(|c: &String| c.eq_ignore_ascii_case(&t.name))
                {
                    candidates.push(t.name.clone());
                }
            }
            if let Some(result) = &tab.result {
                for col in &result.columns {
                    if col.to_lowercase().starts_with(&prefix_lower)
                        && !candidates
                            .iter()
                            .any(|c: &String| c.eq_ignore_ascii_case(col))
                    {
                        candidates.push(col.clone());
                    }
                }
            }
        }

        // 2. Common SQL Keywords
        const SQL_KEYWORDS: &[&str] = &[
            "SELECT",
            "FROM",
            "WHERE",
            "JOIN",
            "LEFT JOIN",
            "INNER JOIN",
            "ON",
            "GROUP BY",
            "ORDER BY",
            "HAVING",
            "LIMIT",
            "OFFSET",
            "INSERT INTO",
            "VALUES",
            "UPDATE",
            "SET",
            "DELETE FROM",
            "CREATE TABLE",
            "DROP TABLE",
            "ALTER TABLE",
            "AND",
            "OR",
            "NOT",
            "IN",
            "LIKE",
            "ILIKE",
            "BETWEEN",
            "IS NULL",
            "IS NOT NULL",
            "AS",
            "DISTINCT",
            "COUNT",
            "SUM",
            "AVG",
            "MIN",
            "MAX",
            "EXPLAIN",
            "PRAGMA",
            "SHOW TABLES",
            "DESCRIBE",
        ];

        for kw in SQL_KEYWORDS {
            if kw.to_lowercase().starts_with(&prefix_lower)
                && !candidates
                    .iter()
                    .any(|c: &String| c.eq_ignore_ascii_case(kw))
            {
                candidates.push(kw.to_string());
            }
        }

        candidates.truncate(6);
        self.query_suggestions = candidates;
        if self.query_suggestion_index >= self.query_suggestions.len() {
            self.query_suggestion_index = 0;
        }
    }

    pub fn apply_query_suggestion(&mut self, suggestion: &str) {
        let cursor = self.query_cursor.min(self.query_input.len());
        let before_cursor = &self.query_input[..cursor];

        let mut word_start = before_cursor.len();
        let bytes = before_cursor.as_bytes();
        while word_start > 0 {
            let b = bytes[word_start - 1];
            if b.is_ascii_alphanumeric() || b == b'_' {
                word_start -= 1;
            } else {
                break;
            }
        }

        let mut new_input = String::new();
        new_input.push_str(&self.query_input[..word_start]);
        new_input.push_str(suggestion);
        new_input.push(' ');
        let new_cursor = new_input.len();
        new_input.push_str(&self.query_input[cursor..]);

        self.query_input = new_input;
        self.query_cursor = new_cursor;
        self.update_query_suggestions();
    }

    pub(super) async fn execute_current_query(&mut self, sql: String) {
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

                    if let Ok(tables) = tab.db.list_tables(tab.current_schema()).await {
                        tab.tables = tables;
                    }
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
            self.save_persisted_query_history();
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
                    self.query_cursor = self.query_input.len();
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
    pub fn history_config_path(&self) -> Option<std::path::PathBuf> {
        #[cfg(target_os = "windows")]
        let base = std::env::var("APPDATA").ok().map(std::path::PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".config"));

        base.map(|b| b.join("db-eye").join("history.json"))
    }

    pub fn load_persisted_query_history(&mut self) {
        if let Some(path) = self.history_config_path()
            && path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(history) = serde_json::from_str::<Vec<QueryHistoryEntry>>(&content)
        {
            self.query_history = history;
            if self.query_history.len() > 100 {
                self.query_history.truncate(100);
            }
            let sql_strings: Vec<String> = self
                .query_history
                .iter()
                .rev()
                .map(|h| h.sql.clone())
                .collect();
            if let Some(tab) = self.current_tab_mut() {
                tab.query_history = sql_strings;
            }
            return;
        }
        self.query_history = vec![];
    }

    pub fn save_persisted_query_history(&self) {
        if let Some(path) = self.history_config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let to_save: &[QueryHistoryEntry] = if self.query_history.len() > 100 {
                &self.query_history[..100]
            } else {
                &self.query_history
            };
            if let Ok(content) = serde_json::to_string_pretty(to_save) {
                let _ = std::fs::write(&path, content);
            }
        }
    }

    pub(super) fn seed_tab_history(&self, tab: &mut crate::app::Tab) {
        tab.query_history = self
            .query_history
            .iter()
            .rev()
            .map(|h| h.sql.clone())
            .collect();
    }

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
                        self.named_queries.push(NamedQuery {
                            name: name.clone(),
                            sql,
                        });
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
                    self.query_cursor = self.query_input.len();
                    self.screen = Screen::Query;
                    self.status = format!("Loaded named query '{}'", q.name);
                }
            }
            KeyCode::Delete | KeyCode::Backspace
                if self.named_query_index < self.named_queries.len() =>
            {
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
        assert!(DbClient::is_read_only_sql("PRAGMA table_info(users)"));
        assert!(DbClient::is_read_only_sql(
            "SELECT 'INSERT' AS op FROM users"
        ));
        assert!(DbClient::is_read_only_sql(
            "SELECT \"DELETE\" AS op FROM users"
        ));
        assert!(DbClient::is_read_only_sql(
            "/* comment with INSERT */ SELECT * FROM users"
        ));
        assert!(DbClient::is_read_only_sql(
            "-- line comment INSERT\nSELECT * FROM users"
        ));
    }

    #[test]
    fn read_only_sql_blocks_writes() {
        assert!(!DbClient::is_read_only_sql("insert into users values (1)"));
        assert!(!DbClient::is_read_only_sql("UPDATE users SET name = 'Ada'"));
        assert!(!DbClient::is_read_only_sql("delete from users"));
        assert!(!DbClient::is_read_only_sql("drop table users"));
        assert!(!DbClient::is_read_only_sql(
            "WITH ins AS (INSERT INTO users VALUES (1) RETURNING *) SELECT * FROM ins"
        ));
        assert!(!DbClient::is_read_only_sql("SELECT 1; DROP TABLE users;"));
        assert!(!DbClient::is_read_only_sql("SELECT * INTO copy FROM users"));
        assert!(!DbClient::is_read_only_sql("PRAGMA foreign_keys = OFF"));
    }

    #[tokio::test]
    async fn test_handle_query_explain() {
        let mut app = App::new(false);
        app.screen = Screen::Query;
        app.query_input = "SELECT * FROM users".to_string();

        let db = DbClient::connect("sqlite::memory:").await.unwrap();
        app.tabs.push(Tab::new(
            "sqlite::memory:".to_string(),
            "sqlite::memory:".to_string(),
            db,
        ));
        app.active_tab = 0;

        // Execute Ctrl+E
        app.handle_query(KeyCode::Char('e'), KeyModifiers::CONTROL)
            .await;

        // It should try to execute "EXPLAIN QUERY PLAN SELECT * FROM users"
        // and fail with "no such table: users" or similar, proving it prepended the explain prefix
        assert!(
            app.query_history
                .iter()
                .any(|h| h.sql.starts_with("EXPLAIN QUERY PLAN"))
        );
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

    #[test]
    fn test_persisted_query_history_serialization_and_roundtrip() {
        let entry = QueryHistoryEntry {
            sql: "SELECT * FROM users WHERE active = 1".to_string(),
            rows: Some(42),
            duration_ms: 15,
            success: true,
            error: None,
        };
        let serialized = serde_json::to_string(&vec![entry.clone()]).unwrap();
        let deserialized: Vec<QueryHistoryEntry> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized[0].sql, entry.sql);
        assert_eq!(deserialized[0].rows, Some(42));
        assert_eq!(deserialized[0].duration_ms, 15);
        assert!(deserialized[0].success);
        assert_eq!(deserialized[0].status_label(), "OK (42 rows)");

        let err_entry = QueryHistoryEntry {
            sql: "INVALID SQL".to_string(),
            rows: None,
            duration_ms: 2,
            success: false,
            error: Some("syntax error".to_string()),
        };
        assert_eq!(err_entry.status_label(), "ERROR");

        let temp_dir = std::env::temp_dir().join(format!("db-eye-hist-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let hist_file = temp_dir.join("history.json");

        let json = serde_json::to_string_pretty(&vec![entry.clone(), err_entry.clone()]).unwrap();
        std::fs::write(&hist_file, json).unwrap();

        let read_back = std::fs::read_to_string(&hist_file).unwrap();
        let loaded: Vec<QueryHistoryEntry> = serde_json::from_str(&read_back).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].sql, entry.sql);
        assert_eq!(loaded[1].sql, err_entry.sql);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_query_editor_cursor_navigation() {
        let mut app = App::new(false);
        app.screen = Screen::Query;

        for c in "SELECT".chars() {
            app.handle_query(KeyCode::Char(c), KeyModifiers::NONE).await;
        }
        assert_eq!(app.query_input, "SELECT");
        assert_eq!(app.query_cursor, 6);

        // Move left twice: cursor at index 4 (between E and C)
        app.handle_query(KeyCode::Left, KeyModifiers::NONE).await;
        app.handle_query(KeyCode::Left, KeyModifiers::NONE).await;
        assert_eq!(app.query_cursor, 4);

        // Insert 'X' mid-string -> "SELEXCT"
        app.handle_query(KeyCode::Char('X'), KeyModifiers::NONE)
            .await;
        assert_eq!(app.query_input, "SELEXCT");
        assert_eq!(app.query_cursor, 5);

        // Backspace removes 'X' -> "SELECT"
        app.handle_query(KeyCode::Backspace, KeyModifiers::NONE)
            .await;
        assert_eq!(app.query_input, "SELECT");
        assert_eq!(app.query_cursor, 4);

        // Move right -> cursor 5
        app.handle_query(KeyCode::Right, KeyModifiers::NONE).await;
        assert_eq!(app.query_cursor, 5);

        // Home -> cursor 0
        app.handle_query(KeyCode::Home, KeyModifiers::NONE).await;
        assert_eq!(app.query_cursor, 0);

        // Delete -> removes 'S' -> "ELECT"
        app.handle_query(KeyCode::Delete, KeyModifiers::NONE).await;
        assert_eq!(app.query_input, "ELECT");
        assert_eq!(app.query_cursor, 0);

        // End -> cursor 5
        app.handle_query(KeyCode::End, KeyModifiers::NONE).await;
        assert_eq!(app.query_cursor, 5);
    }

    #[tokio::test]
    async fn test_auto_refresh_schema_after_ddl() {
        let mut app = App::new(false);
        app.screen = Screen::Query;

        let path =
            std::env::temp_dir().join(format!("db-eye-ddl-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let db = DbClient::connect(path.to_str().unwrap()).await.unwrap();
        let tab = Tab::new("test".to_string(), path.to_str().unwrap().to_string(), db);
        app.tabs.push(tab);
        app.active_tab = 0;

        assert_eq!(app.tabs[0].tables.len(), 0);

        // Execute CREATE TABLE
        app.query_input = "CREATE TABLE products (id INT, name TEXT)".to_string();
        app.query_cursor = app.query_input.len();
        app.handle_query(KeyCode::Enter, KeyModifiers::NONE).await;

        // Schema auto-refresh should have populated tab.tables
        assert_eq!(app.tabs[0].tables.len(), 1);
        assert_eq!(app.tabs[0].tables[0].name, "products");

        app.tabs[0].db.close().await;
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_query_suggestions_and_tab_autocomplete() {
        let mut app = App::new(false);
        app.screen = Screen::Query;

        // Type "sel"
        for c in "sel".chars() {
            app.handle_query(KeyCode::Char(c), KeyModifiers::NONE).await;
        }
        assert_eq!(app.query_input, "sel");
        assert!(!app.query_suggestions.is_empty());
        assert_eq!(app.query_suggestions[0], "SELECT");

        // Press Tab -> autocompletes to "SELECT "
        app.handle_query(KeyCode::Tab, KeyModifiers::NONE).await;
        assert_eq!(app.query_input, "SELECT ");
        assert_eq!(app.query_cursor, 7);

        // Add dummy table to active tab
        let db = DbClient::connect("sqlite::memory:").await.unwrap();
        let mut tab = Tab::new("test".to_string(), "sqlite::memory:".to_string(), db);
        tab.tables = vec![crate::db::TableEntry {
            name: "users".to_string(),
            is_view: false,
        }];
        app.tabs.push(tab);
        app.active_tab = 0;

        // Type "* FROM u"
        for c in "* FROM u".chars() {
            app.handle_query(KeyCode::Char(c), KeyModifiers::NONE).await;
        }
        assert_eq!(app.query_input, "SELECT * FROM u");
        assert!(!app.query_suggestions.is_empty());
        // First candidate should be table "users"
        assert_eq!(app.query_suggestions[0], "users");

        // Press Tab -> autocompletes to "SELECT * FROM users "
        app.handle_query(KeyCode::Tab, KeyModifiers::NONE).await;
        assert_eq!(app.query_input, "SELECT * FROM users ");
    }
}
