use super::{App, Focus, Screen, Tab};
use crate::db::{DbClient, format_db_error};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

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

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    pub db_type: DbTypeChoice,
    pub form: ConnectForm,
    pub sqlite_path: Option<String>,
}

impl App {
    fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        let base = std::env::var("APPDATA").ok().map(PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config"));

        base.map(|b| b.join("db-eye").join("connections.json"))
    }

    pub(super) fn load_saved_connections(&mut self) {
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

    pub(super) fn connect_help(&self) -> &'static str {
        if self.read_only {
            "READ-ONLY  |  ←/→: switch DB type  Enter: connect  URL optional"
        } else {
            "←/→: switch DB type  Enter: connect  URL optional"
        }
    }

    pub(super) async fn handle_schemas(&mut self, key: KeyCode) {
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
        let _ = terminal.draw(|f| crate::ui::draw(f, self));
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
                let mut tab = Tab::new(path.clone(), url.clone(), client);
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
                let mut tab = Tab::new(display.clone(), url.clone(), client);

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
                let mut tab = Tab::new(display.clone(), url.clone(), client);
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

    pub(super) async fn handle_connect(&mut self, key: KeyCode, modifiers: KeyModifiers) {
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

    pub(super) async fn handle_databases(&mut self, key: KeyCode) {
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
}
