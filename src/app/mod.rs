mod connect;
mod crud;
mod crud_actions;
mod data_panel;
mod query;
mod tabs;

pub use connect::{ConnectForm, DbTypeChoice, SavedConnection, ServerConn};
pub use crud::{CrudForm, CrudMode, DeleteConfirm};
pub use query::{QueryHistoryEntry, format_duration_ms};
pub use tabs::Tab;

use crate::ui;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use std::time::Duration;

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
    TableStats,
    Jump,
    MultilineEditor,
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
    pub jump_input: String,
    pub multiline_buffer: String,
    pub multiline_cursor: usize,
    pub query_history: Vec<QueryHistoryEntry>,
    pub history_index: usize,
    pub status: String,
    pub page_size: usize,
    pub read_only: bool,
    pub crud_form: Option<CrudForm>,
    pub delete_confirm: Option<DeleteConfirm>,
    pub saved_connections: Vec<SavedConnection>,
    pub saved_index: usize,
    pub table_stats: Option<crate::db::TableStats>,
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
            jump_input: String::new(),
            multiline_buffer: String::new(),
            multiline_cursor: 0,
            query_history: vec![],
            history_index: 0,
            status: status.into(),
            page_size: 100,
            read_only,
            crud_form: None,
            delete_confirm: None,
            saved_connections: vec![],
            saved_index: 0,
            table_stats: None,
        };
        app.load_saved_connections();
        app
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
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
                    Screen::TableStats => self.handle_table_stats(key.code),
                    Screen::Jump => self.handle_jump(key.code).await,
                    Screen::MultilineEditor => self.handle_multiline_editor(key.code),
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

    fn handle_table_stats(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => {
                self.table_stats = None;
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }
}
