use crate::app::{App, DbTypeChoice};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Length((r.height.saturating_sub(height)) / 2),
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

pub fn scroll_input_view(text: &str, cursor_pos: usize, max_width: usize) -> (String, u16) {
    if max_width == 0 {
        return (String::new(), 0);
    }
    let char_count = text.chars().count();
    let cursor = cursor_pos.min(char_count);

    let scroll_offset = if cursor >= max_width {
        cursor - max_width + 1
    } else {
        0
    };

    let visible: String = text.chars().skip(scroll_offset).take(max_width).collect();
    let cursor_x = (cursor - scroll_offset) as u16;
    (visible, cursor_x)
}

pub fn render_input_box(
    f: &mut Frame,
    area: Rect,
    title: &str,
    text: &str,
    cursor_pos: usize,
    border_style: Style,
    is_active: bool,
) {
    let max_width = area.width.saturating_sub(2) as usize;
    let (visible_text, cursor_x) = scroll_input_view(text, cursor_pos, max_width);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .border_style(border_style);

    let paragraph = Paragraph::new(visible_text).block(block);
    f.render_widget(paragraph, area);

    if is_active && area.width > 2 && area.height > 2 {
        f.set_cursor_position((area.x + 1 + cursor_x, area.y + 1));
    }
}

pub fn db_type_selector<'a>(app: &'a App) -> Paragraph<'a> {
    let mut spans = vec![Span::raw(" ")];
    for dt in DbTypeChoice::all() {
        let is_selected = app.db_type == dt;
        let label = dt.label();
        if is_selected {
            spans.push(Span::styled(
                format!("[{label}]"),
                Style::default().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {label} "),
                Style::default().fg(Color::DarkGray),
            ));
        }
        spans.push(Span::raw("  "));
    }

    Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" DB Type  ←/→ "),
    )
}

pub fn format_status_to_spans<'a>(status: &'a str) -> Line<'a> {
    if status.starts_with("Error") || status.starts_with("error") {
        return Line::from(vec![
            Span::styled(
                " ✖ ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                status,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]);
    }

    let mut spans = Vec::new();
    let parts: Vec<&str> = status
        .split("  ")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    for p in parts {
        if p == "|" || p == "•" {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::styled(" • ", Style::default().fg(Color::DarkGray)));
        }

        if p == "READ-ONLY" {
            spans.push(Span::styled(
                " READ-ONLY ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if let Some(conn) = p.strip_prefix("Connected:") {
            spans.push(Span::styled(
                format!("● Connected:{}", conn),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if p == ": (SQL query)"
            || p == ":: (SQL query)"
            || p == ":::SQL query"
            || p == "::SQL query"
        {
            spans.push(Span::styled(
                " [:] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled("SQL query", Style::default()));
        } else if let Some(colon_idx) = p.find(':') {
            let key = p[..colon_idx].trim();
            let desc = p[colon_idx + 1..].trim();
            if key.is_empty() {
                spans.push(Span::styled(
                    " [:] ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" [{key}] "),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if !desc.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(desc, Style::default()));
            }
        } else {
            spans.push(Span::styled(p, Style::default().fg(Color::DarkGray)));
        }
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_input_view_fits_within_max_width() {
        let (vis, cur) = scroll_input_view("short", 5, 20);
        assert_eq!(vis, "short");
        assert_eq!(cur, 5);

        let long = "123456789012345678901234567890";
        let (vis_long, cur_long) = scroll_input_view(long, 30, 10);
        assert!(vis_long.chars().count() <= 10);
        assert!(cur_long < 10);
        let (empty_vis, empty_cur) = scroll_input_view("", 0, 10);
        assert_eq!(empty_vis, "");
        assert_eq!(empty_cur, 0);
    }
}
