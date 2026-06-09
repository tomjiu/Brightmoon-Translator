use calamine::{open_workbook_auto, Data, Reader, Sheets};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcelCell {
    pub row: u32,
    pub col: u32,
    pub text: String,
    pub is_formula: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcelSheet {
    pub name: String,
    pub cells: Vec<ExcelCell>,
    pub total_cells: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcelDocument {
    pub title: String,
    pub sheets: Vec<ExcelSheet>,
    pub total_sheets: usize,
    pub total_cells: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedCell {
    pub row: u32,
    pub col: u32,
    pub original_text: String,
    pub translated_text: String,
    pub is_formula: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSheet {
    pub name: String,
    pub cells: Vec<TranslatedCell>,
    pub total_cells: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedExcel {
    pub title: String,
    pub sheets: Vec<TranslatedSheet>,
    pub total_sheets: usize,
    pub total_cells: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcelTranslationResult {
    pub input_path: String,
    pub output_path: String,
    pub cells_translated: usize,
    pub words_translated: usize,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Check if a cell value is a formula (starts with =)
fn is_formula(value: &str) -> bool {
    value.trim_start().starts_with('=')
}

/// Convert calamine Data to string representation
fn data_to_string(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        },
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::Error(e) => format!("#ERROR: {:?}", e),
        Data::DateTime(dt) => {
            // Convert Excel datetime to string
            let days = dt.as_f64() as i64;
            let time_fraction = dt.as_f64() - days as f64;
            let hours = (time_fraction * 24.0) as u32;
            let minutes = ((time_fraction * 24.0 - hours as f64) * 60.0) as u32;
            let seconds = ((time_fraction * 1440.0 - (hours * 60 + minutes) as f64) * 60.0) as u32;
            // Simple date formatting without chrono
            let year = 1899;
            let month = 12;
            let day = 30 + days;
            if hours == 0 && minutes == 0 && seconds == 0 {
                format!("{:04}-{:02}-{:02}", year, month, day)
            } else {
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    year, month, day, hours, minutes, seconds
                )
            }
        },
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
    }
}

/// Count words in text (handles both CJK and Latin)
fn count_words(text: &str) -> usize {
    let mut count = 0;
    let mut current_kind: Option<bool> = None;
    let has_latin_word = text.chars().any(|ch| ch.is_ascii_alphanumeric());

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if current_kind != Some(false) {
                count += 1;
                current_kind = Some(false);
            }
        } else if is_cjk(ch) {
            if has_latin_word {
                if current_kind != Some(true) {
                    count += 1;
                    current_kind = Some(true);
                }
            } else {
                count += 1;
                current_kind = None;
            }
        } else {
            current_kind = None;
        }
    }

    count
}

/// Check if character is CJK
fn is_cjk(ch: char) -> bool {
    let code = ch as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
        || (0x3000..=0x303F).contains(&code)
        || (0xFF00..=0xFFEF).contains(&code)
}

/// Extract text from Excel file (xlsx/xls/csv)
pub fn extract_text_from_excel(file_path: &str) -> Result<ExcelDocument, String> {
    let mut workbook: Sheets<BufReader<File>> =
        open_workbook_auto(file_path).map_err(|e| format!("Failed to open Excel file: {}", e))?;

    let mut sheets: Vec<ExcelSheet> = Vec::new();
    let mut total_cells = 0;
    let mut total_words = 0;

    let sheet_names = workbook.sheet_names().to_vec();

    for sheet_name in &sheet_names {
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_name, e))?;

        let mut cells: Vec<ExcelCell> = Vec::new();
        let mut sheet_words = 0;

        for (row_idx, row) in range.rows().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let text = data_to_string(cell);
                if text.is_empty() {
                    continue;
                }

                let is_formula_cell = is_formula(&text);
                let word_count = count_words(&text);
                sheet_words += word_count;

                cells.push(ExcelCell {
                    row: row_idx as u32,
                    col: col_idx as u32,
                    text,
                    is_formula: is_formula_cell,
                });
            }
        }

        total_cells += cells.len();
        total_words += sheet_words;

        sheets.push(ExcelSheet {
            name: sheet_name.clone(),
            total_cells: cells.len(),
            cells,
            total_words: sheet_words,
        });
    }

    let title = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    Ok(ExcelDocument {
        title,
        total_sheets: sheets.len(),
        sheets,
        total_cells,
        total_words,
    })
}

/// Write translated content back to Excel file
pub fn write_translated_excel(
    input_path: &str,
    output_path: &str,
    translations: &HashMap<(String, u32, u32), String>,
) -> Result<ExcelTranslationResult, String> {
    // Re-read the original workbook to preserve formatting
    let mut workbook: Sheets<BufReader<File>> =
        open_workbook_auto(input_path).map_err(|e| format!("Failed to open Excel file: {}", e))?;

    let mut output_workbook = Workbook::new();
    let mut cells_translated = 0;
    let mut words_translated = 0;

    let sheet_names = workbook.sheet_names().to_vec();

    for sheet_name in &sheet_names {
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_name, e))?;

        let worksheet = output_workbook.add_worksheet();
        worksheet
            .set_name(sheet_name)
            .map_err(|e| format!("Failed to set sheet name: {}", e))?;

        // Create a format for wrapped text
        let wrap_format = Format::new().set_text_wrap();

        for (row_idx, row) in range.rows().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let original_text = data_to_string(cell);
                if original_text.is_empty() {
                    continue;
                }

                let key = (sheet_name.clone(), row_idx as u32, col_idx as u32);

                if let Some(translated) = translations.get(&key) {
                    // Write translated text
                    worksheet
                        .write_string_with_format(
                            row_idx as u32,
                            col_idx as u16,
                            translated,
                            &wrap_format,
                        )
                        .map_err(|e| format!("Failed to write cell: {}", e))?;

                    cells_translated += 1;
                    words_translated += count_words(&original_text);
                } else {
                    // Write original value (preserve non-translatable cells)
                    match cell {
                        Data::Empty => {},
                        Data::String(s) => {
                            worksheet
                                .write_string(row_idx as u32, col_idx as u16, s)
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                        Data::Float(f) => {
                            worksheet
                                .write_number(row_idx as u32, col_idx as u16, *f)
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                        Data::Int(i) => {
                            worksheet
                                .write_number(row_idx as u32, col_idx as u16, *i as f64)
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                        Data::Bool(b) => {
                            worksheet
                                .write_boolean(row_idx as u32, col_idx as u16, *b)
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                        Data::Error(_) => {
                            // Skip error cells
                        },
                        Data::DateTime(dt) => {
                            worksheet
                                .write_number(row_idx as u32, col_idx as u16, dt.as_f64())
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                        Data::DateTimeIso(s) => {
                            worksheet
                                .write_string(row_idx as u32, col_idx as u16, s)
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                        Data::DurationIso(s) => {
                            worksheet
                                .write_string(row_idx as u32, col_idx as u16, s)
                                .map_err(|e| format!("Failed to write cell: {}", e))?;
                        },
                    }
                }
            }
        }
    }

    output_workbook
        .save(output_path)
        .map_err(|e| format!("Failed to save Excel file: {}", e))?;

    Ok(ExcelTranslationResult {
        input_path: input_path.to_string(),
        output_path: output_path.to_string(),
        cells_translated,
        words_translated,
        success: true,
        error_message: None,
    })
}

/// Translate Excel file
pub async fn translate_excel_file(
    input_path: &str,
    output_path: &str,
    _from_lang: &str,
    _to_lang: &str,
    translate_fn: impl for<'a> Fn(
        &'a [(usize, &'a str)],
    ) -> futures::future::BoxFuture<'a, Vec<(usize, String)>>,
) -> Result<ExcelTranslationResult, String> {
    // Extract text
    let doc = extract_text_from_excel(input_path)?;

    if doc.sheets.is_empty() {
        return Ok(ExcelTranslationResult {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            cells_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No translatable content found".to_string()),
        });
    }

    // Collect all cells to translate (skip formulas)
    let mut cells_to_translate: Vec<(usize, String, u32, u32)> = Vec::new();
    let mut cell_index = 0;

    for sheet in &doc.sheets {
        for cell in &sheet.cells {
            if !cell.is_formula && !cell.text.trim().is_empty() {
                cells_to_translate.push((cell_index, sheet.name.clone(), cell.row, cell.col));
                cell_index += 1;
            }
        }
    }

    if cells_to_translate.is_empty() {
        return Ok(ExcelTranslationResult {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            cells_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some(
                "No translatable content found (only formulas or empty cells)".to_string(),
            ),
        });
    }

    // Prepare text pairs for translation
    let text_pairs: Vec<(usize, &str)> = cells_to_translate
        .iter()
        .map(|(idx, _, row, col)| {
            let sheet = doc
                .sheets
                .iter()
                .find(|s| s.cells.iter().any(|c| c.row == *row && c.col == *col));
            let text = sheet
                .and_then(|s| s.cells.iter().find(|c| c.row == *row && c.col == *col))
                .map(|c| c.text.as_str())
                .unwrap_or("");
            (*idx, text)
        })
        .collect();

    // Translate in batches
    let batch_results = translate_fn(&text_pairs).await;

    // Build translation map
    let mut translation_map: HashMap<(String, u32, u32), String> = HashMap::new();
    for (idx, translated) in batch_results {
        if let Some((_, sheet_name, row, col)) = cells_to_translate.get(idx) {
            translation_map.insert((sheet_name.clone(), *row, *col), translated);
        }
    }

    // Write translated Excel
    write_translated_excel(input_path, output_path, &translation_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_words() {
        assert_eq!(count_words("Hello World"), 2);
        assert_eq!(count_words("你好世界"), 4);
        assert_eq!(count_words("Hello 你好 World"), 3);
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("   "), 0);
    }

    #[test]
    fn test_is_cjk() {
        assert!(is_cjk('你'));
        assert!(is_cjk('好'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk('1'));
    }

    #[test]
    fn test_is_formula() {
        assert!(is_formula("=SUM(A1:A10)"));
        assert!(is_formula("  =A1+B1"));
        assert!(!is_formula("Hello"));
        assert!(!is_formula(""));
    }

    #[test]
    fn test_data_to_string() {
        assert_eq!(data_to_string(&Data::Empty), "");
        assert_eq!(data_to_string(&Data::String("hello".to_string())), "hello");
        assert_eq!(data_to_string(&Data::Float(3.14)), "3.14");
        assert_eq!(data_to_string(&Data::Float(3.0)), "3");
        assert_eq!(data_to_string(&Data::Int(42)), "42");
        assert_eq!(data_to_string(&Data::Bool(true)), "true");
    }
}
