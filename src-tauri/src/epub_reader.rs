use epub::doc::EpubDoc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpubChapter {
    pub chapter_number: usize,
    pub title: String,
    pub text: String,
    /// Original XHTML content (preserved for bilingual export)
    #[serde(skip)]
    pub html_content: String,
    /// Spine idref for matching during export
    #[serde(skip)]
    pub spine_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpubDocument {
    pub title: String,
    pub chapters: Vec<EpubChapter>,
    pub total_chapters: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedChapter {
    pub chapter_number: usize,
    pub title: String,
    pub original_text: String,
    pub translated_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedEpub {
    pub title: String,
    pub chapters: Vec<TranslatedChapter>,
    pub total_chapters: usize,
}

pub fn extract_text_from_epub(file_path: &str) -> Result<EpubDocument, String> {
    let mut epub =
        EpubDoc::new(file_path).map_err(|e| format!("Failed to open EPUB file: {}", e))?;

    let title = epub
        .mdata("title")
        .map(|m| m.value.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let mut chapters = Vec::new();
    let mut chapter_num = 0;

    // Get all spine items (reading order)
    let spine = epub.spine.clone();

    for spine_item in &spine {
        if let Some((content, _)) = epub.get_resource(&spine_item.idref) {
            let html = String::from_utf8_lossy(&content).to_string();

            // Extract plain text from HTML content
            let extracted = extract_text_from_html(&html);
            if !extracted.trim().is_empty() {
                chapter_num += 1;
                chapters.push(EpubChapter {
                    chapter_number: chapter_num,
                    title: format!("Chapter {}", chapter_num),
                    text: extracted,
                    html_content: html,
                    spine_id: spine_item.idref.clone(),
                });
            }
        }
    }

    // If no chapters found from spine, try to get all resources
    if chapters.is_empty() {
        epub.set_current_chapter(0);
        while epub.go_next() {
            if let Some((content, _)) = epub.get_current() {
                let html = String::from_utf8_lossy(&content).to_string();
                let extracted = extract_text_from_html(&html);
                if !extracted.trim().is_empty() {
                    chapter_num += 1;
                    chapters.push(EpubChapter {
                        chapter_number: chapter_num,
                        title: epub
                            .get_title()
                            .unwrap_or_else(|| format!("Chapter {}", chapter_num)),
                        text: extracted,
                        html_content: html,
                        spine_id: String::new(),
                    });
                }
            }
        }
    }

    let total_chapters = chapters.len();

    Ok(EpubDocument {
        title,
        chapters,
        total_chapters,
    })
}

fn extract_text_from_html(html: &str) -> String {
    // Simple HTML tag removal
    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    for line in html.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '<' {
                let mut tag = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '>' {
                        chars.next();
                        break;
                    }
                    tag.push(next);
                    chars.next();
                }

                let tag_lower = tag.to_lowercase();
                if tag_lower.starts_with("script") {
                    in_script = true;
                } else if tag_lower.starts_with("/script") {
                    in_script = false;
                } else if tag_lower.starts_with("style") {
                    in_style = true;
                } else if tag_lower.starts_with("/style") {
                    in_style = false;
                } else if tag_lower.starts_with("p")
                    || tag_lower.starts_with("br")
                    || tag_lower.starts_with("div")
                {
                    text.push('\n');
                }

                in_tag = false;
            } else if !in_tag && !in_script && !in_style {
                text.push(c);
            }
        }

        text.push('\n');
    }

    // Clean up whitespace
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// S3-6: theme-neutral bilingual EPUB CSS. Previously hardcoded #333/#1a56db/#999
// which were invisible on dark-mode EPUB readers (Apple Books dark, Kobo dark).
// Now original text uses `inherit` to follow the reader's default text color,
// and the translation uses a blue that is readable on both light and dark
// backgrounds. The separator and label use `currentColor` + opacity so they
// adapt to whatever text color the reader applies.
const BILINGUAL_CSS: &str = r#"
.bilingual-orig { color: inherit; margin-bottom: 1.5em; }
.bilingual-trans { color: #2563eb; margin-bottom: 1.5em; padding-left: 1em; border-left: 3px solid #2563eb; }
.bilingual-sep { border: none; border-top: 1px dashed currentColor; opacity: 0.3; margin: 1.5em 0; }
.bilingual-label { font-size: 0.85em; font-weight: bold; color: inherit; opacity: 0.6; text-transform: uppercase; letter-spacing: 0.05em; }
@media (prefers-color-scheme: dark) {
  .bilingual-trans { color: #60a5fa; border-left-color: #60a5fa; }
}
"#;

/// Create a bilingual EPUB file preserving original formatting with translated text interleaved.
///
/// Opens the original EPUB as a ZIP, injects translated text into each chapter's HTML,
/// and writes a new EPUB that can be opened in any EPUB reader.
pub fn create_bilingual_epub(
    original_path: &str,
    output_path: &str,
    translated_chapters: &[TranslatedChapter],
    original_chapters: &[EpubChapter],
) -> Result<(), String> {
    let original_file = fs::File::open(original_path)
        .map_err(|e| format!("Failed to open original EPUB: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(original_file).map_err(|e| format!("Failed to read EPUB as ZIP: {}", e))?;

    // Build index: spine_id → translated text
    let mut trans_map: HashMap<String, &str> = HashMap::new();
    let mut html_map: HashMap<String, &str> = HashMap::new();
    for (_i, ch) in original_chapters.iter().enumerate() {
        if !ch.spine_id.is_empty() {
            html_map.insert(ch.spine_id.clone(), &ch.html_content);
            if let Some(tc) = translated_chapters.iter().find(|t| t.chapter_number == ch.chapter_number) {
                trans_map.insert(ch.spine_id.clone(), &tc.translated_text);
            }
        }
    }

    let out_file = fs::File::create(output_path)
        .map_err(|e| format!("Failed to create output EPUB: {}", e))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Collect all file names first (need to read them before we start writing)
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;
        let name = entry.name().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read entry {}: {}", name, e))?;
        entries.push((name, data));
    }

    // Write all entries, modifying chapter HTML files with translations
    for (name, data) in &entries {
        let is_chapter = name.ends_with(".xhtml")
            || name.ends_with(".html")
            || name.ends_with(".htm");

        let modified = if is_chapter {
            let html_str = String::from_utf8_lossy(data);
            inject_bilingual_content(&html_str, &trans_map, &html_map)
        } else {
            None
        };

        let content = modified.as_deref().unwrap_or_else(|| data.as_slice());

        // mimetype must be stored uncompressed and first
        if name == "mimetype" {
            writer
                .start_file("mimetype", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
                .map_err(|e| format!("Failed to write mimetype: {}", e))?;
        } else {
            writer
                .start_file(name, options)
                .map_err(|e| format!("Failed to start file {}: {}", name, e))?;
        }
        writer
            .write_all(content)
            .map_err(|e| format!("Failed to write {}: {}", name, e))?;
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize EPUB: {}", e))?;

    Ok(())
}

/// Inject bilingual content into a chapter's HTML by matching against stored chapter HTML.
fn inject_bilingual_content(
    current_html: &str,
    trans_map: &HashMap<String, &str>,
    html_map: &HashMap<String, &str>,
) -> Option<Vec<u8>> {
    // Find which spine entry this HTML matches by comparing content
    let mut matched_trans: Option<&&str> = None;
    for (spine_id, original_html) in html_map {
        if current_html == *original_html
            || current_html.contains(original_html)
            || original_html.contains(current_html)
        {
            matched_trans = trans_map.get(spine_id);
            break;
        }
    }

    // Also try direct content comparison
    if matched_trans.is_none() {
        for (spine_id, original_html) in html_map {
            // Compare normalized (trimmed) content
            if current_html.trim() == original_html.trim() {
                matched_trans = trans_map.get(spine_id);
                break;
            }
        }
    }

    let translated = matched_trans?;

    // Format translated text as HTML paragraphs
    let trans_html: String = translated
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| format!("<p>{}</p>", escape_html(l)))
        .collect::<Vec<_>>()
        .join("\n");

    let bilingual_block = format!(
        r#"<hr class="bilingual-sep"/>
<div class="bilingual-trans">
<p class="bilingual-label">Translation</p>
{}
</div>"#,
        trans_html
    );

    // Inject before </body>
    let mut result = String::new();
    if let Some(pos) = current_html.to_lowercase().rfind("</body>") {
        result.push_str(&current_html[..pos]);
        result.push_str(&bilingual_block);
        result.push_str(&current_html[pos..]);
    } else {
        // No body tag found, append at end
        result.push_str(current_html);
        result.push_str(&bilingual_block);
    }

    // Inject CSS into <head>
    if let Some(pos) = result.to_lowercase().rfind("</head>") {
        let style_block = format!("<style>{}</style>", BILINGUAL_CSS);
        let mut final_html = String::new();
        final_html.push_str(&result[..pos]);
        final_html.push_str(&style_block);
        final_html.push_str(&result[pos..]);
        Some(final_html.into_bytes())
    } else {
        // No head tag, prepend style
        let style_block = format!("<style>{}</style>", BILINGUAL_CSS);
        Some(format!("{}{}", style_block, result).into_bytes())
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
