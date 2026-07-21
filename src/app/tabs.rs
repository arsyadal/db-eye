use super::{App, ConnectForm, DbTypeChoice, Focus, Screen, ServerConn};
use crate::db::{DbClient, QueryResult, TableEntry, format_db_error};
use crossterm::event::{KeyCode, KeyModifiers};

pub struct Tab {
    pub path: String,
    pub reconnect_url: String,
    pub db: DbClient,
    pub schemas: Vec<String>,
    pub schema_index: usize,
    pub tables: Vec<TableEntry>,
    pub table_index: usize,
    pub result: Option<QueryResult>,
    pub filtered_rows: Vec<Vec<String>>,
    pub row_offset: usize,
    pub col_offset: usize,
    pub selected_row: usize,
    pub sort_column: Option<String>,
    pub sort_desc: bool,
    pub total_rows: i64,
    pub search_query: String,
    pub server_info: Option<(ConnectForm, DbTypeChoice)>,
    pub query_history: Vec<String>,
    pub history_index: Option<usize>,
    pub editing_cell: Option<(usize, usize)>, // (row, col)
    pub edit_buffer: String,
}

impl Tab {
    pub fn new(path: String, reconnect_url: String, db: DbClient) -> Self {
        Self {
            path,
            reconnect_url,
            db,
            schemas: vec![],
            schema_index: 0,
            tables: vec![],
            table_index: 0,
            result: None,
            filtered_rows: vec![],
            row_offset: 0,
            col_offset: 0,
            selected_row: 0,
            sort_column: None,
            sort_desc: false,
            total_rows: 0,
            search_query: String::new(),
            server_info: None,
            query_history: vec![],
            history_index: None,
            editing_cell: None,
            edit_buffer: String::new(),
        }
    }

    pub fn current_schema(&self) -> Option<&str> {
        self.schemas.get(self.schema_index).map(|s| s.as_str())
    }

    pub fn update_filter(&mut self) {
        if let Some(ref result) = self.result {
            if self.search_query.is_empty() {
                self.filtered_rows = result.rows.clone();
            } else {
                let q = self.search_query.to_lowercase();
                self.filtered_rows = result
                    .rows
                    .iter()
                    .filter(|row| row.iter().any(|cell| cell.to_lowercase().contains(&q)))
                    .cloned()
                    .collect();
            }
            self.selected_row = 0;
        }
    }

    pub fn display_rows(&self) -> &[Vec<String>] {
        &self.filtered_rows
    }

    pub fn short_name(&self) -> String {
        self.path
            .split('/')
            .next_back()
            .unwrap_or(&self.path)
            .to_string()
    }
}

impl App {
    fn data_help(&self) -> &'static str {
        if self.read_only {
            "READ-ONLY  |  j/k:nav  PgUp/PgDn:page  g:jump  o:sort  v:export  s:stats  /:search  :::query  Ctrl+H:history  Esc:back"
        } else {
            "j/k:nav  i:insert  u:update  d:delete  e:edit  PgUp/PgDn:page  g:jump  o:sort  v:export  s:stats  /:search  :::query  Ctrl+H:history  Esc:back"
        }
    }

    pub(super) fn table_help(&self) -> &'static str {
        if self.read_only {
            "READ-ONLY  |  Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close  Esc:back"
        } else {
            "Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close  Esc:back"
        }
    }

    pub(super) async fn handle_main(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::CONTROL) {
            match key {
                KeyCode::Char('t') => {
                    self.screen = Screen::Connect;
                    self.sqlite_input.clear();
                    self.connection_url_input.clear();
                    return;
                }
                KeyCode::Char('w') => {
                    if !self.tabs.is_empty() {
                        self.tabs.remove(self.active_tab);
                        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                            self.active_tab = self.tabs.len() - 1;
                        }
                        if self.tabs.is_empty() {
                            self.screen = Screen::Connect;
                        }
                    }
                    return;
                }
                KeyCode::Char('h') => {
                    self.history_index = 0;
                    self.screen = Screen::QueryHistory;
                    self.status = "Query history — j/k:nav  Enter:edit  r:run  Esc:close".into();
                    return;
                }
                KeyCode::Char('r') => {
                    self.reconnect_current_tab().await;
                    return;
                }
                _ => {}
            }
        }

        // Tab switching with [ and ] (Mac-friendly, no Ctrl+arrow)
        match key {
            KeyCode::Char('?') => {
                self.screen = Screen::Help;
                return;
            }
            KeyCode::Char('H') => {
                self.history_index = 0;
                self.screen = Screen::QueryHistory;
                self.status = "Query history — j/k:nav  Enter:edit  r:run  Esc:close".into();
                return;
            }
            KeyCode::Char('[') => {
                if self.active_tab > 0 {
                    self.active_tab -= 1;
                }
                return;
            }
            KeyCode::Char(']') => {
                if self.active_tab + 1 < self.tabs.len() {
                    self.active_tab += 1;
                }
                return;
            }
            _ => {}
        }

        match key {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Tables => {
                        self.status = self.data_help().into();
                        Focus::Data
                    }
                    Focus::Data => {
                        self.status = self.table_help().into();
                        Focus::Tables
                    }
                    Focus::Saved => Focus::Tables,
                };
            }
            _ => match self.focus {
                Focus::Tables => self.handle_tables_focus(key).await,
                Focus::Data => self.handle_data_focus(key).await,
                Focus::Saved => {}
            },
        }
    }

    async fn handle_tables_focus(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(tab) = self.current_tab_mut()
                    && tab.table_index + 1 < tab.tables.len()
                {
                    tab.table_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(tab) = self.current_tab_mut()
                    && tab.table_index > 0
                {
                    tab.table_index -= 1;
                }
            }
            KeyCode::Enter => {
                self.load_table_data().await;
                self.focus = Focus::Data;
                self.status = self.data_help().into();
            }
            KeyCode::Esc => {
                self.go_back().await;
            }
            _ => {}
        }
    }

    async fn go_back(&mut self) {
        let is_postgres = self
            .current_tab()
            .map(|t| t.db.db_type == crate::db::DbType::Postgres)
            .unwrap_or(false);

        if is_postgres {
            self.screen = Screen::Schemas;
            self.status = "j/k: navigate  Enter: select schema  Esc: back".into();
            return;
        }

        let server_info = self.current_tab().and_then(|t| t.server_info.clone());
        if let Some((form, db_type)) = server_info {
            // Reconnect to server and show database list
            let url = form.server_url(&db_type);
            self.status = format!("Reconnecting to {}...", form.host);
            match crate::db::DbClient::connect(&url).await {
                Ok(client) => match client.list_databases().await {
                    Ok(databases) => {
                        self.server_conn = Some(ServerConn {
                            form,
                            db_type,
                            databases,
                            db_index: 0,
                        });
                        self.screen = Screen::Databases;
                        self.status = "j/k: navigate  Enter: open database  Esc: back".into();
                    }
                    Err(e) => {
                        self.status = format_db_error("Listing databases", &e);
                        self.screen = Screen::Connect;
                    }
                },
                Err(e) => {
                    self.status = format_db_error("Reconnect", &e);
                    self.screen = Screen::Connect;
                }
            }
        } else {
            self.screen = Screen::Connect;
            self.status = self.connect_help().into();
        }
    }

    pub(super) async fn reconnect_current_tab(&mut self) {
        let (url, had_table, prev_schema) = match self.current_tab() {
            Some(t) => (
                t.reconnect_url.clone(),
                t.result.is_some(),
                t.current_schema().map(|s| s.to_string()),
            ),
            None => return,
        };
        self.status = "Reconnecting...".into();
        let client = match DbClient::connect(&url).await {
            Ok(client) => client,
            Err(e) => {
                self.status = format_db_error("Reconnect", &e);
                return;
            }
        };
        let is_postgres = client.db_type == crate::db::DbType::Postgres;
        if let Some(tab) = self.current_tab_mut() {
            tab.db = client;
        }

        if is_postgres {
            let schemas = match self.current_tab() {
                Some(tab) => tab.db.list_schemas().await.unwrap_or_default(),
                None => return,
            };
            if let Some(tab) = self.current_tab_mut() {
                tab.schemas = schemas;
                if let Some(s) = &prev_schema {
                    tab.schema_index = tab.schemas.iter().position(|x| x == s).unwrap_or(0);
                }
            }
        }

        let schema = self
            .current_tab()
            .and_then(|t| t.current_schema().map(|s| s.to_string()));
        let tables = match self.current_tab() {
            Some(tab) => tab
                .db
                .list_tables(schema.as_deref())
                .await
                .unwrap_or_default(),
            None => return,
        };
        if let Some(tab) = self.current_tab_mut() {
            tab.tables = tables;
        }

        self.status = "Reconnected".into();
        if had_table {
            self.load_table_data().await;
        }
    }

    pub(super) async fn load_table_data(&mut self) {
        let (table, offset, page_size, schema, sort_column, sort_desc) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.name.clone(),
                None => return,
            };
            (
                table,
                tab.row_offset,
                self.page_size,
                tab.current_schema().map(|s| s.to_string()),
                tab.sort_column.clone(),
                tab.sort_desc,
            )
        };
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            match tab.db.count_rows(schema.as_deref(), &table).await {
                Ok(count) => tab.total_rows = count,
                Err(_) => tab.total_rows = 0,
            }
            let sort = sort_column.as_deref().map(|c| (c, sort_desc));
            match tab
                .db
                .query_table(
                    schema.as_deref(),
                    &table,
                    page_size as u32,
                    offset as u32,
                    sort,
                )
                .await
            {
                Ok(result) => {
                    tab.result = Some(result);
                    tab.search_query.clear();
                    tab.update_filter();
                    let actions = if self.read_only {
                        "READ-ONLY  /:search  o:sort  PgUp/PgDn:page  g:jump  v:csv  s:stats  ::sql  Ctrl+H:history  q:back"
                    } else {
                        "i:insert  u:update  d:delete  e:edit  /:search  o:sort  PgUp/PgDn:page  g:jump  v:csv  s:stats  ::sql  Ctrl+H:history  q:back"
                    };
                    self.status = format!(
                        "{}  |  {} rows  |  Tab:focus  j/k:scroll  h/l:cols  {}",
                        table, tab.total_rows, actions
                    );
                }
                Err(e) => self.status = format_db_error("Loading table data", &e),
            }
        }
    }

    pub(super) async fn handle_jump(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.jump_input.push(c);
            }
            KeyCode::Backspace => {
                self.jump_input.pop();
            }
            KeyCode::Enter => {
                let row_num: usize = self.jump_input.trim().parse().unwrap_or(0);
                self.jump_input.clear();
                self.screen = Screen::Main;
                if row_num == 0 {
                    return;
                }
                if let Some(tab) = self.current_tab_mut() {
                    tab.row_offset = row_num.saturating_sub(1);
                    tab.selected_row = 0;
                }
                self.load_table_data().await;
            }
            KeyCode::Esc => {
                self.jump_input.clear();
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    pub(super) async fn toggle_sort_column(&mut self) {
        let col_name = match self.current_tab() {
            Some(tab) => tab
                .result
                .as_ref()
                .and_then(|r| r.columns.get(tab.col_offset))
                .cloned(),
            None => return,
        };
        let col_name = match col_name {
            Some(c) => c,
            None => return,
        };

        if let Some(tab) = self.current_tab_mut() {
            match &tab.sort_column {
                Some(c) if *c == col_name && !tab.sort_desc => {
                    tab.sort_desc = true;
                }
                Some(c) if *c == col_name && tab.sort_desc => {
                    tab.sort_column = None;
                    tab.sort_desc = false;
                }
                _ => {
                    tab.sort_column = Some(col_name);
                    tab.sort_desc = false;
                }
            }
            tab.row_offset = 0;
            tab.selected_row = 0;
        }
        self.load_table_data().await;
    }
}
