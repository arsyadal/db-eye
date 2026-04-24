use crate::app::{App, ConnectForm, DbTypeChoice, Focus, Screen};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Connect => draw_connect(f, app),
        Screen::Databases => draw_databases(f, app),
        Screen::Main => draw_main(f, app),
        Screen::Query => {
            draw_main(f, app);
            draw_query_popup(f, app);
        }
        Screen::Search => {
            draw_main(f, app);
            draw_search_bar(f, app);
        }
    }
}

// ── Connect screen ────────────────────────────────────────────────────────────

fn draw_connect(f: &mut Frame, app: &App) {
    match app.db_type {
        DbTypeChoice::Sqlite => draw_connect_sqlite(f, app),
        _ => draw_connect_server(f, app),
    }
}

fn db_type_selector<'a>(app: &'a App) -> Paragraph<'a> {
    let spans: Vec<Span> = DbTypeChoice::all()
        .iter()
        .flat_map(|t| {
            let active = t == &app.db_type;
            let label = if active {
                format!(" [{}] ", t.label())
            } else {
                format!("  {}  ", t.label())
            };
            let style = if active {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            vec![Span::styled(label, style)]
        })
        .collect();
    Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(" DB Type  ←/→ "))
}

fn draw_connect_sqlite(f: &mut Frame, app: &App) {
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
        .margin(6)
        .split(f.area());

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("DB-EYE", Style::default().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(
                "Database Browser",
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        chunks[1],
    );
    f.render_widget(db_type_selector(app), chunks[2]);
    f.render_widget(
        Paragraph::new(app.sqlite_input.as_str())
            .block(Block::default().borders(Borders::ALL).title(" File Path ")),
        chunks[3],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  e.g. {}", app.db_type.hint()),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[4],
    );
    f.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL).title(" Status ")),
        chunks[5],
    );
    f.set_cursor_position((chunks[3].x + app.sqlite_input.len() as u16 + 1, chunks[3].y + 1));
}

fn draw_connect_server(f: &mut Frame, app: &App) {
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
            Constraint::Length(3), // pass
            Constraint::Length(3), // status
            Constraint::Min(0),
        ])
        .margin(4)
        .split(f.area());

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("DB-EYE", Style::default().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(
                "Database Browser",
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        chunks[1],
    );
    f.render_widget(db_type_selector(app), chunks[2]);

    let labels = ConnectForm::labels();
    let values = form.values();
    let field_chunks = [chunks[3], chunks[4], chunks[5], chunks[6]];

    for (i, (label, chunk)) in labels.iter().zip(field_chunks.iter()).enumerate() {
        let active = i == form.active;
        let display = if *label == "Password" {
            "*".repeat(values[i].len())
        } else {
            values[i].to_string()
        };
        let border_style = if active {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(display).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", label))
                    .border_style(border_style),
            ),
            *chunk,
        );
    }

    f.render_widget(
        Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL).title(" Status ")),
        chunks[7],
    );

    // Cursor on active field
    let active_chunk = field_chunks[form.active];
    let active_len = if labels[form.active] == "Password" {
        values[form.active].len()
    } else {
        values[form.active].len()
    };
    f.set_cursor_position((active_chunk.x + active_len as u16 + 1, active_chunk.y + 1));
}

// ── Databases screen ──────────────────────────────────────────────────────────

fn draw_databases(f: &mut Frame, app: &App) {
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

// ── Main screen ───────────────────────────────────────────────────────────────

fn draw_main(f: &mut Frame, app: &App) {
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
        Paragraph::new(app.status.as_str())
            .block(Block::default().borders(Borders::ALL)),
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
        .map(|(i, name)| {
            let mut display = name.clone();
            if display.len() > 18 {
                display.truncate(17);
                display.push('…');
            }
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

    let table = Table::new(rows, [Constraint::Percentage(100)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Tables ({}) ", tab.tables.len()))
                .border_style(border_style),
        );
    f.render_widget(table, area);
}

fn draw_data_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = matches!(app.focus, Focus::Data);
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
                    .title(" Data ")
                    .border_style(border_style),
                area,
            );
            return;
        }
    };

    if tab.result.is_none() {
        f.render_widget(
            Paragraph::new("\n  Enter: open table  Tab: switch focus")
                .block(Block::default().borders(Borders::ALL).title(" Data ").border_style(border_style)),
            area,
        );
        return;
    }

    let result = tab.result.as_ref().unwrap();
    let display_rows = tab.display_rows();
    let col_count = result.columns.len();
    let available_w = area.width.saturating_sub(2) as usize;
    let col_offset = tab.col_offset.min(col_count.saturating_sub(1));
    let constraints = auto_col_widths(&result.columns, display_rows, col_offset, available_w);
    let visible = constraints.len();

    let header = Row::new(
        result
            .columns
            .iter()
            .skip(col_offset)
            .take(visible)
            .map(|c| Cell::from(c.as_str()).style(Style::default().add_modifier(Modifier::BOLD)))
            .collect::<Vec<_>>(),
    )
    .bottom_margin(1);

    let rows: Vec<Row> = display_rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let cells: Vec<Cell> = row
                .iter()
                .skip(col_offset)
                .take(visible)
                .map(|v| Cell::from(v.as_str()))
                .collect();
            let style = if i == tab.selected_row && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if i == tab.selected_row {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(cells).style(style)
        })
        .collect();

    let title = if !tab.search_query.is_empty() {
        format!(
            " /{} — {}/{} rows ",
            tab.search_query,
            display_rows.len(),
            tab.total_rows
        )
    } else {
        format!(
            " row {}/{} col {} ",
            tab.row_offset + tab.selected_row + 1,
            tab.total_rows,
            col_offset
        )
    };

    let mut state = TableState::default();
    state.select(Some(tab.selected_row));

    f.render_stateful_widget(
        Table::new(rows, constraints)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style)),
        area,
        &mut state,
    );
}

fn auto_col_widths(
    columns: &[String],
    rows: &[Vec<String>],
    col_offset: usize,
    available_w: usize,
) -> Vec<Constraint> {
    let mut constraints = vec![];
    let mut used = 0usize;

    for (i, col) in columns.iter().enumerate().skip(col_offset) {
        let header_w = col.len();
        let max_cell_w = rows
            .iter()
            .filter_map(|r| r.get(i))
            .map(|c| c.len())
            .max()
            .unwrap_or(0);
        let w = (header_w.max(max_cell_w).max(4).min(28) + 2) as u16;
        if used + w as usize + 1 > available_w {
            break;
        }
        used += w as usize + 1;
        constraints.push(Constraint::Length(w));
    }

    if constraints.is_empty() {
        constraints.push(Constraint::Min(10));
    }
    constraints
}

// ── Popups ────────────────────────────────────────────────────────────────────

fn draw_query_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(75, 5, f.area());
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(format!(": {}_", app.query_input)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" SQL Query — Enter: run  Esc: cancel "),
        ),
        area,
    );
    f.set_cursor_position((area.x + app.query_input.len() as u16 + 3, area.y + 1));
}

fn draw_search_bar(f: &mut Frame, app: &App) {
    let area = f.area();
    let bar_area = Rect {
        x: 22,
        y: area.height.saturating_sub(6),
        width: area.width.saturating_sub(22),
        height: 3,
    };
    f.render_widget(Clear, bar_area);
    f.render_widget(
        Paragraph::new(format!("/{}_", app.search_input)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search — Enter/Esc: close "),
        ),
        bar_area,
    );
    f.set_cursor_position((
        bar_area.x + app.search_input.len() as u16 + 2,
        bar_area.y + 1,
    ));
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100u16.saturating_sub(height * 4)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}
