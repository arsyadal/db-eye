mod app;
mod db;
mod ui;
pub mod update;
use app::App;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

fn parse_args() -> (Option<String>, bool, bool, bool, bool) {
    let mut path = None;
    let mut read_only = false;
    let mut help = false;
    let mut is_update = false;
    let mut version = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "update" => is_update = true,
            "-r" | "--read-only" => read_only = true,
            "-v" | "--version" => version = true,
            "-h" | "--help" => help = true,
            _ if path.is_none() => path = Some(arg),
            _ => {}
        }
    }

    (path, read_only, help, is_update, version)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (path_arg, read_only, help, is_update, version) = parse_args();
    if version {
        println!("db-eye v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if is_update {
        update::handle_update_command()?;
        return Ok(());
    }
    if help {
        println!("Usage: db-eye [OPTIONS] [COMMAND] [sqlite-file]");
        println!();
        println!("Commands:");
        println!("  update                 Check for and install updates to db-eye");
        println!();
        println!("Options:");
        println!("  -r, --read-only        Start in read-only mode");
        println!("  -v, --version          Print version information");
        println!("  -h, --help             Print help information");
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
