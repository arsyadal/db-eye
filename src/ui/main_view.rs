use super::data_panel::draw_data_panel;
use crate::app::{App, Focus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub fn draw_main(f: &mut Frame, app: &App) {
    let area = f.area();
    let tabs_height: u16 = if app.tabs.len() > 1 { 1 } else { 0 };

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tabs_height),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    if app.tabs.len() > 1 {
        let spans: Vec<Span> = app
            .tabs
            .iter()
            .enumerate()
            .flat_map(|(i, tab)| {
                let label = format!(" {} ", tab.short_name());
                let style = if i == app.active_tab {
                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                vec![Span::styled(label, style), Span::raw("│")]
            })
            .collect();
        f.render_widget(Paragraph::new(Line::from(spans)), vert[0]);
    }

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(vert[1]);

    draw_tables_panel(f, app, horiz[0]);
    draw_data_panel(f, app, horiz[1]);

    f.render_widget(
        Paragraph::new(app.status.as_str()).block(Block::default().borders(Borders::ALL)),
        vert[2],
    );
}

fn draw_tables_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = matches!(app.focus, Focus::Tables);
    let border_style = if focused {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tab = match app.current_tab() {
        Some(t) => t,
        None => {
            f.render_widget(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Tables ")
                    .border_style(border_style),
                area,
            );
            return;
        }
    };

    let rows: Vec<Row> = tab
        .tables
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let mut name = entry.name.clone();
            if name.len() > 18 {
                name.truncate(17);
                name.push('…');
            }
            let display = if entry.is_view {
                format!("{} (view)", name)
            } else {
                name
            };
            let style = if i == tab.table_index && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if i == tab.table_index {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(display)]).style(style)
        })
        .collect();

    let schema_label = tab
        .current_schema()
        .map(|s| format!(" [{}]", s))
        .unwrap_or_default();
    let table = Table::new(rows, [Constraint::Percentage(100)]).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Tables ({}){} ", tab.tables.len(), schema_label))
            .border_style(border_style),
    );
    f.render_widget(table, area);
}

pub fn draw_databases(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let sc = match app.server_conn.as_ref() {
        Some(s) => s,
        None => return,
    };

    let title = format!(
        " Databases — {}@{} ({}) ",
        sc.form.user,
        sc.form.host,
        sc.db_type.label()
    );

    let rows: Vec<Row> = sc
        .databases
        .iter()
        .enumerate()
        .map(|(i, db)| {
            let style = if i == sc.db_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(db.as_str())]).style(style)
        })
        .collect();

    let table = Table::new(rows, [Constraint::Percentage(100)])
        .header(
            Row::new(vec!["Database"])
                .style(Style::default().add_modifier(Modifier::BOLD))
                .bottom_margin(1),
        )
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(table, chunks[0]);
    f.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL).title(" Keys ")),
        chunks[1],
    );
}

pub fn draw_schemas(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let tab = match app.current_tab() {
        Some(t) => t,
        None => return,
    };

    let title = format!(" Schemas — {} ", tab.path);

    let rows: Vec<Row> = tab
        .schemas
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == tab.schema_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(s.as_str())]).style(style)
        })
        .collect();

    let table = Table::new(rows, [Constraint::Percentage(100)])
        .header(
            Row::new(vec!["Schema"])
                .style(Style::default().add_modifier(Modifier::BOLD))
                .bottom_margin(1),
        )
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(table, chunks[0]);
    f.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL).title(" Keys ")),
        chunks[1],
    );
}
