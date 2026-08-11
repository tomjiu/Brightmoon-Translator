use calamine::{open_workbook_auto, Data, Reader, Sheets};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read as IoRead, Write as IoWrite};
use zip::read::ZipArchive;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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
                format!("{f}")
            }
        },
        Data::Int(i) => format!("{i}"),
        Data::Bool(b) => format!("{b}"),
        Data::Error(e) => format!("#ERROR: {e:?}"),
        Data::DateTime(dt) => {
            // Convert Excel datetime to string
            let days = dt.as_f64() as i64;
            let time_fraction = dt.as_f64() - days as f64;
            let hours = (time_fraction * 24.0) as u32;
            let minutes = ((time_fraction * 24.0 - f64::from(hours)) * 60.0) as u32;
            let seconds = ((time_fraction * 1440.0 - f64::from(hours * 60 + minutes)) * 60.0) as u32;
            // Simple date formatting without chrono
            let year = 1899;
            let month = 12;
            let day = 30 + days;
            if hours == 0 && minutes == 0 && seconds == 0 {
                format!("{year:04}-{month:02}-{day:02}")
            } else {
                format!(
                    "{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02}"
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
        open_workbook_auto(file_path).map_err(|e| format!("Failed to open Excel file: {e}"))?;

    let mut sheets: Vec<ExcelSheet> = Vec::new();
    let mut total_cells = 0;
    let mut total_words = 0;

    let sheet_names = workbook.sheet_names().clone();

    for sheet_name in &sheet_names {
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("Failed to read sheet '{sheet_name}': {e}"))?;

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
    // P0#8 fix: instead of rebuilding the workbook with a write-only writer
    // (rust_xlsxwriter), rewrite the xlsx ZIP in place — copy every entry
    // verbatim and only patch the matching <c> cells in xl/worksheets/sheetN.xml.
    // This preserves merged cells, column widths, rich styles and charts.
    let input_file =
        File::open(input_path).map_err(|e| format!("Failed to open Excel file: {e}"))?;
    let mut archive =
        ZipArchive::new(input_file).map_err(|e| format!("Failed to read Excel archive: {e}"))?;

    let output_file =
        File::create(output_path).map_err(|e| format!("Failed to create output file: {e}"))?;
    let mut zip_writer = ZipWriter::new(output_file);

    // sheet name → worksheet file path (xl/worksheets/sheetN.xml)
    let sheet_map = xlsx_sheet_name_to_file(&mut archive)?;

    let mut cells_translated = 0;
    let mut words_translated = 0;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {e}"))?;
        let name = entry.name().to_string();
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|e| format!("Failed to read entry content: {e}"))?;

        // Patch worksheet XML files that contain translated cells.
        if let Some(sheet_name) = sheet_map.iter().find(|(_, file)| *file == &name).map(|(n, _)| n) {
            let xml = String::from_utf8(content.clone())
                .map_err(|e| format!("Invalid UTF-8 in worksheet: {e}"))?;
            let (patched, count, words) =
                patch_worksheet_cells(&xml, sheet_name, translations)?;
            if count > 0 {
                cells_translated += count;
                words_translated += words;
            }
            let out_bytes = patched.into_bytes();
            zip_writer
                .start_file(&name, SimpleFileOptions::default())
                .map_err(|e| format!("Failed to write to archive: {e}"))?;
            zip_writer
                .write_all(&out_bytes)
                .map_err(|e| format!("Failed to write worksheet content: {e}"))?;
        } else {
            // Copy all other entries verbatim (styles, charts, merges, etc).
            zip_writer
                .start_file(&name, SimpleFileOptions::default())
                .map_err(|e| format!("Failed to write to archive: {e}"))?;
            zip_writer
                .write_all(&content)
                .map_err(|e| format!("Failed to write entry content: {e}"))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize Excel file: {e}"))?;

    Ok(ExcelTranslationResult {
        input_path: input_path.to_string(),
        output_path: output_path.to_string(),
        cells_translated,
        words_translated,
        success: true,
        error_message: None,
    })
}

/// Build sheet name → worksheet file path map by reading workbook.xml
/// and the workbook relationships (xl/_rels/workbook.xml.rels).
fn xlsx_sheet_name_to_file(archive: &mut ZipArchive<File>) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    let workbook_xml = read_entry(archive, "xl/workbook.xml")?;
    // Collect sheet names in document order with their r:id.
    let mut names: Vec<(String, String)> = Vec::new();
    for cap in workbook_xml.split("<sheet ").skip(1) {
        // name="..." sheetId="N" r:id="rIdX"
        let name = extract_attr(cap, "name");
        let rid = extract_attr(cap, "r:id");
        if let (Some(name), Some(rid)) = (name, rid) {
            names.push((name, rid));
        }
    }
    // Read rels to map rId → target (worksheets/sheet1.xml).
    let rels_xml = read_entry(archive, "xl/_rels/workbook.xml.rels")?;
    let mut rid_to_target = HashMap::new();
    for rel in rels_xml.split("<Relationship ").skip(1) {
        let id = extract_attr(rel, "Id");
        let target = extract_attr(rel, "Target");
        if let (Some(id), Some(target)) = (id, target) {
            // Normalize target: "worksheets/sheet1.xml" or "/xl/worksheets/sheet1.xml"
            let t = target.trim_start_matches('/');
            let t = t.strip_prefix("xl/").unwrap_or(t);
            rid_to_target.insert(id, format!("xl/{}", t.trim_start_matches('/')));
        }
    }
    for (name, rid) in names {
        if let Some(target) = rid_to_target.get(&rid) {
            map.insert(name, target.clone());
        }
    }
    Ok(map)
}

fn read_entry(archive: &mut ZipArchive<File>, path: &str) -> Result<String, String> {
    let mut entry = archive
        .by_name(path)
        .map_err(|_| format!("Missing entry: {path}"))?;
    let mut content = Vec::new();
    entry
        .read_to_end(&mut content)
        .map_err(|e| format!("Failed to read {path}: {e}"))?;
    String::from_utf8(content).map_err(|e| format!("Invalid UTF-8 in {path}: {e}"))
}

fn extract_attr(xml: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = xml.find(&needle)? + needle.len();
    let end = xml[start..].find('"')? + start;
    Some(xml[start..end].to_string())
}

/// Convert an Excel cell ref like "A1" / "BC12" into 0-based (row, col).
fn cell_ref_to_rc(ref_: &str) -> Option<(u32, u32)> {
    let bytes = ref_.as_bytes();
    let mut col: u32 = 0;
    let mut row_start = 0;
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_alphabetic() {
            col = col * 26 + u32::from(*b) - if b.is_ascii_uppercase() { 'A' as u32 } else { 'a' as u32 } + 1;
        } else {
            row_start = i;
            break;
        }
    }
    if row_start == 0 || row_start >= bytes.len() {
        return None;
    }
    let row = bytes[row_start..]
        .iter()
        .filter(|b| b.is_ascii_digit())
        .collect::<Vec<_>>();
    let row: String = row.into_iter().map(|&b| b as char).collect();
    let row: u32 = row.parse().ok()?;
    Some((row - 1, col - 1))
}

/// Escape text for XML text node content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Rewrite a worksheet XML, replacing matching cell values with translated
/// text as inline strings. Returns (`patched_xml`, `cells_patched`, `words_patched`).
fn patch_worksheet_cells(
    xml: &str,
    sheet_name: &str,
    translations: &HashMap<(String, u32, u32), String>,
) -> Result<(String, usize, usize), String> {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    let mut patched = 0usize;
    let mut words = 0usize;
    // Find each <c ...>...</c> cell element.
    while let Some(c_start) = rest.find("<c ") {
        let tag_end = rest[c_start..].find('>').map(|p| c_start + p).ok_or_else(|| "malformed cell tag".to_string())?;
        let is_self_closing = rest[tag_end - 1..tag_end] == *"/";
        let open_tag = &rest[c_start..=tag_end];
        // Parse r="A1"
        let r_attr = extract_attr(open_tag, "r");
        // Emit everything before the cell.
        out.push_str(&rest[..c_start]);
        if is_self_closing {
            out.push_str(open_tag);
            rest = &rest[tag_end + 1..];
            continue;
        }
        // Find the matching close tag </c>.
        let close_marker = "</c>";
        let close_pos = rest[tag_end + 1..].find(close_marker).map(|p| tag_end + 1 + p).ok_or_else(|| "unterminated cell".to_string())?;
        let cell_body_end = close_pos + close_marker.len();
        let cell_full = &rest[c_start..cell_body_end];
        // Check translation for this cell.
        let mut replaced = false;
        if let Some(r) = r_attr {
            if let Some((row, col)) = cell_ref_to_rc(&r) {
                if let Some(translated) = translations.get(&(sheet_name.to_string(), row, col)) {
                    // Compute original word count from cell body text (best effort).
                    if let Some(orig) = cell_body_text(cell_full) {
                        words += count_words(&orig);
                    }
                    out.push_str(&format!(
                        "<c r=\"{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                        xml_escape(&r),
                        xml_escape(translated)
                    ));
                    replaced = true;
                    patched += 1;
                }
            }
        }
        if !replaced {
            out.push_str(cell_full);
        }
        rest = &rest[cell_body_end..];
    }
    out.push_str(rest);
    Ok((out, patched, words))
}

/// Best-effort extraction of the raw text inside a <c>…</c> cell for word counting.
fn cell_body_text(cell_xml: &str) -> Option<String> {
    let mut text = String::new();
    let mut start = 0;
    while let Some(rel) = cell_xml[start..].find("<t") {
        let t_start = start + rel;
        let tag_end_rel = cell_xml[t_start..].find('>')?;
        let tag_end = t_start + tag_end_rel;
        // Skip if self-closing (<t .../> with no text).
        if cell_xml[tag_end - 1..tag_end] == *"/" {
            start = tag_end + 1;
            continue;
        }
        let after = &cell_xml[tag_end + 1..];
        let t_end = after.find("</t>")?;
        text.push_str(&after[..t_end]);
        start = tag_end + 1 + t_end + 4;
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
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
                .map_or("", |c| c.text.as_str());
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
    #[allow(clippy::approx_constant)] // 3.14 here is a formatting fixture, not an approximation of PI
    fn test_data_to_string() {
        assert_eq!(data_to_string(&Data::Empty), "");
        assert_eq!(data_to_string(&Data::String("hello".to_string())), "hello");
        assert_eq!(data_to_string(&Data::Float(3.14)), "3.14");
        assert_eq!(data_to_string(&Data::Float(3.0)), "3");
        assert_eq!(data_to_string(&Data::Int(42)), "42");
        assert_eq!(data_to_string(&Data::Bool(true)), "true");
    }

    #[test]
    fn test_cell_ref_to_rc() {
        assert_eq!(cell_ref_to_rc("A1"), Some((0, 0)));
        assert_eq!(cell_ref_to_rc("B2"), Some((1, 1)));
        assert_eq!(cell_ref_to_rc("AA10"), Some((9, 26)));
        assert_eq!(cell_ref_to_rc("BC12"), Some((11, 54)));
        assert_eq!(cell_ref_to_rc("1"), None);
        assert_eq!(cell_ref_to_rc(""), None);
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
        assert_eq!(xml_escape("plain"), "plain");
        assert_eq!(xml_escape(""), "");
    }

    #[test]
    fn test_patch_worksheet_cells_inline() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>5</v></c></row></sheetData></worksheet>"#;
        let mut translations = HashMap::new();
        translations.insert(("Sheet1".to_string(), 0, 0), "你好".to_string());
        let (out, count, _words) =
            patch_worksheet_cells(xml, "Sheet1", &translations).unwrap();
        assert_eq!(count, 1);
        // A1 replaced with inline string; B1 untouched.
        assert!(out.contains(r#"<c r="A1" t="inlineStr"><is><t xml:space="preserve">你好</t></is></c>"#));
        assert!(out.contains(r#"<c r="B1"><v>5</v></c>"#));
    }

    #[test]
    fn test_patch_worksheet_cells_no_match() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c></row></sheetData></worksheet>"#;
        let mut translations = HashMap::new();
        translations.insert(("Sheet1".to_string(), 1, 0), "x".to_string());
        let (out, count, _words) =
            patch_worksheet_cells(xml, "Sheet1", &translations).unwrap();
        assert_eq!(count, 0);
        assert!(out.contains(r#"<c r="A1" t="s"><v>0</v></c>"#));
    }
}
