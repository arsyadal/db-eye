use crate::app::{App, Focus};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

pub fn draw_data_panel(f: &mut Frame, app: &App, area: Rect) {
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
            .map(|c| {
                let label = if tab.sort_column.as_deref() == Some(c.as_str()) {
                    let arrow = if tab.sort_desc { "▼" } else { "▲" };
                    format!("{c} {arrow}")
                } else {
                    c.clone()
                };
                Cell::from(label).style(Style::default().add_modifier(Modifier::BOLD))
            })
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
                                style = style.add_modifier(Modifier::BOLD | Modifier::REVERSED);
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
    if let Some((edit_r, edit_c)) = tab.editing_cell
        && focused
        && edit_c >= col_offset
        && edit_c < col_offset + visible
    {
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

        let y = area.y + 3 + edit_r.saturating_sub(state.offset()) as u16;
        if y < area.y + area.height - 1
            && x_offset + (tab.edit_buffer.len() as u16) < area.x + area.width - 1
        {
            f.set_cursor_position((area.x + x_offset + tab.edit_buffer.len() as u16, y));
        }
    }
}

pub fn auto_col_widths(
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
        let w = (header_w.max(max_cell_w).clamp(4, 28) + 2) as u16;
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
