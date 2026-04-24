use crate::db::{DbClient, QueryResult};
use crate::ui;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::Backend, Terminal};
use std::{fs, time::Duration};

#[derive(Clone, PartialEq)]
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

#[derive(Clone, Default)]
pub struct ConnectForm {
    pub host: String,
    pub port: String,
    pub user: String,
    pub pass: String,
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
        self.active = (self.active + 1) % 4;
    }

    pub fn prev_field(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(3);
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

pub enum Focus {
    Tables,
    Data,
}

pub enum Screen {
    Connect,
    Databases,
    Main,
    Query,
    Search,
}

pub struct Tab {
    pub path: String,
    pub db: DbClient,
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
}

impl Tab {
    pub fn new(path: String, db: DbClient) -> Self {
        Self {
            path,
            db,
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
        }
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
            .last()
            .unwrap_or(&self.path)
            .to_string()
    }

    pub fn export_csv(&self) -> Result<String, std::io::Error> {
        let result = match &self.result {
            Some(r) => r,
            None => return Err(std::io::Error::new(std::io::ErrorKind::Other, "No data")),
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

pub struct App {
    pub screen: Screen,
    pub focus: Focus,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub sqlite_input: String,
    pub connect_form: ConnectForm,
    pub db_type: DbTypeChoice,
    pub server_conn: Option<ServerConn>,
    pub query_input: String,
    pub search_input: String,
    pub status: String,
    pub page_size: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Connect,
            focus: Focus::Tables,
            tabs: vec![],
            active_tab: 0,
            sqlite_input: String::new(),
            connect_form: ConnectForm::new(&DbTypeChoice::Postgres),
            db_type: DbTypeChoice::Sqlite,
            server_conn: None,
            query_input: String::new(),
            search_input: String::new(),
            status: "←/→: switch DB type  Enter: connect  Ctrl+C: quit".into(),
            page_size: 100,
        }
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
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
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    match self.screen {
                        Screen::Connect => self.handle_connect(key.code).await,
                        Screen::Databases => self.handle_databases(key.code).await,
                        Screen::Main => self.handle_main(key.code, key.modifiers).await,
                        Screen::Query => self.handle_query(key.code).await,
                        Screen::Search => self.handle_search(key.code),
                    }
                }
            }
        }
        Ok(())
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
                match tab.db.list_tables().await {
                    Ok(tables) => tab.tables = tables,
                    Err(e) => {
                        self.status = format!("Error: {}", e);
                        return;
                    }
                }
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.screen = Screen::Main;
                self.focus = Focus::Tables;
                self.sqlite_input.clear();
                self.status =
                    format!("Connected: {}  |  Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close", path);
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
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
                    self.status = format!("Error listing databases: {}", e);
                }
            },
            Err(e) => {
                self.status = format!("Connection failed: {}", e);
            }
        }
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
                match tab.db.list_tables().await {
                    Ok(tables) => tab.tables = tables,
                    Err(e) => {
                        self.status = format!("Error loading tables: {}", e);
                        return;
                    }
                }
                tab.server_info = Some((sc_form, sc_db_type));
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
                self.screen = Screen::Main;
                self.focus = Focus::Tables;
                self.server_conn = None;
                self.status =
                    format!("Connected: {}  |  Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close  Esc:back-to-db-list", display);
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
            }
        }
    }

    async fn handle_connect(&mut self, key: KeyCode) {
        match key {
            KeyCode::Left => {
                self.db_type = self.db_type.prev();
                self.connect_form = ConnectForm::new(&self.db_type);
                self.status = "←/→: switch DB type  Enter: connect".into();
            }
            KeyCode::Right => {
                self.db_type = self.db_type.next();
                self.connect_form = ConnectForm::new(&self.db_type);
                self.status = "←/→: switch DB type  Enter: connect".into();
            }
            KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
                if self.db_type != DbTypeChoice::Sqlite {
                    self.connect_form.next_field();
                }
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
                if self.db_type != DbTypeChoice::Sqlite {
                    self.connect_form.prev_field();
                }
            }
            KeyCode::Char(c) => match self.db_type {
                DbTypeChoice::Sqlite => self.sqlite_input.push(c),
                _ => {
                    self.connect_form.active_value_mut().push(c);
                }
            },
            KeyCode::Backspace => match self.db_type {
                DbTypeChoice::Sqlite => {
                    self.sqlite_input.pop();
                }
                _ => {
                    self.connect_form.active_value_mut().pop();
                }
            },
            KeyCode::Enter => match self.db_type {
                DbTypeChoice::Sqlite => {
                    let path = self.sqlite_input.trim().to_string();
                    if !path.is_empty() {
                        self.connect_sqlite(path).await;
                    }
                }
                _ => {
                    self.connect_to_server().await;
                }
            },
            KeyCode::Esc => {
                if !self.tabs.is_empty() {
                    self.screen = Screen::Main;
                }
            }
            _ => {}
        }
    }

    async fn handle_databases(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut sc) = self.server_conn {
                    if sc.db_index + 1 < sc.databases.len() {
                        sc.db_index += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut sc) = self.server_conn {
                    if sc.db_index > 0 {
                        sc.db_index -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                self.connect_to_selected_db().await;
            }
            KeyCode::Esc => {
                self.server_conn = None;
                self.screen = Screen::Connect;
                self.status = "←/→: switch DB type  Enter: connect".into();
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
                _ => {}
            }
        }

        // Tab switching with [ and ] (Mac-friendly, no Ctrl+arrow)
        match key {
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
                    Focus::Tables => Focus::Data,
                    Focus::Data => Focus::Tables,
                };
            }
            _ => match self.focus {
                Focus::Tables => self.handle_tables_focus(key).await,
                Focus::Data => self.handle_data_focus(key).await,
            },
        }
    }

    async fn handle_tables_focus(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(tab) = self.current_tab_mut() {
                    if tab.table_index + 1 < tab.tables.len() {
                        tab.table_index += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(tab) = self.current_tab_mut() {
                    if tab.table_index > 0 {
                        tab.table_index -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                self.load_table_data().await;
                self.focus = Focus::Data;
            }
            KeyCode::Esc => {
                self.go_back().await;
            }
            _ => {}
        }
    }

    async fn go_back(&mut self) {
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
                        self.status = format!("Error: {}", e);
                        self.screen = Screen::Connect;
                    }
                },
                Err(e) => {
                    self.status = format!("Reconnect failed: {}", e);
                    self.screen = Screen::Connect;
                }
            }
        } else {
            self.screen = Screen::Connect;
            self.status = "←/→: switch DB type  Enter: connect".into();
        }
    }

    async fn handle_data_focus(&mut self, key: KeyCode) {
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
                } else if let Some(t) = self.current_tab_mut() {
                    if t.selected_row > 0 {
                        t.selected_row -= 1;
                    }
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(t) = self.current_tab_mut() {
                    t.col_offset += 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(t) = self.current_tab_mut() {
                    if t.col_offset > 0 {
                        t.col_offset -= 1;
                    }
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
            KeyCode::Char('e') => {
                if let Some(tab) = self.current_tab() {
                    match tab.export_csv() {
                        Ok(f) => self.status = format!("Exported → {}", f),
                        Err(e) => self.status = format!("Export error: {}", e),
                    }
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
                self.status =
                    "Tab:focus  j/k:nav  Enter:open  [:prev-tab  ]:next-tab  Ctrl+T:new  Ctrl+W:close  Esc:back".into();
            }
            _ => {}
        }
    }

    async fn handle_query(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => self.query_input.push(c),
            KeyCode::Backspace => {
                self.query_input.pop();
            }
            KeyCode::Enter => {
                let sql = self.query_input.trim().to_string();
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    match tab.db.execute_query(&sql).await {
                        Ok(result) => {
                            let count = result.rows.len();
                            tab.total_rows = count as i64;
                            tab.result = Some(result);
                            tab.search_query.clear();
                            tab.update_filter();
                            tab.selected_row = 0;
                            tab.row_offset = 0;
                            tab.col_offset = 0;
                            self.status = format!("{} rows returned", count);
                        }
                        Err(e) => {
                            self.status = format!("Query error: {}", e);
                        }
                    }
                }
                self.screen = Screen::Main;
            }
            KeyCode::Esc => {
                self.screen = Screen::Main;
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

    async fn load_table_data(&mut self) {
        let (table, offset, page_size) = {
            let tab = match self.current_tab() {
                Some(t) => t,
                None => return,
            };
            let table = match tab.tables.get(tab.table_index) {
                Some(t) => t.clone(),
                None => return,
            };
            (table, tab.row_offset, self.page_size)
        };
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            match tab.db.count_rows(&table).await {
                Ok(count) => tab.total_rows = count,
                Err(_) => tab.total_rows = 0,
            }
            match tab
                .db
                .query_table(&table, page_size as u32, offset as u32)
                .await
            {
                Ok(result) => {
                    tab.result = Some(result);
                    tab.search_query.clear();
                    tab.update_filter();
                    self.status = format!(
                        "{}  |  {} rows  |  Tab:focus  j/k:scroll  h/l:cols  /:search  e:csv  ::sql  q:back",
                        table, tab.total_rows
                    );
                }
                Err(e) => self.status = format!("Error: {}", e),
            }
        }
    }
}
