use super::components::{centered_rect, scroll_input_view};
use crate::app::App;
use crate::app::format_duration_ms;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

pub fn draw_query_popup(f: &mut Frame, app: &App) {
    let has_suggestions = !app.query_suggestions.is_empty();
    let height = if has_suggestions { 6 } else { 5 };
    let area = centered_rect(75, height, f.area());
    f.render_widget(Clear, area);

    if has_suggestions {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)])
            .split(area);

        let max_w = chunks[0].width.saturating_sub(4) as usize;
        let (visible, cursor_x) = scroll_input_view(&app.query_input, app.query_cursor, max_w);

        let title =
            " SQL Query — Enter: run  Tab: complete  Ctrl+E: explain  Ctrl+S: save  Esc: cancel ";
        f.render_widget(
            Paragraph::new(format!(": {}", visible))
                .block(Block::default().borders(Borders::ALL).title(title)),
            chunks[0],
        );
        f.set_cursor_position((chunks[0].x + 3 + cursor_x, chunks[0].y + 1));

        let mut spans = vec![Span::styled(
            " Suggestions (Tab): ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )];
        for (i, sug) in app.query_suggestions.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            let is_selected = i == app.query_suggestion_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" {sug} "), style));
        }

        f.render_widget(
            Paragraph::new(Line::from(spans)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            chunks[1],
        );
    } else {
        let max_w = area.width.saturating_sub(4) as usize;
        let (visible, cursor_x) = scroll_input_view(&app.query_input, app.query_cursor, max_w);

        f.render_widget(
            Paragraph::new(format!(": {}", visible)).block(Block::default().borders(Borders::ALL).title(
                " SQL Query — Enter: run  Ctrl+E: explain  Ctrl+S: save  Ctrl+N: load  Esc: cancel ",
            )),
            area,
        );
        f.set_cursor_position((area.x + 3 + cursor_x, area.y + 1));
    }
}

pub fn draw_query_history_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(85, 18, f.area());
    f.render_widget(Clear, area);

    if app.query_history.is_empty() {
        f.render_widget(
            Paragraph::new("\n  No query history yet. Run a query with ':' first.").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Query History — Esc: close "),
            ),
            area,
        );
        return;
    }

    let max_rows = area.height.saturating_sub(4) as usize;
    let start = if app.history_index >= max_rows {
        app.history_index + 1 - max_rows
    } else {
        0
    };

    let rows: Vec<Row> = app
        .query_history
        .iter()
        .enumerate()
        .skip(start)
        .take(max_rows)
        .map(|(i, entry)| {
            let style = if i == app.history_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if entry.success {
                Style::default()
            } else {
                Style::default().fg(Color::Red)
            };
            Row::new(vec![
                Cell::from(format!("#{}", i + 1)),
                Cell::from(entry.status_label()),
                Cell::from(format_duration_ms(entry.duration_ms)),
                Cell::from(truncate_line(&entry.sql, 90)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["No", "Status", "Duration", "SQL"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Query History — j/k:nav  Enter:edit  r:run  Esc:close "),
    );

    f.render_widget(table, area);
}

fn truncate_line(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let truncated: String = value.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

pub fn draw_save_query_prompt(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 5, f.area());
    f.render_widget(Clear, area);

    let max_w = area.width.saturating_sub(8) as usize;
    let (visible, cursor_x) =
        scroll_input_view(&app.save_query_input, app.save_query_input.len(), max_w);

    f.render_widget(
        Paragraph::new(format!("Name: {}", visible)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Save SQL Query as Named Query — Enter: save  Esc: cancel "),
        ),
        area,
    );
    f.set_cursor_position((area.x + 7 + cursor_x, area.y + 1));
}

pub fn draw_named_queries_list(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 18, f.area());
    f.render_widget(Clear, area);

    if app.named_queries.is_empty() {
        f.render_widget(
            Paragraph::new("\n  No named queries saved yet. Press Ctrl+S in Query screen to save.")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Named Queries — Esc: close "),
                ),
            area,
        );
        return;
    }

    let items: Vec<Row> = app
        .named_queries
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let style = if i == app.named_query_index {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(q.name.as_str()),
                Cell::from(q.sql.as_str()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        items,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    )
    .header(
        Row::new(vec![
            Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from("SQL").style(Style::default().add_modifier(Modifier::BOLD)),
        ])
        .style(Style::default().bg(Color::DarkGray)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Saved Named Queries — Enter: load  Delete: delete  Esc/q: close "),
    );

    f.render_widget(table, area);
}

pub fn draw_search_bar(f: &mut Frame, app: &App) {
    let area = f.area();
    let bar_area = Rect {
        x: 22,
        y: area.height.saturating_sub(6),
        width: area.width.saturating_sub(22),
        height: 3,
    };
    f.render_widget(Clear, bar_area);

    let max_w = bar_area.width.saturating_sub(4) as usize;
    let (visible, cursor_x) = scroll_input_view(&app.search_input, app.search_input.len(), max_w);

    f.render_widget(
        Paragraph::new(format!("/{}", visible)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search — Enter/Esc: close "),
        ),
        bar_area,
    );
    f.set_cursor_position((bar_area.x + 2 + cursor_x, bar_area.y + 1));
}

pub fn draw_jump_bar(f: &mut Frame, app: &App) {
    let area = f.area();
    let bar_area = Rect {
        x: 22,
        y: area.height.saturating_sub(6),
        width: area.width.saturating_sub(22),
        height: 3,
    };
    f.render_widget(Clear, bar_area);

    let max_w = bar_area.width.saturating_sub(8) as usize;
    let (visible, cursor_x) = scroll_input_view(&app.jump_input, app.jump_input.len(), max_w);

    f.render_widget(
        Paragraph::new(format!("Row #{}", visible)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Jump to row — Enter/Esc: close "),
        ),
        bar_area,
    );
    f.set_cursor_position((bar_area.x + 6 + cursor_x, bar_area.y + 1));
}

pub fn draw_help_popup(f: &mut Frame) {
    let area = centered_rect(80, 22, f.area());
    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(vec![Span::styled(
            " DB-EYE Keybindings ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )]),
        Line::from(""),
        Line::from(Span::styled(
            " General ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(" Ctrl+C      Quit application"),
        Line::from(" :           SQL Query Editor"),
        Line::from(" Ctrl+H      Query History"),
        Line::from(" ?           Show/hide this help screen"),
        Line::from(" Esc         Go back / Cancel"),
        Line::from(""),
        Line::from(Span::styled(
            " Connection & Navigation ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(" Ctrl+T      New connection (Connect screen)"),
        Line::from(" Ctrl+S      Save connection (Connect screen)"),
        Line::from(" Tab        Switch focus (Inputs / Saved list)"),
        Line::from(" Delete     Delete saved connection"),
        Line::from(" Ctrl+W      Close current tab"),
        Line::from(" Ctrl+R      Reconnect current tab (recovers a dropped connection)"),
        Line::from(" [ / ]       Switch between tabs"),
        Line::from(" Tab        Switch focus (Tables list / Data panel)"),
        Line::from(" j / k      Navigate lists (Tables, Databases, Schemas, Saved)"),
        Line::from(" Enter      Select / Open / Load connection"),
        Line::from(" n          Create new database (on Databases list)"),
        Line::from(""),
        Line::from(Span::styled(
            " Data Panel ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(" j / k      Scroll rows up/down"),
        Line::from(" h / l      Scroll columns left/right"),
        Line::from(" Enter / e  Inline cell edit (Confirm with Enter)"),
        Line::from(" /          Search / Filter rows"),
        Line::from(" i / u / d  Insert / Update / Delete row"),
        Line::from(" v          Export current view to CSV"),
        Line::from(" I          Import CSV into active table"),
        Line::from(" s          Show table stats (indexes, size)"),
        Line::from(" o          Sort by column under cursor (asc/desc/off)"),
        Line::from(" PgUp/PgDn  Previous / next page"),
        Line::from(" g          Jump to row number"),
        Line::from(""),
        Line::from(Span::styled(
            " Form / Dialogs ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from(" Tab        Next field"),
        Line::from(" ← / →      Pick a FK value (on FK fields)"),
        Line::from(" F2         Open multiline text editor for the active field"),
        Line::from(" Enter      Confirm / Save"),
        Line::from(" y / n      Confirm Delete (Yes/No)"),
    ];

    let help_para = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(help_para, area);
}

pub fn draw_table_stats_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 50, f.area());
    f.render_widget(Clear, area);

    let table_name = app
        .current_tab()
        .and_then(|t| t.tables.get(t.table_index))
        .map(|t| t.name.as_str())
        .unwrap_or("");

    let mut lines = vec![
        Line::from(vec![Span::styled(
            format!(" {} ", table_name),
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )]),
        Line::from(""),
    ];

    if let Some(stats) = app.table_stats.as_ref() {
        let size_line = match &stats.size_label {
            Some(size) => format!(" Size: {size}"),
            None => " Size: n/a".to_string(),
        };
        lines.push(Line::from(size_line));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" Indexes ({}) ", stats.indexes.len()),
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        if stats.indexes.is_empty() {
            lines.push(Line::from(" (none)"));
        } else {
            for idx in &stats.indexes {
                lines.push(Line::from(format!(" {idx}")));
            }
        }
    }

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Table Stats — Esc/s: close "),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(para, area);
}

pub fn draw_new_database_prompt(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 5, f.area());
    f.render_widget(Clear, area);

    let max_w = area.width.saturating_sub(6) as usize;
    let (visible, cursor_x) = scroll_input_view(&app.new_db_input, app.new_db_input.len(), max_w);

    f.render_widget(
        Paragraph::new(format!("> {}", visible)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Create New Database — Enter: create  Esc: cancel "),
        ),
        area,
    );
    f.set_cursor_position((area.x + 3 + cursor_x, area.y + 1));
}
