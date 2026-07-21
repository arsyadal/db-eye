use crate::app::{App, Screen, Tab};
use crate::db::NULL_DISPLAY;
use crossterm::event::KeyCode;
use std::fs;

#[derive(Clone, Copy, PartialEq)]
pub enum ImportDelimiter {
    Comma,
    Semicolon,
    Tab,
}

impl ImportDelimiter {
    pub fn as_char(&self) -> char {
        match self {
            Self::Comma => ',',
            Self::Semicolon => ';',
            Self::Tab => '\t',
        }
    }
}

pub struct ImportForm {
    pub filepath: String,
    pub delimiter: ImportDelimiter,
    pub has_headers: bool,
    pub active_field: usize, // 0 = filepath, 1 = delimiter, 2 = headers, 3 = import, 4 = cancel
    pub csv_columns: Vec<String>,
    pub db_columns: Vec<String>,
    pub mapped_columns: Vec<(String, Option<String>)>,
    pub parsed_rows: Vec<Vec<String>>,
    pub error_message: Option<String>,
    pub preview_mode: bool,
}

impl ImportForm {
    pub fn new(db_columns: Vec<String>) -> Self {
        Self {
            filepath: String::new(),
            delimiter: ImportDelimiter::Comma,
            has_headers: true,
            active_field: 0,
            csv_columns: vec![],
            db_columns,
            mapped_columns: vec![],
            parsed_rows: vec![],
            error_message: None,
            preview_mode: false,
        }
    }

    pub fn parse_csv(&mut self) -> Result<(), String> {
        let content = fs::read_to_string(&self.filepath)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        let delimiter_char = self.delimiter.as_char();
        let mut lines = content.lines();
        
        let first_line = match lines.next() {
            Some(l) => l,
            None => return Err("CSV file is empty".to_string()),
        };
        
        let raw_headers = parse_csv_line(first_line, delimiter_char);
        
        let mut parsed_rows = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            parsed_rows.push(parse_csv_line(line, delimiter_char));
        }

        if self.has_headers {
            self.csv_columns = raw_headers;
        } else {
            self.csv_columns = (1..=raw_headers.len())
                .map(|i| format!("Column {}", i))
                .collect();
            // If no headers, the first row is actually data
            parsed_rows.insert(0, raw_headers);
        }

        self.parsed_rows = parsed_rows;
        
        // Auto-match columns
        self.mapped_columns = self.csv_columns.iter().map(|csv_col| {
            let normalized_csv = csv_col.to_lowercase().replace(' ', "_");
            let matched = self.db_columns.iter().find(|db_col| {
                let normalized_db = db_col.to_lowercase();
                normalized_db == normalized_csv
            });
            (csv_col.clone(), matched.cloned())
        }).collect();

        Ok(())
    }
}

fn parse_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == delimiter && !in_quotes {
            fields.push(current.clone());
            current.clear();
        } else {
            current.push(c);
        }
    }
    fields.push(current);
    fields
}

impl App {
    pub(super) async fn handle_import_form(&mut self, key: KeyCode) {
        let mut form = match self.import_form.take() {
            Some(f) => f,
            None => return,
        };

        if !form.preview_mode {
            // Screen 1: Filepath entry
            match key {
                KeyCode::Esc => {
                    self.screen = Screen::Main;
                    self.status = "Import cancelled".to_string();
                    return;
                }
                KeyCode::Up => {
                    if form.active_field > 0 {
                        form.active_field -= 1;
                    } else {
                        form.active_field = 4;
                    }
                }
                KeyCode::Down => {
                    form.active_field += 1;
                    if form.active_field > 4 {
                        form.active_field = 0;
                    }
                }
                KeyCode::Left | KeyCode::Right => {
                    match form.active_field {
                        1 => { // Delimiter
                            form.delimiter = match form.delimiter {
                                ImportDelimiter::Comma => if key == KeyCode::Left { ImportDelimiter::Tab } else { ImportDelimiter::Semicolon },
                                ImportDelimiter::Semicolon => if key == KeyCode::Left { ImportDelimiter::Comma } else { ImportDelimiter::Tab },
                                ImportDelimiter::Tab => if key == KeyCode::Left { ImportDelimiter::Semicolon } else { ImportDelimiter::Comma },
                            };
                        }
                        2 => { // Headers
                            form.has_headers = !form.has_headers;
                        }
                        _ => {}
                    }
                }
                KeyCode::Char(c) if form.active_field == 0 => {
                    form.filepath.push(c);
                    form.error_message = None;
                }
                KeyCode::Backspace if form.active_field == 0 => {
                    form.filepath.pop();
                    form.error_message = None;
                }
                KeyCode::Enter => {
                    match form.active_field {
                        0 | 3 => { // Preview / Continue
                            match form.parse_csv() {
                                Ok(()) => {
                                    form.preview_mode = true;
                                    form.active_field = 3; // Focus on Import Button
                                }
                                Err(e) => {
                                    form.error_message = Some(e);
                                }
                            }
                        }
                        4 => { // Cancel
                            self.screen = Screen::Main;
                            self.status = "Import cancelled".to_string();
                            return;
                        }
                        _ => {
                            form.active_field += 1;
                        }
                    }
                }
                _ => {}
            }
        } else {
            // Screen 2: Column mapping and data preview
            match key {
                KeyCode::Esc => {
                    // Back to filepath input
                    form.preview_mode = false;
                    form.active_field = 0;
                }
                KeyCode::Up | KeyCode::Down => {
                    // Switch between Import and Cancel buttons
                    if form.active_field == 3 {
                        form.active_field = 4;
                    } else {
                        form.active_field = 3;
                    }
                }
                KeyCode::Left | KeyCode::Right => {
                    // Also switch between Import and Cancel buttons
                    if form.active_field == 3 {
                        form.active_field = 4;
                    } else {
                        form.active_field = 3;
                    }
                }
                KeyCode::Enter => {
                    if form.active_field == 4 {
                        // Cancel
                        self.screen = Screen::Main;
                        self.status = "Import cancelled".to_string();
                        return;
                    } else if form.active_field == 3 {
                        // Perform the actual import
                        if let Some(tab) = self.current_tab_mut() {
                            match execute_import(tab, &form).await {
                                Ok(count) => {
                                    self.status = format!("Imported {} rows successfully!", count);
                                    // Refresh table data
                                    self.load_table_data().await;
                                    self.screen = Screen::Main;
                                    return;
                                }
                                Err(e) => {
                                    form.error_message = Some(format!("Import failed: {}", e));
                                    form.preview_mode = false;
                                    form.active_field = 0;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.import_form = Some(form);
    }
}

async fn execute_import(tab: &mut Tab, form: &ImportForm) -> Result<usize, Box<dyn std::error::Error>> {
    let table_name = tab.path.split('/').next_back().unwrap_or("table");
    let quoted_table = if tab.db.uses_numbered_placeholders() {
        format!("\"{}\"", table_name.replace('"', "\"\""))
    } else {
        format!("`{}`", table_name.replace('`', "``"))
    };
    
    let mut target_db_cols = Vec::new();
    let mut csv_indices = Vec::new();
    for (i, (_csv, db_opt)) in form.mapped_columns.iter().enumerate() {
        if let Some(db_col) = db_opt {
            target_db_cols.push(db_col.clone());
            csv_indices.push(i);
        }
    }
    
    if target_db_cols.is_empty() {
        return Err("No columns mapped to database table".into());
    }

    let columns_part = target_db_cols.iter()
        .map(|c| if tab.db.uses_numbered_placeholders() { format!("\"{}\"", c.replace('"', "\"\"")) } else { format!("`{}`", c.replace('`', "``")) })
        .collect::<Vec<_>>()
        .join(", ");
        
    let placeholders_part = (0..target_db_cols.len())
        .map(|i| {
            if tab.db.uses_numbered_placeholders() {
                format!("${}", i + 1)
            } else {
                "?".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
        
    let sql = format!("INSERT INTO {} ({}) VALUES ({})", quoted_table, columns_part, placeholders_part);
    
    let mut count = 0;
    for row in &form.parsed_rows {
        let mut vals = Vec::new();
        for &idx in &csv_indices {
            let val = row.get(idx).cloned().unwrap_or_default();
            let val_opt = if val.is_empty() || val == NULL_DISPLAY {
                None
            } else {
                Some(val)
            };
            vals.push(val_opt);
        }
        tab.db.execute_write_with_values(&sql, &vals).await?;
        count += 1;
    }
    
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_line() {
        assert_eq!(
            parse_csv_line("1,hello,world", ','),
            vec!["1".to_string(), "hello".to_string(), "world".to_string()]
        );
        assert_eq!(
            parse_csv_line("1,\"hello, world\",3", ','),
            vec!["1".to_string(), "hello, world".to_string(), "3".to_string()]
        );
        assert_eq!(
            parse_csv_line("1,\"hello \"\"world\"\"\",3", ','),
            vec!["1".to_string(), "hello \"world\"".to_string(), "3".to_string()]
        );
    }

    #[test]
    fn test_column_matching() {
        let db_cols = vec!["id".to_string(), "first_name".to_string(), "last_name".to_string()];
        let mut form = ImportForm::new(db_cols);
        form.csv_columns = vec!["ID".to_string(), "First Name".to_string(), "Unused".to_string()];
        
        // Auto-match columns
        form.mapped_columns = form.csv_columns.iter().map(|csv_col| {
            let normalized_csv = csv_col.to_lowercase().replace(' ', "_");
            let matched = form.db_columns.iter().find(|db_col| {
                let normalized_db = db_col.to_lowercase();
                normalized_db == normalized_csv
            });
            (csv_col.clone(), matched.cloned())
        }).collect();

        assert_eq!(form.mapped_columns[0], ("ID".to_string(), Some("id".to_string())));
        assert_eq!(form.mapped_columns[1], ("First Name".to_string(), Some("first_name".to_string())));
        assert_eq!(form.mapped_columns[2], ("Unused".to_string(), None));
    }
}
