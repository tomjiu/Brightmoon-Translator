use docx_rs::{Docx, Paragraph, Run, RunProperty};
use serde::{Deserialize, Serialize};
use std::fs::File;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxParagraph {
    pub index: usize,
    pub text: String,
    pub style: String,
    pub is_heading: bool,
    pub heading_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxDocument {
    pub title: String,
    pub paragraphs: Vec<DocxParagraph>,
    pub total_paragraphs: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedParagraph {
    pub index: usize,
    pub original_text: String,
    pub translated_text: String,
    pub style: String,
    pub is_heading: bool,
    pub heading_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedDocx {
    pub title: String,
    pub paragraphs: Vec<TranslatedParagraph>,
    pub total_paragraphs: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxTranslationResult {
    pub input_path: String,
    pub output_path: String,
    pub paragraphs_translated: usize,
    pub words_translated: usize,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Extract text from DOCX file
pub fn extract_text_from_docx(file_path: &str) -> Result<DocxDocument, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open DOCX file: {}", e))?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut std::io::BufReader::new(file), &mut buf)
        .map_err(|e| format!("Failed to read DOCX file: {}", e))?;

    let docx = docx_rs::read_docx(&buf).map_err(|e| format!("Failed to parse DOCX file: {}", e))?;

    let mut paragraphs: Vec<DocxParagraph> = Vec::new();
    let mut title = String::from("Untitled");
    let mut total_words = 0;

    for (index, child) in docx.document.children.iter().enumerate() {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            let text = extract_paragraph_text(para);
            if text.trim().is_empty() {
                continue;
            }

            // Count words (approximate for CJK and Latin text)
            total_words += count_words(&text);

            // Detect style
            let (style, is_heading, heading_level) = detect_paragraph_style(para);

            // Try to get title from first heading or first paragraph
            if title == "Untitled" && (is_heading || index == 0) {
                title = text.clone();
            }

            paragraphs.push(DocxParagraph {
                index: paragraphs.len(),
                text,
                style,
                is_heading,
                heading_level,
            });
        }
    }

    let total_paragraphs = paragraphs.len();

    Ok(DocxDocument {
        title,
        paragraphs,
        total_paragraphs,
        total_words,
    })
}

/// Extract text from a paragraph, preserving run structure
fn extract_paragraph_text(para: &Paragraph) -> String {
    let mut text = String::new();

    for child in &para.children {
        if let docx_rs::ParagraphChild::Run(run) = child {
            for run_child in &run.children {
                if let docx_rs::RunChild::Text(t) = run_child {
                    text.push_str(&t.text);
                }
            }
        }
    }

    text
}

/// Detect paragraph style
fn detect_paragraph_style(para: &Paragraph) -> (String, bool, u8) {
    let style = para
        .property
        .style
        .as_ref()
        .map(|s| s.val.clone())
        .unwrap_or_default();

    let is_heading = style.starts_with("Heading") || style.starts_with("heading");
    let heading_level = if is_heading {
        style
            .chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .unwrap_or(1) as u8
    } else {
        0
    };

    (style, is_heading, heading_level)
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
    (0x4E00..=0x9FFF).contains(&code)       // CJK Unified Ideographs
        || (0x3400..=0x4DBF).contains(&code) // CJK Unified Ideographs Extension A
        || (0xF900..=0xFAFF).contains(&code) // CJK Compatibility Ideographs
        || (0x3000..=0x303F).contains(&code) // CJK Symbols and Punctuation
        || (0xFF00..=0xFFEF).contains(&code) // Halfwidth and Fullwidth Forms
}

/// Write translated content back to DOCX file
pub fn write_translated_docx(
    input_path: &str,
    output_path: &str,
    translations: &[(usize, String)],
) -> Result<DocxTranslationResult, String> {
    let file = File::open(input_path).map_err(|e| format!("Failed to open DOCX file: {}", e))?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut std::io::BufReader::new(file), &mut buf)
        .map_err(|e| format!("Failed to read DOCX file: {}", e))?;

    let docx = docx_rs::read_docx(&buf).map_err(|e| format!("Failed to parse DOCX file: {}", e))?;

    let mut new_doc = Docx::new();
    let mut para_index = 0;
    let mut paragraphs_translated = 0;
    let mut words_translated = 0;
    let mut skipped_non_para = 0usize;

    // Create a lookup map for translations
    let translation_map: std::collections::HashMap<usize, &String> = translations
        .iter()
        .map(|(idx, text)| (*idx, text))
        .collect();

    for child in docx.document.children {
        match child {
            docx_rs::DocumentChild::Paragraph(para) => {
                let text = extract_paragraph_text(&para);

                // Skip empty paragraphs
                if text.trim().is_empty() {
                    new_doc = new_doc.add_paragraph(*para);
                    continue;
                }

                // Check if we have a translation for this paragraph
                if let Some(translated) = translation_map.get(&para_index) {
                    // Create new paragraph with translated text but preserve first-run rPr
                    let new_para = create_translated_paragraph(&para, translated);
                    new_doc = new_doc.add_paragraph(new_para);

                    paragraphs_translated += 1;
                    words_translated += count_words(&text);
                } else {
                    // Keep original paragraph
                    new_doc = new_doc.add_paragraph(*para);
                }

                para_index += 1;
            }
            docx_rs::DocumentChild::Table(table) => {
                // Best-effort: re-attach tables so they are not silently dropped.
                // Cell text is not rewritten in this pass (known fidelity limit).
                new_doc = new_doc.add_table(*table);
            }
            _ => {
                skipped_non_para += 1;
            }
        }
    }

    // Write output file
    let output_file =
        File::create(output_path).map_err(|e| format!("Failed to create output file: {}", e))?;

    new_doc
        .build()
        .pack(std::io::BufWriter::new(output_file))
        .map_err(|e| format!("Failed to write DOCX file: {}", e))?;

    let warning = if skipped_non_para > 0 {
        Some(format!(
            "Preserved tables; skipped {skipped_non_para} non-paragraph document children (styles may be incomplete)"
        ))
    } else {
        None
    };

    Ok(DocxTranslationResult {
        input_path: input_path.to_string(),
        output_path: output_path.to_string(),
        paragraphs_translated,
        words_translated,
        success: true,
        error_message: warning,
    })
}

/// Create a new paragraph with translated text, preserving original formatting.
///
/// P0#7 fix: when the original paragraph has MULTIPLE runs with different
/// formatting (e.g. "Hello **bold** world"), the translated text is split back
/// across the same run count, each run keeping its own run_property — instead
/// of collapsing everything into the first run's format. Split is proportional
/// to each original run's text length, so the formatting skeleton (bold /
/// italic / color per segment) survives the translation.
fn create_translated_paragraph(original: &Paragraph, translated_text: &str) -> Paragraph {
    let mut new_para = Paragraph::new();

    // Copy paragraph properties
    new_para.property = original.property.clone();

    let original_runs: Vec<(RunProperty, String)> = original
        .children
        .iter()
        .filter_map(|child| {
            if let docx_rs::ParagraphChild::Run(run) = child {
                let text: String = run
                    .children
                    .iter()
                    .filter_map(|rc| match rc {
                        docx_rs::RunChild::Text(t) => Some(t.text.clone()),
                        _ => None,
                    })
                    .collect();
                if text.is_empty() {
                    None
                } else {
                    Some((run.run_property.clone(), text))
                }
            } else {
                None
            }
        })
        .collect();

    if original_runs.is_empty() {
        // No textual runs — simple run with translated text.
        new_para = new_para.add_run(Run::new().add_text(translated_text));
        return new_para;
    }

    if original_runs.len() == 1 {
        // Single run — keep its formatting for the whole translation.
        let (prop, _) = &original_runs[0];
        let mut new_run = Run::new().add_text(translated_text);
        new_run.run_property = prop.clone();
        new_para = new_para.add_run(new_run);
        return new_para;
    }

    // Multi-run: split translated text proportionally to original run lengths.
    let total_len: usize = original_runs.iter().map(|(_, t)| t.chars().count()).sum();
    let translated_chars = translated_text.chars().count();
    let mut consumed = 0usize;
    for (i, (prop, text)) in original_runs.iter().enumerate() {
        let run_chars = text.chars().count();
        let is_last = i + 1 == original_runs.len();
        let slice: String = if is_last {
            translated_text.chars().skip(consumed).collect()
        } else {
            let ratio = if total_len > 0 { run_chars as f64 / total_len as f64 } else { 0.0 };
            let take = ((translated_chars as f64) * ratio).round() as usize;
            let end = (consumed + take).min(translated_chars);
            translated_text.chars().skip(consumed).take(end - consumed).collect()
        };
        consumed += slice.chars().count();
        let mut new_run = Run::new().add_text(slice);
        new_run.run_property = prop.clone();
        new_para = new_para.add_run(new_run);
    }

    new_para
}

/// Translate DOCX file
pub async fn translate_docx_file(
    input_path: &str,
    output_path: &str,
    _from_lang: &str,
    _to_lang: &str,
    translate_fn: impl for<'a> Fn(
        &'a [(usize, &'a str)],
    ) -> futures::future::BoxFuture<'a, Vec<(usize, String)>>,
) -> Result<DocxTranslationResult, String> {
    // Extract text
    let doc = extract_text_from_docx(input_path)?;

    if doc.paragraphs.is_empty() {
        return Ok(DocxTranslationResult {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            paragraphs_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No translatable content found".to_string()),
        });
    }

    // Prepare paragraphs for translation
    let paragraphs_to_translate: Vec<(usize, &str)> = doc
        .paragraphs
        .iter()
        .filter(|p| !p.text.trim().is_empty())
        .map(|p| (p.index, p.text.trim()))
        .collect();

    // Translate in batches
    let batch_results = translate_fn(&paragraphs_to_translate).await;

    // Write translated DOCX
    write_translated_docx(input_path, output_path, &batch_results)
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
    fn test_detect_paragraph_style() {
        let para = Paragraph::new();
        let (style, is_heading, level) = detect_paragraph_style(&para);
        assert!(style.is_empty());
        assert!(!is_heading);
        assert_eq!(level, 0);
    }
}
