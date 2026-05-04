use epub::doc::EpubDoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpubChapter {
    pub chapter_number: usize,
    pub title: String,
    pub text: String,
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
    let mut epub = EpubDoc::new(file_path)
        .map_err(|e| format!("Failed to open EPUB file: {}", e))?;

    let title = epub.mdata("title")
        .map(|m| m.value.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let mut chapters = Vec::new();
    let mut chapter_num = 0;

    // Get all spine items (reading order)
    let spine = epub.spine.clone();

    for spine_item in &spine {
        if let Some((content, _)) = epub.get_resource(&spine_item.idref) {
            let text = String::from_utf8_lossy(&content).to_string();

            // Extract text from HTML content
            let extracted = extract_text_from_html(&text);
            if !extracted.trim().is_empty() {
                chapter_num += 1;
                chapters.push(EpubChapter {
                    chapter_number: chapter_num,
                    title: format!("Chapter {}", chapter_num),
                    text: extracted,
                });
            }
        }
    }

    // If no chapters found from spine, try to get all resources
    if chapters.is_empty() {
        epub.set_current_chapter(0);
        while epub.go_next() {
            if let Some((content, _)) = epub.get_current() {
                let text = String::from_utf8_lossy(&content).to_string();
                let extracted = extract_text_from_html(&text);
                if !extracted.trim().is_empty() {
                    chapter_num += 1;
                    chapters.push(EpubChapter {
                        chapter_number: chapter_num,
                        title: epub.get_title().unwrap_or_else(|| format!("Chapter {}", chapter_num)),
                        text: extracted,
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
                } else if tag_lower.starts_with("p") || tag_lower.starts_with("br") || tag_lower.starts_with("div") {
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
