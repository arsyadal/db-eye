use crate::db::{ColumnInfo, DbClient, NULL_DISPLAY, QueryResult, format_db_error};
use crate::ui;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum DbTypeChoice {
    Sqlite,
    Postgres,
    Mysql,
}

impl DbTypeChoice {
    pub fn label(&self) -> &str {
        match self {
            Self::Sqlite => "SQLite",
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
        }
    }
    pub fn hint(&self) -> &str {
        match self {
            Self::Sqlite => "./path/to/file.db",
            Self::Postgres => "connect → pick database from list",
            Self::Mysql => "connect → pick database from list",
        }
    }
    pub fn default_port(&self) -> &str {
        match self {
            Self::Postgres => "5432",
            Self::Mysql => "3306",
            _ => "",
        }
    }
    pub fn next(&self) -> Self {
        match self {
            Self::Sqlite => Self::Postgres,
            Self::Postgres => Self::Mysql,
            Self::Mysql => Self::Sqlite,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            Self::Sqlite => Self::Mysql,
            Self::Postgres => Self::Sqlite,
            Self::Mysql => Self::Postgres,
        }
    }
    pub fn all() -> Vec<Self> {
        vec![Self::Sqlite, Self::Postgres, Self::Mysql]
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ConnectForm {
    pub host: String,
    pub port: String,
    pub user: String,
    #[serde(skip)] // Don't save passwords in plain text config
    pub pass: String,
    #[serde(skip)]
    pub active: usize,
}

impl ConnectForm {
    pub fn new(db_type: &DbTypeChoice) -> Self {
        Self {
            host: "localhost".into(),
            port: db_type.default_port().into(),
            user: String::new(),
            pass: String::new(),
            active: 2, // start on User
        }
    }

    pub fn labels() -> [&'static str; 4] {
        ["Host", "Port", "User", "Password"]
    }

    pub fn values(&self) -> [&str; 4] {
        [&self.host, &self.port, &self.user, &self.pass]
    }

    pub fn active_value_mut(&mut self) -> &mut String {
        match self.active {
            0 => &mut self.host,
            1 => &mut self.port,
            2 => &mut self.user,
            _ => &mut self.pass,
        }
    }

    pub fn next_field(&mut self) {
        self.active = (self.active + 1) % 5;
    }

    pub fn prev_field(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(4);
    }

    pub fn server_url(&self, db_type: &DbTypeChoice) -> String {
        match db_type {
            DbTypeChoice::Postgres => format!(
                "postgres://{}:{}@{}:{}/postgres",
                self.user, self.pass, self.host, self.port
            ),
            DbTypeChoice::Mysql => format!(
                "mysql://{}:{}@{}:{}/information_schema",
                self.user, self.pass, self.host, self.port
            ),
            _ => String::new(),
        }
    }

    pub fn db_url(&self, db_type: &DbTypeChoice, db_name: &str) -> String {
        match db_type {
            DbTypeChoice::Postgres => format!(
                "postgres://{}:{}@{}:{}/{}",
                self.user, self.pass, self.host, self.port, db_name
            ),
            DbTypeChoice::Mysql => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.user, self.pass, self.host, self.port, db_name
            ),
            _ => String::new(),
        }
    }
}

pub struct ServerConn {
    pub form: ConnectForm,
    pub db_type: DbTypeChoice,
    pub databases: Vec<String>,
    pub db_index: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Tables,
    Data,
    Saved,
}

pub enum Screen {
    Connect,
    Databases,
    Schemas,
    Main,
    Query,
    Search,
    CrudForm,
    ConfirmDelete,
    Help,
    QueryHistory,
}

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

    fn form_value(value: &str) -> Option<String> {
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
    }
}

pub struct DeleteConfirm {
    pub sql: String,
    pub values: Vec<Option<String>>,
    pub description: String,
}

pub struct Tab {
    pub path: String,
    pub db: DbClient,
    pub schemas: Vec<String>,
    pub schema_index: usize,
    pub tables: Vec<String>,
    pub table_index: usize,
    pub result: Option<QueryResult>,
    pub filtered_rows: Vec<Vec<String>>,
    pub row_offset: usize,
    pub col_offset: usize,
    pub selected_row: usize,
    pub total_rows: i64,
    pub search_query: String,
    pub server_info: Option<(ConnectForm, DbTypeChoice)>,
    pub query_history: Vec<String>,
    pub history_index: Option<usize>,
    pub editing_cell: Option<(usize, usize)>, // (row, col)
    pub edit_buffer: String,
}

impl Tab {
    pub fn new(path: String, db: DbClient) -> Self {
        Self {
            path,
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

    pub fn export_csv(&self) -> Result<String, std::io::Error> {
        let result = match &self.result {
            Some(r) => r,
            None => return Err(std::io::Error::other("No data")),
        };
        let rows = self.display_rows();
        let safe_name = self.path.replace(['.', '/', '@', ':'], "_");
        let filename = format!("{}_export.csv", safe_name.trim_matches('_'));
        let mut content = result.columns.join(",") + "\n";
        for row in rows {
            let line: Vec<String> = row
                .iter()
                .map(|cell| {
                    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.clone()
                    }
                })
                .collect();
            content += &line.join(",");
            content += "\n";
        }
        fs::write(&filename, content)?;
        Ok(filename)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    pub db_type: DbTypeChoice,
    pub form: ConnectForm,
    pub sqlite_path: Option<String>,
}

pub struct App {
    pub screen: Screen,
    pub focus: Focus,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub sqlite_input: String,
    pub connect_form: ConnectForm,
    pub db_type: DbTypeChoice,
    pub connection_url_input: String,
    pub server_conn: Option<ServerConn>,
    pub query_input: String,
    pub search_input: String,
    pub query_history: Vec<QueryHistoryEntry>,
    pub history_index: usize,
    pub status: String,
    pub page_size: usize,
    pub read_only: bool,
    pub crud_form: Option<CrudForm>,
    pub delete_confirm: Option<DeleteConfirm>,
    pub saved_connections: Vec<SavedConnection>,
    pub saved_index: usize,
}

impl App {
    pub fn new(read_only: bool) -> Self {
        let status = if read_only {
            "READ-ONLY  |  ←/→: switch DB type  Enter: connect  Ctrl+C: quit"
        } else {
            "←/→: switch DB type  Enter: connect  Ctrl+C: quit"
        };
        let mut app = Self {
            screen: Screen::Connect,
            focus: Focus::Tables,
            tabs: vec![],
            active_tab: 0,
            sqlite_input: String::new(),
            connect_form: ConnectForm::new(&DbTypeChoice::Postgres),
            db_type: DbTypeChoice::Sqlite,
            connection_url_input: String::new(),
            server_conn: None,
            query_input: String::new(),
            search_input: String::new(),
            query_history: vec![],
            history_index: 0,
            status: status.into(),
            page_size: 100,
            read_only,
            crud_form: None,
            delete_confirm: None,
            saved_connections: vec![],
            saved_index: 0,
        };
        app.load_saved_connections();
        app
    }

    fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        let base = std::env::var("APPDATA").ok().map(PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config"));

        base.map(|b| b.join("db-eye").join("connections.json"))
    }

    fn load_saved_connections(&mut self) {
        if let Some(path) = Self::config_path()
            && let Ok(content) = fs::read_to_string(path)
            && let Ok(saved) = serde_json::from_str::<Vec<SavedConnection>>(&content)
        {
            self.saved_connections = saved;
        }
    }

    fn save_saved_connections(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&self.saved_connections) {
                let _ = fs::write(path, json);
            }
        }
    }

    fn add_saved_connection(&mut self, name: String) {
        let conn = match self.db_type {
            DbTypeChoice::Sqlite => SavedConnection {
                name,
                db_type: self.db_type.clone(),
                form: ConnectForm::default(),
                sqlite_path: Some(self.sqlite_input.clone()),
            },
            _ => SavedConnection {
                name,
                db_type: self.db_type.clone(),
                form: self.connect_form.clone(),
                sqlite_path: None,
            },
        };
        self.saved_connections.push(conn);
        self.save_saved_connections();
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    fn connect_help(&self) -> &'static str {
        if self.read_only {
            "READ-ONLY  |  ←/→: switch DB type  Enter: connect  URL optional"
        } else {
            "←/→: switch DB type  Enter: connect  URL optional"
        }
    }

    fn data_help(&self) -> &'static str {
        if self.read_only {
            "READ-ONLY  |  j/k:nav  e:export  /:search  :::query  Ctrl+H:history  Esc:back"
        } else {
            "j/k:nav  i:insert  u:update  d:delete  e:export  /:search  :::query  Ctrl+H:history  Esc:back"
        }
    }

    fn table_help(&self) -> &'static str {
        if self.read_only {
            "READ-ONLY  |  Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close  Esc:back"
        } else {
            "Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close  Esc:back"
        }
    }

    fn read_only_block(&mut self) {
        self.status = "READ-ONLY mode: write actions are disabled".into();
    }

    pub async fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        <B as Backend>::Error: 'static,
    {
        loop {
            terminal.draw(|f| ui::draw(f, self))?;
            if event::poll(Duration::from_millis(200))?
                && let Event::Key(key) = event::read()?
            {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                match self.screen {
                    Screen::Connect => self.handle_connect(key.code, key.modifiers).await,
                    Screen::Databases => self.handle_databases(key.code).await,
                    Screen::Schemas => self.handle_schemas(key.code).await,
                    Screen::Main => self.handle_main(key.code, key.modifiers).await,
                    Screen::Query => self.handle_query(key.code).await,
                    Screen::Search => self.handle_search(key.code),
                    Screen::CrudForm => self.handle_crud_form(key.code).await,
                    Screen::ConfirmDelete => self.handle_confirm_delete(key.code).await,
                    Screen::Help => self.handle_help(key.code),
                    Screen::QueryHistory => self.handle_query_history(key.code).await,
                }
            }
        }
        Ok(())
    }

    fn handle_help(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    async fn handle_schemas(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(tab) = self.current_tab_mut()
                    && tab.schema_index + 1 < tab.schemas.len()
                {
                    tab.schema_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(tab) = self.current_tab_mut()
                    && tab.schema_index > 0
                {
                    tab.schema_index -= 1;
                }
            }
            KeyCode::Enter => {
                let (schema, result) = if let Some(tab) = self.current_tab_mut() {
                    let schema = tab.current_schema().map(|s| s.to_string());
                    let res = tab.db.list_tables(schema.as_deref()).await;
                    (schema, res)
                } else {
                    return;
                };

                match result {
                    Ok(tables) => {
                        let path = if let Some(tab) = self.current_tab_mut() {
                            tab.tables = tables;
                            tab.path.clone()
                        } else {
                            String::new()
                        };
                        self.screen = Screen::Main;
                        self.focus = Focus::Tables;
                        self.server_conn = None;
                        self.status = format!(
                            "Connected: {} ({})  |  {}",
                            path,
                            schema.unwrap_or_else(|| "public".to_string()),
                            self.table_help()
                        );
                    }
                    Err(e) => {
                        self.status = format_db_error("Loading tables", &e);
                    }
                }
            }
            KeyCode::Esc => {
                if !self.tabs.is_empty() {
                    self.tabs.pop();
                    if self.tabs.is_empty() {
                        self.active_tab = 0;
                    } else {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
                self.screen = Screen::Databases;
                self.status = "j/k: navigate  Enter: open database  Esc: back".into();
            }
            _ => {}
        }
    }

    pub async fn connect_path<B: Backend>(&mut self, path: String, terminal: &mut Terminal<B>)
    where
        <B as Backend>::Error: 'static,
    {
        self.sqlite_input = path.clone();
        let _ = terminal.draw(|f| ui::draw(f, self));
        self.connect_sqlite(path).await;
    }

    async fn connect_sqlite(&mut self, path: String) {
        let url = if path.starts_with("sqlite:") {
            path.clone()
        } else {
            format!("sqlite:{}", path)
        };
        match DbClient::connect(&url).await {
            Ok(client) => {
                let mut tab = Tab::new(path.clone(), client);
                match tab.db.list_tables(None).await {
                    Ok(tables) => tab.tables = tables,
                    Err(e) => {
                        self.status = format_db_error("Loading tables", &e);
                        return;
                    }
                }
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.screen = Screen::Main;
                self.focus = Focus::Tables;
                self.sqlite_input.clear();
                self.status = format!("Connected: {}  |  {}", path, self.table_help());
            }
            Err(e) => {
                self.status = format_db_error("Connection", &e);
            }
        }
    }

    async fn connect_to_server(&mut self) {
        let url = self.connect_form.server_url(&self.db_type);
        self.status = format!(
            "Connecting to {}:{}...",
            self.connect_form.host, self.connect_form.port
        );
        match DbClient::connect(&url).await {
            Ok(client) => match client.list_databases().await {
                Ok(databases) => {
                    self.server_conn = Some(ServerConn {
                        form: self.connect_form.clone(),
                        db_type: self.db_type.clone(),
                        databases,
                        db_index: 0,
                    });
                    self.screen = Screen::Databases;
                    self.status = "j/k: navigate  Enter: open database  Esc: back".into();
                }
                Err(e) => {
                    self.status = format_db_error("Listing databases", &e);
                }
            },
            Err(e) => {
                self.status = format_db_error("Connection", &e);
            }
        }
    }

    async fn connect_to_url(&mut self, url: String) {
        let display = Self::display_connection_url(&url);
        self.status = format!("Opening {}...", display);
        match DbClient::connect(&url).await {
            Ok(client) => {
                let mut tab = Tab::new(display.clone(), client);

                if tab.db.db_type == crate::db::DbType::Postgres {
                    match tab.db.list_schemas().await {
                        Ok(schemas) => {
                            tab.schemas = schemas;
                            tab.schema_index =
                                tab.schemas.iter().position(|s| s == "public").unwrap_or(0);
                            self.tabs.push(tab);
                            self.active_tab = self.tabs.len() - 1;
                            self.screen = Screen::Schemas;
                            self.status = "j/k: navigate  Enter: select schema  Esc: back".into();
                            return;
                        }
                        Err(e) => {
                            self.status = format_db_error("Listing schemas", &e);
                            return;
                        }
                    }
                }

                match tab.db.list_tables(None).await {
                    Ok(tables) => tab.tables = tables,
                    Err(e) => {
                        self.status = format_db_error("Loading tables", &e);
                        return;
                    }
                }
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.screen = Screen::Main;
                self.focus = Focus::Tables;
                self.server_conn = None;
                self.status = format!("Connected: {}  |  {}", display, self.table_help());
            }
            Err(e) => {
                self.status = format_db_error("Connection URL", &e);
            }
        }
    }

    fn display_connection_url(url: &str) -> String {
        let Some((scheme, rest)) = url.split_once("://") else {
            return url.to_string();
        };
        let Some((userinfo, host_part)) = rest.split_once('@') else {
            return url.to_string();
        };
        let masked_userinfo = userinfo
            .split_once(':')
            .map(|(user, _)| format!("{}:***", user))
            .unwrap_or_else(|| userinfo.to_string());
        format!("{}://{}@{}", scheme, masked_userinfo, host_part)
    }

    async fn connect_to_selected_db(&mut self) {
        let (db_name, url, display, sc_form, sc_db_type) = {
            let sc = match self.server_conn.as_ref() {
                Some(s) => s,
                None => return,
            };
            let db_name = match sc.databases.get(sc.db_index) {
                Some(d) => d.clone(),
                None => return,
            };
            let url = sc.form.db_url(&sc.db_type, &db_name);
            let display = format!("{}@{}/{}", sc.form.user, sc.form.host, db_name);
            (db_name, url, display, sc.form.clone(), sc.db_type.clone())
        };

        self.status = format!("Opening {}...", db_name);
        match DbClient::connect(&url).await {
            Ok(client) => {
                let mut tab = Tab::new(display.clone(), client);
                tab.server_info = Some((sc_form, sc_db_type));

                if tab.db.db_type == crate::db::DbType::Postgres {
                    match tab.db.list_schemas().await {
                        Ok(schemas) => {
                            tab.schemas = schemas;
                            tab.schema_index =
                                tab.schemas.iter().position(|s| s == "public").unwrap_or(0);
                            self.tabs.push(tab);
                            self.active_tab = self.tabs.len() - 1;
                            self.screen = Screen::Schemas;
                            self.status = "j/k: navigate  Enter: select schema  Esc: back".into();
                            return;
                        }
                        Err(e) => {
                            self.status = format_db_error("Listing schemas", &e);
                            return;
                        }
                    }
                }

                match tab.db.list_tables(None).await {
                    Ok(tables) => tab.tables = tables,
                    Err(e) => {
                        self.status = format_db_error("Loading tables", &e);
                        return;
                    }
                }
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.screen = Screen::Main;
                self.focus = Focus::Tables;
                self.server_conn = None;
                self.status = format!("Connected: {}  |  {}", display, self.table_help());
            }
            Err(e) => {
                self.status = format_db_error("Opening database", &e);
            }
        }
    }

    async fn handle_connect(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::CONTROL) && key == KeyCode::Char('s') {
            let name = if self.db_type == DbTypeChoice::Sqlite {
                self.sqlite_input
                    .split('/')
                    .next_back()
                    .unwrap_or("SQLite")
                    .to_string()
            } else {
                format!("{}@{}", self.connect_form.user, self.connect_form.host)
            };
            self.add_saved_connection(name);
            self.status = "Connection saved".into();
            return;
        }

        match key {
            KeyCode::Left if self.focus != Focus::Saved => {
                self.db_type = self.db_type.prev();
                self.connect_form = ConnectForm::new(&self.db_type);
                self.connection_url_input.clear();
                self.status = self.connect_help().into();
            }
            KeyCode::Right if self.focus != Focus::Saved => {
                self.db_type = self.db_type.next();
                self.connect_form = ConnectForm::new(&self.db_type);
                self.connection_url_input.clear();
                self.status = self.connect_help().into();
            }
            KeyCode::Tab => {
                if self.focus == Focus::Saved {
                    self.focus = Focus::Tables; // reuse Tables as "Inputs" focus
                } else {
                    self.focus = Focus::Saved;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.focus == Focus::Saved {
                    if !self.saved_connections.is_empty() {
                        self.saved_index = (self.saved_index + 1) % self.saved_connections.len();
                    }
                } else if self.db_type != DbTypeChoice::Sqlite {
                    self.connect_form.next_field();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.focus == Focus::Saved {
                    if !self.saved_connections.is_empty() {
                        self.saved_index = self
                            .saved_index
                            .checked_sub(1)
                            .unwrap_or(self.saved_connections.len().saturating_sub(1));
                    }
                } else if self.db_type != DbTypeChoice::Sqlite {
                    self.connect_form.prev_field();
                }
            }
            KeyCode::Delete if self.focus == Focus::Saved && !self.saved_connections.is_empty() => {
                self.saved_connections.remove(self.saved_index);
                self.saved_index = self
                    .saved_index
                    .min(self.saved_connections.len().saturating_sub(1));
                self.save_saved_connections();
            }
            KeyCode::Char(c) if self.focus != Focus::Saved => match self.db_type {
                DbTypeChoice::Sqlite => self.sqlite_input.push(c),
                _ if self.connect_form.active == 4 => self.connection_url_input.push(c),
                _ => {
                    self.connect_form.active_value_mut().push(c);
                }
            },
            KeyCode::Backspace if self.focus != Focus::Saved => match self.db_type {
                DbTypeChoice::Sqlite => {
                    self.sqlite_input.pop();
                }
                _ if self.connect_form.active == 4 => {
                    self.connection_url_input.pop();
                }
                _ => {
                    self.connect_form.active_value_mut().pop();
                }
            },
            KeyCode::Enter => {
                if self.focus == Focus::Saved && !self.saved_connections.is_empty() {
                    let saved = &self.saved_connections[self.saved_index];
                    self.db_type = saved.db_type.clone();
                    if self.db_type == DbTypeChoice::Sqlite {
                        self.sqlite_input = saved.sqlite_path.clone().unwrap_or_default();
                    } else {
                        self.connect_form = saved.form.clone();
                        self.connect_form.active = 3; // Focus Password
                        self.connection_url_input.clear();
                    }
                    self.focus = Focus::Tables;
                    self.status = "Connection loaded. Enter password if needed.".into();
                } else {
                    match self.db_type {
                        DbTypeChoice::Sqlite => {
                            let path = self.sqlite_input.trim().to_string();
                            if !path.is_empty() {
                                self.connect_sqlite(path).await;
                            }
                        }
                        _ => {
                            let url = self.connection_url_input.trim().to_string();
                            if url.is_empty() {
                                self.connect_to_server().await;
                            } else {
                                self.connect_to_url(url).await;
                            }
                        }
                    }
                }
            }
            KeyCode::Esc if !self.tabs.is_empty() => {
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    async fn handle_databases(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut sc) = self.server_conn
                    && sc.db_index + 1 < sc.databases.len()
                {
                    sc.db_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut sc) = self.server_conn
                    && sc.db_index > 0
                {
                    sc.db_index -= 1;
                }
            }
            KeyCode::Enter => {
                self.connect_to_selected_db().await;
            }
            KeyCode::Esc => {
                self.server_conn = None;
                self.screen = Screen::Connect;
                self.status = self.connect_help().into();
            }
            _ => {}
        }
    }

    async fn handle_main(&mut self, key: KeyCode, modifiers: KeyModifiers) {
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

    async fn handle_data_focus(&mut self, key: KeyCode) {
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
                    match tab.export_csv() {
                        Ok(f) => self.status = format!("Exported → {}", f),
                        Err(e) => self.status = format!("Export error: {}", e),
                    }
                }
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

    async fn handle_query(&mut self, key: KeyCode) {
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

    async fn handle_query_history(&mut self, key: KeyCode) {
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

    fn handle_search(&mut self, key: KeyCode) {
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
                Some(t) => t.clone(),
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

    async fn open_insert_form(&mut self) {
        if self.read_only {
            self.read_only_block();
            return;
        }
        let table = match self.current_tab().and_then(|t| t.tables.get(t.table_index)) {
            Some(t) => t.clone(),
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

    async fn open_update_form(&mut self) {
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
                Some(t) => t.clone(),
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

    async fn open_delete_confirm(&mut self) {
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
                Some(t) => t.clone(),
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

    async fn handle_crud_form(&mut self, key: KeyCode) {
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

    async fn handle_confirm_delete(&mut self, key: KeyCode) {
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

    async fn load_table_data(&mut self) {
        let (table, offset, page_size, schema) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.clone(),
                None => return,
            };
            (
                table,
                tab.row_offset,
                self.page_size,
                tab.current_schema().map(|s| s.to_string()),
            )
        };
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            match tab.db.count_rows(schema.as_deref(), &table).await {
                Ok(count) => tab.total_rows = count,
                Err(_) => tab.total_rows = 0,
            }
            match tab
                .db
                .query_table(schema.as_deref(), &table, page_size as u32, offset as u32)
                .await
            {
                Ok(result) => {
                    tab.result = Some(result);
                    tab.search_query.clear();
                    tab.update_filter();
                    let actions = if self.read_only {
                        "READ-ONLY  /:search  e:csv  ::sql  Ctrl+H:history  q:back"
                    } else {
                        "i:insert  u:update  d:delete  /:search  e:csv  ::sql  Ctrl+H:history  q:back"
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

    fn col(name: &str, is_pk: bool) -> ColumnInfo {
        let pk_order = if is_pk { 1 } else { 0 };
        ColumnInfo {
            name: name.to_string(),
            data_type: "text".to_string(),
            is_pk,
            pk_order,
            fk_table: None,
            fk_column: None,
        }
    }

    fn form(mode: CrudMode) -> CrudForm {
        CrudForm {
            table: "users".to_string(),
            schema: None,
            columns: vec![col("id", true), col("name", false), col("email", false)],
            values: vec!["1".to_string(), "Ada".to_string(), "\\null".to_string()],
            fk_hints: vec![vec![], vec![], vec![]],
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
                },
                ColumnInfo {
                    name: "user_id".to_string(),
                    data_type: "text".to_string(),
                    is_pk: true,
                    pk_order: 2,
                    fk_table: None,
                    fk_column: None,
                },
                col("name", false),
            ],
            values: vec!["t1".to_string(), "u1".to_string(), "Ada".to_string()],
            fk_hints: vec![vec![], vec![], vec![]],
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
