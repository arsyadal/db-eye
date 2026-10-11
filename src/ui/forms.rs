use super::components::{centered_rect, scroll_input_view};
use crate::app::App;
use crate::app::CrudMode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

pub fn draw_crud_form(f: &mut Frame, app: &App) {
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

        let editable = !col.is_binary() && (!is_pk || form.mode == CrudMode::Insert);
        let border_style = if is_active && editable {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let name_label = if col.is_required() {
            format!("{}*", col.name)
        } else {
            col.name.clone()
        };
        let default_suffix = col
            .default_value
            .as_deref()
            .map(|d| format!(" [default: {}]", d))
            .unwrap_or_default();
        let fk_label = col
            .fk_table
            .as_deref()
            .map(|t| {
                format!(
                    " {} ({}) → {} | ←/→:pick  {}{} ",
                    name_label,
                    col.data_type,
                    t,
                    col.input_hint(),
                    default_suffix
                )
            })
            .unwrap_or_else(|| {
                format!(
                    " {} ({}) | {}{} ",
                    name_label,
                    col.data_type,
                    col.input_hint(),
                    default_suffix
                )
            });
        let field_rect = Rect {
            x: field_area.x,
            y,
            width: field_area.width,
            height: row_height,
        };

        let max_w = field_rect.width.saturating_sub(2) as usize;
        let (visible_val, cursor_x) = scroll_input_view(&display_val, display_val.len(), max_w);

        f.render_widget(
            Paragraph::new(visible_val.as_str()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(fk_label)
                    .border_style(border_style),
            ),
            field_rect,
        );
        if is_active && editable && field_rect.width > 2 {
            f.set_cursor_position((field_rect.x + 1 + cursor_x, field_rect.y + 1));
        }
        y += row_height;
    }

    // FK hint (valid values for active FK field)
    if hint_height > 0 {
        let mut spans = vec![Span::raw("  ")];
        for (i, val) in active_hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  |  "));
            }
            let style = if i == form.fk_hint_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            spans.push(Span::styled(val.as_str(), style));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Valid Values — ←/→: pick ")
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
        "  Tab or ↑↓: navigate fields    Enter: save    F2: multiline edit    Esc: cancel    empty/\\null = NULL    * = required"
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

pub fn draw_confirm_delete(f: &mut Frame, app: &App) {
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
        "\n  {}\n\n  SQL: {}\n\n  Press Y to confirm, N to cancel",
        confirm.description, confirm.sql
    );
    f.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚠ Confirm Delete ")
                .border_style(Style::default().add_modifier(Modifier::BOLD)),
        ),
        chunks[0],
    );
    let y_style = Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED);
    let n_style = Style::default().fg(Color::DarkGray);
    let spans = vec![
        Span::raw("  "),
        Span::styled(" [Y] Yes, delete ", y_style),
        Span::raw("    "),
        Span::styled(" [N] Cancel ", n_style),
    ];
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL)),
        chunks[1],
    );
}

pub fn draw_multiline_editor(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 60, f.area());
    f.render_widget(Clear, area);

    let chars: Vec<char> = app.multiline_buffer.chars().collect();
    let mut row = 0u16;
    let mut col = 0u16;
    for &c in chars.iter().take(app.multiline_cursor) {
        if c == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    let para = Paragraph::new(app.multiline_buffer.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Multiline Edit — F2: save  Esc: cancel "),
    );
    f.render_widget(para, area);
    f.set_cursor_position((area.x + 1 + col, area.y + 1 + row));
}

pub fn draw_export_popup(f: &mut Frame, app: &App) {
    let form = match &app.export_form {
        Some(f) => f,
        None => return,
    };

    let area = centered_rect(60, 15, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Export Data — Left/Right: change value  Up/Down: move  Esc: cancel ");
    f.render_widget(block.clone(), area);

    let inner_area = block.inner(area);

    let constraints = if form.format == crate::app::ExportFormat::Csv {
        vec![
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(2),
            Constraint::Length(0),
            Constraint::Length(0),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner_area);

    // Format Selection (row 0)
    let format_label = Span::styled(
        "Format:        ",
        Style::default().add_modifier(Modifier::BOLD),
    );
    let format_csv = Span::styled(
        " [CSV] ",
        if form.format == crate::app::ExportFormat::Csv {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    );
    let format_json = Span::styled(
        " [JSON] ",
        if form.format == crate::app::ExportFormat::Json {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    );
    let format_sql = Span::styled(
        " [SQL] ",
        if form.format == crate::app::ExportFormat::Sql {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    );
    let format_line = Line::from(vec![
        format_label,
        Span::raw(" "),
        format_csv,
        Span::raw("    "),
        format_json,
        Span::raw("    "),
        format_sql,
    ]);
    f.render_widget(Paragraph::new(format_line), chunks[0]);

    if form.format == crate::app::ExportFormat::Csv {
        // Delimiter Selection (row 1)
        let delim_label = Span::styled(
            "CSV Delimiter: ",
            Style::default().add_modifier(Modifier::BOLD),
        );
        let delim_comma = Span::styled(
            " [Comma] ",
            if form.csv_delimiter == crate::app::CsvDelimiter::Comma {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let delim_semi = Span::styled(
            " [Semicolon] ",
            if form.csv_delimiter == crate::app::CsvDelimiter::Semicolon {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let delim_tab = Span::styled(
            " [Tab] ",
            if form.csv_delimiter == crate::app::CsvDelimiter::Tab {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let delim_line = Line::from(vec![
            delim_label,
            Span::raw(" "),
            delim_comma,
            Span::raw("  "),
            delim_semi,
            Span::raw("  "),
            delim_tab,
        ]);
        f.render_widget(Paragraph::new(delim_line), chunks[1]);

        // Headers (row 2)
        let headers_label = Span::styled(
            "CSV Headers:   ",
            Style::default().add_modifier(Modifier::BOLD),
        );
        let headers_yes = Span::styled(
            " [Yes] ",
            if form.csv_headers {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let headers_no = Span::styled(
            " [No] ",
            if !form.csv_headers {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );

        let headers_line = Line::from(vec![
            headers_label,
            Span::raw(" "),
            headers_yes,
            Span::raw("    "),
            headers_no,
        ]);
        f.render_widget(Paragraph::new(headers_line), chunks[2]);
    }

    // Filename input (row 3)
    let filename_label = Span::styled(
        "File Name:     ",
        Style::default().add_modifier(Modifier::BOLD),
    );
    let filename_style = if form.active_field == 3 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let filename_line = Line::from(vec![
        filename_label,
        Span::styled(&form.filename, filename_style),
        if form.active_field == 3 {
            Span::styled("_", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        },
    ]);
    f.render_widget(Paragraph::new(filename_line), chunks[3]);

    // Buttons (row 5)
    let btn_area = chunks[5];
    let btn_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(12),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Min(0),
        ])
        .split(btn_area);

    let export_btn_style = if form.active_field == 4 {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(" [ Export ] ").style(export_btn_style),
        btn_layout[1],
    );

    let cancel_btn_style = if form.active_field == 5 {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(" [ Cancel ] ").style(cancel_btn_style),
        btn_layout[3],
    );
}

pub fn draw_import_popup(f: &mut Frame, app: &App) {
    let form = match &app.import_form {
        Some(f) => f,
        None => return,
    };

    if !form.preview_mode {
        let area = centered_rect(60, 15, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Import CSV — Left/Right: change value  Up/Down: move  Esc: cancel ");
        f.render_widget(block.clone(), area);

        let inner_area = block.inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(inner_area);

        // File Path (row 0)
        let path_label = Span::styled(
            "File Path:     ",
            Style::default().add_modifier(Modifier::BOLD),
        );
        let path_style = if form.active_field == 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let path_line = Line::from(vec![
            path_label,
            Span::styled(&form.filepath, path_style),
            if form.active_field == 0 {
                Span::styled("_", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]);
        f.render_widget(Paragraph::new(path_line), chunks[0]);

        // Delimiter (row 1)
        let delim_label = Span::styled(
            "CSV Delimiter: ",
            Style::default().add_modifier(Modifier::BOLD),
        );
        let delim_comma = Span::styled(
            " [Comma] ",
            if form.delimiter == crate::app::ImportDelimiter::Comma {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let delim_semi = Span::styled(
            " [Semicolon] ",
            if form.delimiter == crate::app::ImportDelimiter::Semicolon {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let delim_tab = Span::styled(
            " [Tab] ",
            if form.delimiter == crate::app::ImportDelimiter::Tab {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let delim_line = Line::from(vec![
            delim_label,
            Span::raw(" "),
            delim_comma,
            Span::raw("  "),
            delim_semi,
            Span::raw("  "),
            delim_tab,
        ]);
        f.render_widget(Paragraph::new(delim_line), chunks[1]);

        // Headers (row 2)
        let headers_label = Span::styled(
            "CSV Headers:   ",
            Style::default().add_modifier(Modifier::BOLD),
        );
        let headers_yes = Span::styled(
            " [Yes] ",
            if form.has_headers {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );
        let headers_no = Span::styled(
            " [No] ",
            if !form.has_headers {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );

        let headers_line = Line::from(vec![
            headers_label,
            Span::raw(" "),
            headers_yes,
            Span::raw("    "),
            headers_no,
        ]);
        f.render_widget(Paragraph::new(headers_line), chunks[2]);

        // Error message (row 3)
        if let Some(err) = &form.error_message {
            f.render_widget(
                Paragraph::new(format!("Error: {}", err)).style(Style::default().fg(Color::Red)),
                chunks[3],
            );
        }

        // Buttons (row 4)
        let btn_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(12),
                Constraint::Length(4),
                Constraint::Length(12),
                Constraint::Min(0),
            ])
            .split(chunks[4]);

        let preview_btn_style = if form.active_field == 3 {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
        };
        f.render_widget(
            Paragraph::new(" [ Preview ] ").style(preview_btn_style),
            btn_layout[1],
        );

        let cancel_btn_style = if form.active_field == 4 {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(" [ Cancel ] ").style(cancel_btn_style),
            btn_layout[3],
        );
    } else {
        let area = centered_rect(85, 22, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" CSV Import Preview & Column Mapping — Enter: confirm  Esc: back ");
        f.render_widget(block.clone(), area);

        let inner_area = block.inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(4),
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner_area);

        let info_line = Line::from(vec![
            Span::raw("File: "),
            Span::styled(&form.filepath, Style::default().fg(Color::Cyan)),
            Span::raw("  |  Parsed: "),
            Span::styled(
                format!("{} rows", form.parsed_rows.len()),
                Style::default().fg(Color::Green),
            ),
        ]);
        f.render_widget(Paragraph::new(info_line), chunks[0]);

        let mut mapping_spans = vec![Span::styled(
            "Mapping:  ",
            Style::default().add_modifier(Modifier::BOLD),
        )];
        for (csv_col, db_col_opt) in &form.mapped_columns {
            mapping_spans.push(Span::raw(format!("{} -> ", csv_col)));
            if let Some(db_col) = db_col_opt {
                mapping_spans.push(Span::styled(
                    format!("{}  ", db_col),
                    Style::default().fg(Color::Green),
                ));
            } else {
                mapping_spans.push(Span::styled(
                    "[Skip]  ",
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        f.render_widget(
            Paragraph::new(Line::from(mapping_spans)).wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[1],
        );

        let preview_rows = form.parsed_rows.iter().take(5);
        let headers: Row =
            Row::new(form.csv_columns.iter().map(|c| {
                Cell::from(c.as_str()).style(Style::default().add_modifier(Modifier::BOLD))
            }))
            .style(Style::default().bg(Color::DarkGray));

        let rows: Vec<Row> = preview_rows
            .map(|r| Row::new(r.iter().map(|c| Cell::from(c.as_str()))))
            .collect();

        let num_cols = form.csv_columns.len().max(1);
        let col_percent = 100 / num_cols as u16;
        let widths: Vec<Constraint> = (0..num_cols)
            .map(|_| Constraint::Percentage(col_percent))
            .collect();

        let table = Table::new(rows, widths).header(headers).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Sample Data Preview (First 5 rows) "),
        );
        f.render_widget(table, chunks[2]);

        let btn_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Length(20),
                Constraint::Length(4),
                Constraint::Length(12),
                Constraint::Min(0),
            ])
            .split(chunks[4]);

        let import_btn_style = if form.active_field == 3 {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
        };
        f.render_widget(
            Paragraph::new(" [ Confirm Import ] ").style(import_btn_style),
            btn_layout[1],
        );

        let cancel_btn_style = if form.active_field == 4 {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(" [ Cancel ] ").style(cancel_btn_style),
            btn_layout[3],
        );
    }
}
