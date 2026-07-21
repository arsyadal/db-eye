use crate::app::{App, Screen, Tab};
use crate::db::NULL_DISPLAY;
use crossterm::event::KeyCode;
use std::fs;

#[derive(Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Csv,
    Json,
    Sql,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CsvDelimiter {
    Comma,
    Semicolon,
    Tab,
}

impl CsvDelimiter {
    pub fn as_char(&self) -> char {
        match self {
            Self::Comma => ',',
            Self::Semicolon => ';',
            Self::Tab => '\t',
        }
    }
}

pub struct ExportForm {
    pub format: ExportFormat,
    pub csv_delimiter: CsvDelimiter,
    pub csv_headers: bool,
    pub filename: String,
    pub active_field: usize, // 0 = format, 1 = delimiter, 2 = headers, 3 = filename, 4 = export button, 5 = cancel button
}

impl ExportForm {
    pub fn new(table_name: &str) -> Self {
        let safe_name = table_name.replace(['.', '/', '@', ':'], "_");
        let filename = format!("{}_export.csv", safe_name.trim_matches('_'));
        Self {
            format: ExportFormat::Csv,
            csv_delimiter: CsvDelimiter::Comma,
            csv_headers: true,
            filename,
            active_field: 0,
        }
    }

    pub fn update_filename_extension(&mut self) {
        let path = std::path::Path::new(&self.filename);
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("export");
        let ext = match self.format {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
            ExportFormat::Sql => "sql",
        };
        self.filename = format!("{}.{}", stem, ext);
    }
}

impl App {
    pub(super) fn handle_export_form(&mut self, key: KeyCode) {
        let mut form = match self.export_form.take() {
            Some(f) => f,
            None => return,
        };

        match key {
            KeyCode::Esc => {
                self.screen = Screen::Main;
                self.status = "Export cancelled".to_string();
                return;
            }
            KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                } else {
                    form.active_field = 5;
                }
                // Skip delimiter and headers for non-CSV formats
                if form.format != ExportFormat::Csv && (form.active_field == 1 || form.active_field == 2) && key == KeyCode::Up {
                    form.active_field = 0;
                }
            }
            KeyCode::Down => {
                form.active_field += 1;
                if form.format != ExportFormat::Csv && (form.active_field == 1 || form.active_field == 2) {
                    form.active_field = 3;
                }
                if form.active_field > 5 {
                    form.active_field = 0;
                }
            }
            KeyCode::Left | KeyCode::Right => {
                match form.active_field {
                    0 => { // Format
                        form.format = match form.format {
                            ExportFormat::Csv => if key == KeyCode::Left { ExportFormat::Sql } else { ExportFormat::Json },
                            ExportFormat::Json => if key == KeyCode::Left { ExportFormat::Csv } else { ExportFormat::Sql },
                            ExportFormat::Sql => if key == KeyCode::Left { ExportFormat::Json } else { ExportFormat::Csv },
                        };
                        form.update_filename_extension();
                    }
                    1 => { // Delimiter
                        form.csv_delimiter = match form.csv_delimiter {
                            CsvDelimiter::Comma => if key == KeyCode::Left { CsvDelimiter::Tab } else { CsvDelimiter::Semicolon },
                            CsvDelimiter::Semicolon => if key == KeyCode::Left { CsvDelimiter::Comma } else { CsvDelimiter::Tab },
                            CsvDelimiter::Tab => if key == KeyCode::Left { CsvDelimiter::Semicolon } else { CsvDelimiter::Comma },
                        };
                    }
                    2 => { // Headers
                        form.csv_headers = !form.csv_headers;
                    }
                    _ => {}
                }
            }
            KeyCode::Char(c) if form.active_field == 3 => {
                form.filename.push(c);
            }
            KeyCode::Backspace if form.active_field == 3 => {
                form.filename.pop();
            }
            KeyCode::Enter => {
                match form.active_field {
                    3 => { // If enter on filename, go to export button or just export directly
                        form.active_field = 4;
                    }
                    4 => { // Export
                        if let Some(tab) = self.current_tab() {
                            match tab.export_data(&form) {
                                Ok(()) => {
                                    self.status = format!("Exported successfully to {}", form.filename);
                                    self.screen = Screen::Main;
                                    return;
                                }
                                Err(e) => {
                                    self.status = format!("Export error: {}", e);
                                }
                            }
                        }
                    }
                    5 => { // Cancel
                        self.screen = Screen::Main;
                        self.status = "Export cancelled".to_string();
                        return;
                    }
                    _ => {
                        // For fields 0, 1, 2: enter moves down
                        form.active_field += 1;
                        if form.format != ExportFormat::Csv && (form.active_field == 1 || form.active_field == 2) {
                            form.active_field = 3;
                        }
                    }
                }
            }
            _ => {}
        }

        self.export_form = Some(form);
    }
}

impl Tab {
    pub fn export_data(&self, form: &ExportForm) -> Result<(), std::io::Error> {
        let result = match &self.result {
            Some(r) => r,
            None => return Err(std::io::Error::other("No data to export")),
        };

        match form.format {
            ExportFormat::Csv => {
                let delimiter = form.csv_delimiter.as_char();
                let delimiter_str = delimiter.to_string();
                let mut content = String::new();
                if form.csv_headers {
                    content += &result.columns.join(&delimiter_str);
                    content += "\n";
                }
                for row in &self.filtered_rows {
                    let line: Vec<String> = row
                        .iter()
                        .map(|cell| {
                            if cell == NULL_DISPLAY {
                                "".to_string()
                            } else if cell.contains(delimiter) || cell.contains('"') || cell.contains('\n') {
                                format!("\"{}\"", cell.replace('"', "\"\""))
                            } else {
                                cell.clone()
                            }
                        })
                        .collect();
                    content += &line.join(&delimiter_str);
                    content += "\n";
                }
                fs::write(&form.filename, content)?;
            }
            ExportFormat::Json => {
                let mut json_arr = Vec::new();
                for row in &self.filtered_rows {
                    let mut obj = serde_json::Map::new();
                    for (col, val) in result.columns.iter().zip(row.iter()) {
                        let json_val = if val == NULL_DISPLAY {
                            serde_json::Value::Null
                        } else {
                            serde_json::Value::String(val.clone())
                        };
                        obj.insert(col.clone(), json_val);
                    }
                    json_arr.push(serde_json::Value::Object(obj));
                }
                let content = serde_json::to_string_pretty(&json_arr)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                fs::write(&form.filename, content)?;
            }
            ExportFormat::Sql => {
                let mut content = String::new();
                let table_name = self.path.split('/').next_back().unwrap_or("table");
                let quoted_table = format!("\"{}\"", table_name.replace('"', "\"\""));
                let columns_list = result.columns.iter()
                    .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
                    .collect::<Vec<_>>()
                    .join(", ");
                
                for row in &self.filtered_rows {
                    let values_list = row.iter()
                        .map(|cell| {
                            if cell == NULL_DISPLAY {
                                "NULL".to_string()
                            } else {
                                format!("'{}'", cell.replace('\'', "''"))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    content += &format!(
                        "INSERT INTO {} ({}) VALUES ({});\n",
                        quoted_table, columns_list, values_list
                    );
                }
                fs::write(&form.filename, content)?;
            }
        }

        Ok(())
    }
}
