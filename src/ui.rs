use crate::app::{App, ConnectForm, CrudMode, DbTypeChoice, Focus, Screen};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};

pub fn draw(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Connect => draw_connect(f, app),
        Screen::Databases => draw_databases(f, app),
        Screen::Schemas => draw_schemas(f, app),
        Screen::Main => draw_main(f, app),
        Screen::Query => {
            draw_main(f, app);
            draw_query_popup(f, app);
        }
        Screen::Search => {
            draw_main(f, app);
            draw_search_bar(f, app);
        }
        Screen::CrudForm => draw_crud_form(f, app),
        Screen::ConfirmDelete => {
            draw_main(f, app);
            draw_confirm_delete(f, app);
        }
        Screen::Help => {
            draw_main(f, app);
            draw_help_popup(f);
        }
    }
}

// ── Connect screen ────────────────────────────────────────────────────────────

fn draw_connect(f: &mut Frame, app: &App) {
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
    Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" DB Type  ←/→ "),
    )
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

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "DB-EYE",
                Style::default().add_modifier(Modifier::BOLD),
            )),
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
    if app.focus != Focus::Saved {
        f.set_cursor_position((
            chunks[3].x + app.sqlite_input.len() as u16 + 1,
            chunks[3].y + 1,
        ));
    }
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
            Constraint::Length(3), // pass
            Constraint::Length(3), // status
            Constraint::Min(0),
        ])
        .margin(2)
        .split(area);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "DB-EYE",
                Style::default().add_modifier(Modifier::BOLD),
            )),
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
        let active = i == form.active && app.focus != Focus::Saved;
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
    if app.focus != Focus::Saved {
        let active_chunk = field_chunks[form.active];
        let active_len = values[form.active].len();
        f.set_cursor_position((active_chunk.x + active_len as u16 + 1, active_chunk.y + 1));
    }
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

fn draw_schemas(f: &mut Frame, app: &App) {
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
            Paragraph::new("\n  Enter: open table  Tab: switch focus").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Data ")
                    .border_style(border_style),
            ),
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
                .enumerate()
                .skip(col_offset)
                .take(visible)
                .map(|(j, v)| {
                    let mut style = Style::default();
                    let mut display_val = v.clone();

                    if i == tab.selected_row && j == tab.col_offset && focused {
                        if let Some((edit_r, edit_c)) = tab.editing_cell {
                            if i == edit_r && j == edit_c {
                                style = style.bg(Color::Yellow).fg(Color::Black);
                                display_val = tab.edit_buffer.clone();
                            } else {
                                style = style.add_modifier(Modifier::REVERSED);
                            }
                        } else {
                            style = style.add_modifier(Modifier::REVERSED);
                        }
                    } else if i == tab.selected_row {
                        style = style.add_modifier(Modifier::BOLD);
                    }

                    Cell::from(display_val).style(style)
                })
                .collect();
            Row::new(cells)
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
            col_offset + 1
        )
    };

    let mut state = TableState::default();
    state.select(Some(tab.selected_row));

    f.render_stateful_widget(
        Table::new(rows, constraints.clone())
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .column_spacing(2),
        area,
        &mut state,
    );

    // Cursor for inline edit
    if let Some((edit_r, edit_c)) = tab.editing_cell {
        if focused && edit_c >= col_offset && edit_c < col_offset + visible {
            let mut x_offset: u16 = 1; // Left border
            for (i, constraint) in constraints.iter().enumerate() {
                if col_offset + i == edit_c {
                    break;
                }
                match constraint {
                    Constraint::Length(l) => x_offset += *l + 2, // +2 for column_spacing
                    _ => x_offset += 20,
                }
            }

            // Stateful Table positioning is complex to calculate exactly, 
            // but for simple cases where current row is visible:
            let y = area.y + 3 + edit_r.saturating_sub(state.offset()) as u16;
            if y < area.y + area.height - 1 && x_offset + (tab.edit_buffer.len() as u16) < area.x + area.width - 1 {
                f.set_cursor_position((area.x + x_offset + tab.edit_buffer.len() as u16, y));
            }
        }
    }
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

fn draw_crud_form(f: &mut Frame, app: &App) {
    let form = match app.crud_form.as_ref() {
        Some(f) => f,
        None => return,
    };

    let title = match form.mode {
        CrudMode::Insert => format!(" Insert — {} ", form.table),
        CrudMode::Update => format!(" Update — {} ", form.table),
    };

    let area = f.area();
    let active_hints = form
        .fk_hints
        .get(form.active_field)
        .map(|h| h.as_slice())
        .unwrap_or(&[]);
    let hint_height: u16 = if active_hints.is_empty() { 0 } else { 3 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(hint_height),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .margin(2)
        .split(area);

    // Fields
    let row_height = 3u16;
    let max_visible = (chunks[0].height / row_height) as usize;
    let active = form.active_field;
    let start = if active >= max_visible {
        active + 1 - max_visible
    } else {
        0
    };

    let field_area = chunks[0];
    let mut y = field_area.y;

    for (i, col) in form
        .columns
        .iter()
        .enumerate()
        .skip(start)
        .take(max_visible)
    {
        let is_active = i == active;
        let is_pk = col.is_pk;
        let display_val = if is_pk && form.mode == CrudMode::Update {
            format!("{} [PK - read only]", form.values[i])
        } else {
            form.values[i].clone()
        };

        let editable = !is_pk || form.mode == CrudMode::Insert;
        let border_style = if is_active && editable {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let fk_label = col
            .fk_table
            .as_deref()
            .map(|t| format!(" {} ({}) → {} ", col.name, col.data_type, t))
            .unwrap_or_else(|| format!(" {} ({}) ", col.name, col.data_type));
        let field_rect = Rect {
            x: field_area.x,
            y,
            width: field_area.width,
            height: row_height,
        };
        f.render_widget(
            Paragraph::new(display_val.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(fk_label)
                    .border_style(border_style),
            ),
            field_rect,
        );
        if is_active && editable {
            f.set_cursor_position((
                field_rect.x + display_val.len() as u16 + 1,
                field_rect.y + 1,
            ));
        }
        y += row_height;
    }

    // FK hint (valid values for active FK field)
    if hint_height > 0 {
        let vals_text = active_hints
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("  |  ");
        f.render_widget(
            Paragraph::new(format!("  {}", vals_text)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Valid Values ")
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            chunks[1],
        );
    }

    // SQL Preview
    let sql = form.build_sql();
    f.render_widget(
        Paragraph::new(sql.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" SQL Preview ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        chunks[2],
    );

    // Keys / error
    let hint = if app.status.starts_with("Error") {
        app.status.as_str()
    } else {
        "  Tab or ↑↓: navigate fields    Enter: save    Esc: cancel"
    };
    let hint_style = if app.status.starts_with("Error") {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(hint).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(hint_style),
        ),
        chunks[3],
    );
}

fn draw_confirm_delete(f: &mut Frame, app: &App) {
    let confirm = match app.delete_confirm.as_ref() {
        Some(c) => c,
        None => return,
    };

    let area = centered_rect(60, 10, f.area());
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let content = format!(
        "\n  {}\n\n  SQL: {}\n\n  Tekan Y untuk konfirmasi, N untuk batal",
        confirm.description, confirm.sql
    );
    f.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚠ Konfirmasi Delete ")
                .border_style(Style::default().add_modifier(Modifier::BOLD)),
        ),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new("  [Y] Ya, hapus     [N] Batal")
            .block(Block::default().borders(Borders::ALL)),
        chunks[1],
    );
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

fn draw_help_popup(f: &mut Frame) {
    let area = centered_rect(80, 20, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(vec![
            Span::styled(" DB-EYE Keybindings ", Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" General ", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))),
        Line::from(" Ctrl+C      Quit application"),
        Line::from(" ?          Show/hide this help screen"),
        Line::from(" Esc        Go back / Cancel"),
        Line::from(""),
        Line::from(Span::styled(" Connection & Navigation ", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))),
        Line::from(" Ctrl+T      New connection (Connect screen)"),
        Line::from(" Ctrl+S      Save connection (Connect screen)"),
        Line::from(" Tab        Switch focus (Inputs / Saved list)"),
        Line::from(" Delete     Delete saved connection"),
        Line::from(" Ctrl+W      Close current tab"),
        Line::from(" [ / ]       Switch between tabs"),
        Line::from(" Tab        Switch focus (Tables list / Data panel)"),
        Line::from(" j / k      Navigate lists (Tables, Databases, Schemas, Saved)"),
        Line::from(" Enter      Select / Open / Load connection"),
        Line::from(""),
        Line::from(Span::styled(" Data Panel ", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))),
        Line::from(" j / k      Scroll rows up/down"),
        Line::from(" h / l      Scroll columns left/right"),
        Line::from(" Enter / e  Inline cell edit (Confirm with Enter)"),
        Line::from(" :          Custom SQL query"),
        Line::from(" /          Search / Filter rows"),
        Line::from(" i / u / d  Insert / Update / Delete row"),
        Line::from(" v          Export current view to CSV"),
        Line::from(""),
        Line::from(Span::styled(" Form / Dialogs ", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))),
        Line::from(" Tab        Next field"),
        Line::from(" Enter      Confirm / Save"),
        Line::from(" y / n      Confirm Delete (Yes/No)"),
    ];

    let help_para = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(help_para, area);
}
