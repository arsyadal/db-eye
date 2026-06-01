mod app;
mod db;
mod ui;

use app::App;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

fn parse_args() -> (Option<String>, bool, bool) {
    let mut path = None;
    let mut read_only = false;
    let mut help = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-r" | "--read-only" => read_only = true,
            "-h" | "--help" => help = true,
            _ if path.is_none() => path = Some(arg),
            _ => {}
        }
    }

    (path, read_only, help)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (path_arg, read_only, help) = parse_args();
    if help {
        println!("Usage: db-eye [--read-only|-r] [sqlite-file]");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(read_only);
    if let Some(path) = path_arg {
        app.connect_path(path, &mut terminal).await;
    }
    let result = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
