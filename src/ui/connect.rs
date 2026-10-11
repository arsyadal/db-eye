use super::components::{db_type_selector, format_status_to_spans, render_input_box};
use crate::app::{App, ConnectForm, DbTypeChoice, Focus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub fn draw_connect(f: &mut Frame, app: &App) {
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(f.area());

    match app.db_type {
        DbTypeChoice::Sqlite => draw_connect_sqlite(f, app, horiz[0]),
        _ => draw_connect_server(f, app, horiz[0]),
    }
    draw_saved_connections(f, app, horiz[1]);
}

fn draw_saved_connections(f: &mut Frame, app: &App, area: Rect) {
    let focused = matches!(app.focus, Focus::Saved);
    let border_style = if focused {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let rows: Vec<Row> = app
        .saved_connections
        .iter()
        .enumerate()
        .map(|(i, conn)| {
            let style = if i == app.saved_index && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if i == app.saved_index {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let label = format!("{} ({})", conn.name, conn.db_type.label());
            Row::new(vec![Cell::from(label)]).style(style)
        })
        .collect();

    let table = Table::new(rows, [Constraint::Percentage(100)]).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Saved (Tab) ")
            .border_style(border_style),
    );
    f.render_widget(table, area);
}

fn draw_connect_sqlite(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .margin(2)
        .split(area);

    let mut title_lines = vec![
        Line::from(vec![
            Span::styled("DB-EYE", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "Database Browser",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    if let Some(update) = &app.update_notice {
        title_lines.push(Line::from(Span::styled(
            format!("✨ Update available: {update} (run: db-eye update)"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
    }
    f.render_widget(Paragraph::new(title_lines), chunks[1]);
    f.render_widget(db_type_selector(app), chunks[2]);

    let is_active = app.focus != Focus::Saved;
    render_input_box(
        f,
        chunks[3],
        "File Path",
        &app.sqlite_input,
        app.sqlite_input.len(),
        Style::default(),
        is_active,
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  e.g. {} (Enter to connect)", app.db_type.hint()),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[4],
    );
    f.render_widget(
        Paragraph::new(format_status_to_spans(app.status.as_str()))
            .block(Block::default().borders(Borders::ALL).title(" Status ")),
        chunks[5],
    );
}

fn draw_connect_server(f: &mut Frame, app: &App, area: Rect) {
    let form = &app.connect_form;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3), // title
            Constraint::Length(3), // db type
            Constraint::Length(3), // host
            Constraint::Length(3), // port
            Constraint::Length(3), // user
            Constraint::Length(3), // password
            Constraint::Length(3), // connection url
            Constraint::Length(3), // status
            Constraint::Min(0),
        ])
        .margin(2)
        .split(area);

    let mut title_lines = vec![
        Line::from(vec![
            Span::styled("DB-EYE", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "Database Browser",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    if let Some(update) = &app.update_notice {
        title_lines.push(Line::from(Span::styled(
            format!("✨ Update available: {update} (run: db-eye update)"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
    }
    f.render_widget(Paragraph::new(title_lines), chunks[1]);
    f.render_widget(db_type_selector(app), chunks[2]);

    let labels = ConnectForm::labels();
    let values = form.values();
    let field_chunks = [chunks[3], chunks[4], chunks[5], chunks[6]];

    for (i, (label, chunk)) in labels.iter().zip(field_chunks.iter()).enumerate() {
        let active = i == form.active && app.focus != Focus::Saved;
        let border_style = if active {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let val = &values[i];
        let display = if *label == "Password" {
            "*".repeat(val.len())
        } else {
            val.to_string()
        };

        render_input_box(f, *chunk, label, &display, val.len(), border_style, active);
    }

    let url_active = form.active == 4 && app.focus != Focus::Saved;
    let url_border_style = if url_active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    render_input_box(
        f,
        chunks[7],
        "Connection URL (optional)",
        &app.connection_url_input,
        app.connection_url_input.len(),
        url_border_style,
        url_active,
    );

    f.render_widget(
        Paragraph::new(format_status_to_spans(app.status.as_str()))
            .block(Block::default().borders(Borders::ALL).title(" Status ")),
        chunks[8],
    );
}
