use quick_xml::events::{BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read as IoRead;
use std::io::Write as IoWrite;
use zip::read::ZipArchive;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxSlide {
    pub index: usize,
    pub name: String,
    pub text_blocks: Vec<PptxTextBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxTextBlock {
    pub id: String,
    pub text: String,
    pub slide_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxDocument {
    pub title: String,
    pub slides: Vec<PptxSlide>,
    pub total_slides: usize,
    pub total_text_blocks: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedTextBlock {
    pub id: String,
    pub original_text: String,
    pub translated_text: String,
    pub slide_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSlide {
    pub index: usize,
    pub name: String,
    pub text_blocks: Vec<TranslatedTextBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedPptx {
    pub title: String,
    pub slides: Vec<TranslatedSlide>,
    pub total_slides: usize,
    pub total_text_blocks: usize,
    pub total_words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxTranslationResult {
    pub input_path: String,
    pub output_path: String,
    pub slides_translated: usize,
    pub text_blocks_translated: usize,
    pub words_translated: usize,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Extract text from PPTX file
pub fn extract_text_from_pptx(file_path: &str) -> Result<PptxDocument, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open PPTX file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read PPTX archive: {}", e))?;

    let mut slides: Vec<PptxSlide> = Vec::new();
    let mut total_words = 0;
    let mut title = String::from("Untitled");

    // Collect slide file names and sort them
    let mut slide_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = entry.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_files.push(name);
        }
    }
    slide_files.sort();

    // Re-open archive to read slide contents
    let file = File::open(file_path).map_err(|e| format!("Failed to open PPTX file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read PPTX archive: {}", e))?;

    for (slide_idx, slide_name) in slide_files.iter().enumerate() {
        let mut slide_file = archive
            .by_name(slide_name)
            .map_err(|e| format!("Failed to read slide {}: {}", slide_name, e))?;

        let mut xml_content = String::new();
        slide_file
            .read_to_string(&mut xml_content)
            .map_err(|e| format!("Failed to read slide XML: {}", e))?;

        let text_blocks = extract_text_from_xml(&xml_content, slide_idx);

        for block in &text_blocks {
            total_words += count_words(&block.text);
        }

        // Try to get title from first slide's first text block
        if title == "Untitled" && slide_idx == 0 {
            if let Some(first_block) = text_blocks.first() {
                if !first_block.text.is_empty() {
                    title = first_block.text.clone();
                }
            }
        }

        slides.push(PptxSlide {
            index: slide_idx,
            name: slide_name.clone(),
            text_blocks,
        });
    }

    let total_slides = slides.len();
    let total_text_blocks = slides.iter().map(|s| s.text_blocks.len()).sum();

    Ok(PptxDocument {
        title,
        slides,
        total_slides,
        total_text_blocks,
        total_words,
    })
}

/// Extract text blocks from slide XML
fn extract_text_from_xml(xml: &str, slide_index: usize) -> Vec<PptxTextBlock> {
    let mut reader = Reader::from_str(xml);
    let mut text_blocks = Vec::new();
    let mut in_text_body = false;
    let mut in_paragraph = false;
    let mut in_text_elem = false;
    let mut current_text = String::new();
    let mut block_id = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = e.name().as_ref().to_vec();
                match local_tag_name(&tag) {
                    b"txBody" => {
                        in_text_body = true;
                    },
                    b"p" if in_text_body => {
                        in_paragraph = true;
                        current_text.clear();
                    },
                    b"t" if in_paragraph => {
                        in_text_elem = true;
                    },
                    _ => {},
                }
            },
            Ok(Event::End(ref e)) => {
                let tag = e.name().as_ref().to_vec();
                match local_tag_name(&tag) {
                    b"txBody" => {
                        in_text_body = false;
                    },
                    b"p" if in_text_body => {
                        in_paragraph = false;
                        let trimmed = current_text.trim().to_string();
                        if !trimmed.is_empty() {
                            text_blocks.push(PptxTextBlock {
                                id: format!("{}_{}", slide_index, block_id),
                                text: trimmed,
                                slide_index,
                            });
                            block_id += 1;
                        }
                    },
                    b"t" if in_paragraph => {
                        in_text_elem = false;
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(ref e)) if in_text_elem => {
                if let Ok(text) = e.unescape() {
                    current_text.push_str(&text);
                }
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
    }

    text_blocks
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

fn local_tag_name(tag: &[u8]) -> &[u8] {
    tag.rsplit(|b| *b == b':').next().unwrap_or(tag)
}

/// Write translated content back to PPTX file
pub fn write_translated_pptx(
    input_path: &str,
    output_path: &str,
    translations: &[(String, String)], // (block_id, translated_text)
) -> Result<PptxTranslationResult, String> {
    let input_file =
        File::open(input_path).map_err(|e| format!("Failed to open PPTX file: {}", e))?;
    let mut archive =
        ZipArchive::new(input_file).map_err(|e| format!("Failed to read PPTX archive: {}", e))?;

    let output_file =
        File::create(output_path).map_err(|e| format!("Failed to create output file: {}", e))?;
    let mut zip_writer = ZipWriter::new(output_file);

    // Create translation map
    let translation_map: HashMap<String, &String> = translations
        .iter()
        .map(|(id, text)| (id.clone(), text))
        .collect();

    let mut slides_translated = 0;
    let mut text_blocks_translated = 0;
    let mut words_translated = 0;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = entry.name().to_string();

        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|e| format!("Failed to read entry content: {}", e))?;

        // Process slide XML files
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            let xml_content =
                String::from_utf8(content).map_err(|e| format!("Invalid UTF-8 in slide: {}", e))?;

            // Extract slide index from filename
            let slide_idx = extract_slide_index(&name).unwrap_or(0);

            let (translated_xml, blocks_count, words_count) =
                translate_slide_xml(&xml_content, slide_idx, &translation_map)?;

            if blocks_count > 0 {
                slides_translated += 1;
                text_blocks_translated += blocks_count;
                words_translated += words_count;
            }

            zip_writer
                .start_file(&name, SimpleFileOptions::default())
                .map_err(|e| format!("Failed to write to archive: {}", e))?;
            zip_writer
                .write_all(translated_xml.as_bytes())
                .map_err(|e| format!("Failed to write slide content: {}", e))?;
        } else {
            // Copy other files as-is
            zip_writer
                .start_file(&name, SimpleFileOptions::default())
                .map_err(|e| format!("Failed to write to archive: {}", e))?;
            zip_writer
                .write_all(&content)
                .map_err(|e| format!("Failed to write entry content: {}", e))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize PPTX file: {}", e))?;

    Ok(PptxTranslationResult {
        input_path: input_path.to_string(),
        output_path: output_path.to_string(),
        slides_translated,
        text_blocks_translated,
        words_translated,
        success: true,
        error_message: None,
    })
}

/// Extract slide index from filename like "ppt/slides/slide1.xml"
fn extract_slide_index(name: &str) -> Option<usize> {
    let stem = name.strip_prefix("ppt/slides/slide")?;
    let num_str = stem.strip_suffix(".xml")?;
    num_str.parse::<usize>().ok().map(|n| n.saturating_sub(1))
}

/// Translate text in slide XML while preserving structure and run properties (`a:rPr`).
///
/// For a translated paragraph: put full translation into the **first** `<a:t>`,
/// clear subsequent `<a:t>` in the same paragraph. Never rebuild runs (keeps rPr).
fn translate_slide_xml(
    xml: &str,
    slide_index: usize,
    translations: &HashMap<String, &String>,
) -> Result<(String, usize, usize), String> {
    let blocks = extract_text_from_xml(xml, slide_index);

    let mut paragraph_translations: HashMap<usize, &str> = HashMap::new();
    let mut blocks_translated = 0;
    let mut words_translated = 0;

    for block in &blocks {
        if let Some(translated) = translations.get(&block.id) {
            // block.id is like "s{slide}_{para}" — use extract order index from blocks
            let para_idx = block
                .id
                .rsplit('_')
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            paragraph_translations.insert(para_idx, translated.as_str());
            blocks_translated += 1;
            words_translated += count_words(&block.text);
        }
    }

    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut in_text_body = false;
    let mut in_paragraph = false;
    let mut in_text_elem = false;
    let mut para_index = 0usize;
    let mut t_index_in_para = 0usize;
    let mut para_translation: Option<&str> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = e.name().as_ref().to_vec();
                match local_tag_name(&tag) {
                    b"txBody" => {
                        in_text_body = true;
                        writer
                            .write_event(Event::Start(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                    b"p" if in_text_body => {
                        in_paragraph = true;
                        t_index_in_para = 0;
                        para_translation = paragraph_translations.get(&para_index).copied();
                        writer
                            .write_event(Event::Start(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                    b"t" if in_paragraph => {
                        in_text_elem = true;
                        writer
                            .write_event(Event::Start(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                    _ => {
                        writer
                            .write_event(Event::Start(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = e.name().as_ref().to_vec();
                match local_tag_name(&tag) {
                    b"txBody" => {
                        in_text_body = false;
                        writer
                            .write_event(Event::End(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                    b"p" if in_text_body => {
                        in_paragraph = false;
                        para_translation = None;
                        writer
                            .write_event(Event::End(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                        para_index += 1;
                    }
                    b"t" if in_paragraph => {
                        in_text_elem = false;
                        t_index_in_para += 1;
                        writer
                            .write_event(Event::End(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                    _ => {
                        writer
                            .write_event(Event::End(e.clone()))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_text_elem => {
                if let Some(translated) = para_translation {
                    if t_index_in_para == 0 {
                        writer
                            .write_event(Event::Text(BytesText::new(translated)))
                            .map_err(|err| format!("Write error: {}", err))?;
                    } else {
                        // Clear extra runs' text so layout/rPr stay, content not duplicated
                        writer
                            .write_event(Event::Text(BytesText::new("")))
                            .map_err(|err| format!("Write error: {}", err))?;
                    }
                } else {
                    writer
                        .write_event(Event::Text(e.clone()))
                        .map_err(|err| format!("Write error: {}", err))?;
                }
            }
            Ok(Event::Eof) => break,
            Ok(ref other) => {
                writer
                    .write_event(other.clone())
                    .map_err(|err| format!("Write error: {}", err))?;
            }
            Err(e) => return Err(format!("XML parse error: {}", e)),
        }
    }

    let output =
        String::from_utf8(writer.into_inner()).map_err(|e| format!("UTF-8 error: {}", e))?;
    Ok((output, blocks_translated, words_translated))
}

/// Translate PPTX file with translation function
pub async fn translate_pptx_file(
    input_path: &str,
    output_path: &str,
    translate_fn: impl for<'a> Fn(
        &'a [(usize, &'a str)],
    ) -> futures::future::BoxFuture<'a, Vec<(usize, String)>>,
) -> Result<PptxTranslationResult, String> {
    // Extract text
    let doc = extract_text_from_pptx(input_path)?;

    if doc.slides.is_empty() {
        return Ok(PptxTranslationResult {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            slides_translated: 0,
            text_blocks_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No translatable content found".to_string()),
        });
    }

    // Collect all text blocks for translation
    let mut all_blocks: Vec<(usize, &str)> = Vec::new();
    for slide in &doc.slides {
        for block in &slide.text_blocks {
            if !block.text.trim().is_empty() {
                all_blocks.push((all_blocks.len(), block.text.trim()));
            }
        }
    }

    if all_blocks.is_empty() {
        return Ok(PptxTranslationResult {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            slides_translated: 0,
            text_blocks_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No translatable content found".to_string()),
        });
    }

    // Translate
    let batch_results = translate_fn(&all_blocks).await;

    // Build translations for write function
    let translations: Vec<(String, String)> = batch_results
        .into_iter()
        .map(|r| {
            // Find the block ID for this index
            let mut block_idx = 0;
            for slide in &doc.slides {
                for block in &slide.text_blocks {
                    if !block.text.trim().is_empty() {
                        if block_idx == r.0 {
                            return (block.id.clone(), r.1);
                        }
                        block_idx += 1;
                    }
                }
            }
            (format!("block_{}", r.0), r.1)
        })
        .collect();

    write_translated_pptx(input_path, output_path, &translations)
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
    fn test_extract_slide_index() {
        assert_eq!(extract_slide_index("ppt/slides/slide1.xml"), Some(0));
        assert_eq!(extract_slide_index("ppt/slides/slide12.xml"), Some(11));
        assert_eq!(extract_slide_index("ppt/slides/slides.xml"), None);
        assert_eq!(extract_slide_index("other/file.xml"), None);
    }

    #[test]
    fn test_extract_text_from_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p>
            <a:r>
              <a:t>Hello World</a:t>
            </a:r>
          </a:p>
          <a:p>
            <a:r>
              <a:t>Second paragraph</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

        let blocks = extract_text_from_xml(xml, 0);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "Hello World");
        assert_eq!(blocks[1].text, "Second paragraph");
    }

    #[test]
    fn test_translate_slide_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p>
            <a:r>
              <a:t>Hello World</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

        let mut translations = HashMap::new();
        let translated = String::from("你好世界");
        translations.insert("0_0".to_string(), &translated);

        let (result, blocks_count, _) = translate_slide_xml(xml, 0, &translations).unwrap();
        assert_eq!(blocks_count, 1);
        assert!(result.contains("你好世界"));
        assert!(!result.contains("Hello World"));
    }
}
