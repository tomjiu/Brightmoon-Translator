use crate::excel::{
    self, ExcelDocument, ExcelTranslationResult, TranslatedCell, TranslatedExcel, TranslatedSheet,
};
use crate::security;
use crate::AppState;
use std::collections::HashMap;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn open_excel(file_path: String) -> Result<ExcelDocument, String> {
    security::validate_file_path(&file_path)?;
    excel::extract_text_from_excel(&file_path)
}

#[tauri::command]
pub async fn translate_excel(
    state: State<'_, AppState>,
    window: Window,
    input_path: String,
    output_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<ExcelTranslationResult, String> {
    security::validate_file_path(&input_path)?;
    security::validate_output_path(&output_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    // Extract text
    let doc = excel::extract_text_from_excel(&input_path)?;

    if doc.sheets.is_empty() {
        return Ok(ExcelTranslationResult {
            input_path,
            output_path,
            cells_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No sheets found in Excel file".to_string()),
        });
    }

    // Emit progress event
    let _ = window.emit(
        "excel-progress",
        serde_json::json!({
            "stage": "extracting",
            "totalSheets": doc.total_sheets,
            "totalCells": doc.total_cells,
            "totalWords": doc.total_words,
        }),
    );

    // Collect all cells to translate (skip formulas)
    let mut cells_to_translate: Vec<(usize, String, String, u32, u32)> = Vec::new();
    let mut cell_index = 0;

    for sheet in &doc.sheets {
        for cell in &sheet.cells {
            if !cell.is_formula && !cell.text.trim().is_empty() {
                cells_to_translate.push((
                    cell_index,
                    sheet.name.clone(),
                    cell.text.clone(),
                    cell.row,
                    cell.col,
                ));
                cell_index += 1;
            }
        }
    }

    if cells_to_translate.is_empty() {
        return Ok(ExcelTranslationResult {
            input_path,
            output_path,
            cells_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some(
                "No translatable content found (only formulas or empty cells)".to_string(),
            ),
        });
    }

    // Emit translation start
    let _ = window.emit(
        "excel-progress",
        serde_json::json!({
            "stage": "translating",
            "cellsToTranslate": cells_to_translate.len(),
        }),
    );

    // Prepare paragraphs for batch translation
    let text_pairs: Vec<(usize, &str)> = cells_to_translate
        .iter()
        .map(|(idx, _, text, _, _)| (*idx, text.as_str()))
        .collect();

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .translate_batch(&text_pairs, &from_lang, &to_lang, 2)
        .await;

    // Emit write progress
    let _ = window.emit(
        "excel-progress",
        serde_json::json!({
            "stage": "writing",
        }),
    );

    // Build translation map
    let mut translation_map: HashMap<(String, u32, u32), String> = HashMap::new();
    for result in batch_results {
        if let Some((_, sheet_name, _, row, col)) = cells_to_translate.get(result.index) {
            translation_map.insert((sheet_name.clone(), *row, *col), result.translated);
        }
    }

    // Write translated Excel
    let result = excel::write_translated_excel(&input_path, &output_path, &translation_map)?;

    // Emit completion
    let _ = window.emit(
        "excel-progress",
        serde_json::json!({
            "stage": "completed",
            "cellsTranslated": result.cells_translated,
            "wordsTranslated": result.words_translated,
        }),
    );

    Ok(result)
}

#[tauri::command]
pub async fn translate_excel_preview(
    state: State<'_, AppState>,
    input_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedExcel, String> {
    security::validate_file_path(&input_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    let doc = excel::extract_text_from_excel(&input_path)?;

    // Collect all cells to translate (skip formulas)
    let mut cells_to_translate: Vec<(usize, String, String, u32, u32)> = Vec::new();
    let mut cell_index = 0;

    for sheet in &doc.sheets {
        for cell in &sheet.cells {
            if !cell.is_formula && !cell.text.trim().is_empty() {
                cells_to_translate.push((
                    cell_index,
                    sheet.name.clone(),
                    cell.text.clone(),
                    cell.row,
                    cell.col,
                ));
                cell_index += 1;
            }
        }
    }

    // Prepare paragraphs for batch translation
    let text_pairs: Vec<(usize, &str)> = cells_to_translate
        .iter()
        .map(|(idx, _, text, _, _)| (*idx, text.as_str()))
        .collect();

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .translate_batch(&text_pairs, &from_lang, &to_lang, 2)
        .await;

    // Build translation map
    let mut translation_map: HashMap<(String, u32, u32), String> = HashMap::new();
    for result in batch_results {
        if let Some((_, sheet_name, _, row, col)) = cells_to_translate.get(result.index) {
            translation_map.insert((sheet_name.clone(), *row, *col), result.translated);
        }
    }

    // Build translated sheets
    let mut translated_sheets: Vec<TranslatedSheet> = Vec::new();

    for sheet in &doc.sheets {
        let mut translated_cells: Vec<TranslatedCell> = Vec::new();

        for cell in &sheet.cells {
            let key = (sheet.name.clone(), cell.row, cell.col);
            let translated_text = translation_map.get(&key).cloned().unwrap_or_default();

            translated_cells.push(TranslatedCell {
                row: cell.row,
                col: cell.col,
                original_text: cell.text.clone(),
                translated_text,
                is_formula: cell.is_formula,
            });
        }

        translated_sheets.push(TranslatedSheet {
            name: sheet.name.clone(),
            total_cells: sheet.total_cells,
            total_words: sheet.total_words,
            cells: translated_cells,
        });
    }

    Ok(TranslatedExcel {
        title: doc.title,
        total_sheets: doc.total_sheets,
        total_cells: doc.total_cells,
        total_words: doc.total_words,
        sheets: translated_sheets,
    })
}
