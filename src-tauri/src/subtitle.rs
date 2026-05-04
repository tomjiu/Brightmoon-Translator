use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleEntry {
    pub index: usize,
    pub start_time: String,
    pub end_time: String,
    pub original_text: String,
    pub translated_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleDocument {
    pub entries: Vec<SubtitleEntry>,
    pub total_entries: usize,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSubtitle {
    pub entries: Vec<SubtitleEntry>,
    pub total_entries: usize,
    pub format: String,
}

/// Parse SRT subtitle format
fn parse_srt(content: &str) -> Vec<SubtitleEntry> {
    let mut entries = Vec::new();
    let blocks: Vec<&str> = content.split("\n\n").collect();

    for block in blocks {
        let lines: Vec<&str> = block.trim().lines().collect();
        if lines.len() < 3 {
            continue;
        }

        // First line: index
        let index: usize = match lines[0].trim().parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Second line: timestamps
        let time_parts: Vec<&str> = lines[1].split(" --> ").collect();
        if time_parts.len() != 2 {
            continue;
        }

        let start_time = time_parts[0].trim().to_string();
        let end_time = time_parts[1].trim().to_string();

        // Remaining lines: text
        let text = lines[2..].join("\n");

        entries.push(SubtitleEntry {
            index,
            start_time,
            end_time,
            original_text: text,
            translated_text: String::new(),
        });
    }

    entries
}

/// Parse ASS/SSA subtitle format
fn parse_ass(content: &str) -> Vec<SubtitleEntry> {
    let mut entries = Vec::new();
    let mut in_events = false;
    let mut index = 1;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[Events]" {
            in_events = true;
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_events = false;
            continue;
        }

        if !in_events {
            continue;
        }

        if trimmed.starts_with("Format:") {
            continue;
        }

        if let Some(dialogue) = trimmed.strip_prefix("Dialogue:") {
            let parts: Vec<&str> = dialogue.splitn(10, ',').collect();
            if parts.len() < 10 {
                continue;
            }

            let start_time = parts[1].trim().to_string();
            let end_time = parts[2].trim().to_string();
            // Text is the last field after "Text" in format
            let text = parts[9..].join(",")
                .replace("\\N", "\n")
                .replace("\\n", "\n")
                .replace(r"\h", " ");

            // Strip ASS tags like {\b1}, {\i0}, etc.
            let clean_text = strip_ass_tags(&text);

            entries.push(SubtitleEntry {
                index,
                start_time,
                end_time,
                original_text: clean_text,
                translated_text: String::new(),
            });
            index += 1;
        }
    }

    entries
}

/// Strip ASS style tags from text
fn strip_ass_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in text.chars() {
        match ch {
            '{' => in_tag = true,
            '}' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    result.trim().to_string()
}

/// Parse VTT subtitle format
fn parse_vtt(content: &str) -> Vec<SubtitleEntry> {
    let mut entries = Vec::new();
    let mut index = 1;

    // Skip WEBVTT header and metadata
    let content = if let Some(pos) = content.find("\n\n") {
        &content[pos + 2..]
    } else {
        content
    };

    let blocks: Vec<&str> = content.split("\n\n").collect();

    for block in blocks {
        let lines: Vec<&str> = block.trim().lines().collect();
        if lines.is_empty() {
            continue;
        }

        let mut time_line_idx = 0;
        let mut has_timestamp = false;

        // Find the timestamp line
        for (i, line) in lines.iter().enumerate() {
            if line.contains("-->") {
                time_line_idx = i;
                has_timestamp = true;
                break;
            }
        }

        if !has_timestamp {
            continue;
        }

        let time_parts: Vec<&str> = lines[time_line_idx].split(" --> ").collect();
        if time_parts.len() != 2 {
            continue;
        }

        let start_time = time_parts[0].trim().to_string();
        let end_time = time_parts[1].split_whitespace().next().unwrap_or("").to_string();

        // Text lines after timestamp
        let text = if time_line_idx + 1 < lines.len() {
            lines[time_line_idx + 1..].join("\n")
        } else {
            continue;
        };

        entries.push(SubtitleEntry {
            index,
            start_time,
            end_time,
            original_text: text,
            translated_text: String::new(),
        });
        index += 1;
    }

    entries
}

/// Parse LRC lyrics format
fn parse_lrc(content: &str) -> Vec<SubtitleEntry> {
    let mut entries = Vec::new();
    let mut index = 1;

    for line in content.lines() {
        let trimmed = line.trim();

        // Match [MM:SS.xx] or [MM:SS] pattern
        if trimmed.starts_with('[') {
            if let Some(end_bracket) = trimmed.find(']') {
                let time_str = &trimmed[1..end_bracket];
                let text = trimmed[end_bracket + 1..].trim();

                // Skip metadata tags like [ti:], [ar:], [al:]
                if text.is_empty() || time_str.contains(':') && !time_str.contains('.') {
                    // Check if it's a metadata tag
                    if text.is_empty() {
                        continue;
                    }
                }

                // Convert LRC time to display format
                let start_time = format!("[{}]", time_str);
                let end_time = String::new(); // LRC doesn't have end time

                entries.push(SubtitleEntry {
                    index,
                    start_time,
                    end_time,
                    original_text: text.to_string(),
                    translated_text: String::new(),
                });
                index += 1;
            }
        }
    }

    entries
}

/// Detect subtitle format from file extension and content
pub fn detect_format(file_path: &str) -> String {
    let path_lower = file_path.to_lowercase();

    if path_lower.ends_with(".srt") {
        "srt".to_string()
    } else if path_lower.ends_with(".ass") || path_lower.ends_with(".ssa") {
        "ass".to_string()
    } else if path_lower.ends_with(".vtt") {
        "vtt".to_string()
    } else if path_lower.ends_with(".lrc") {
        "lrc".to_string()
    } else {
        // Try to detect from content
        "srt".to_string()
    }
}

/// Extract text from subtitle file
pub fn extract_text_from_subtitle(file_path: &str) -> Result<SubtitleDocument, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read subtitle file: {}", e))?;

    let format = detect_format(file_path);

    let entries = match format.as_str() {
        "srt" => parse_srt(&content),
        "ass" | "ssa" => parse_ass(&content),
        "vtt" => parse_vtt(&content),
        "lrc" => parse_lrc(&content),
        _ => parse_srt(&content), // Default to SRT
    };

    let total_entries = entries.len();

    Ok(SubtitleDocument {
        entries,
        total_entries,
        format,
    })
}

/// Generate SRT output from translated entries
pub fn generate_srt(entries: &[SubtitleEntry]) -> String {
    let mut output = String::new();

    for entry in entries {
        output.push_str(&format!("{}\n", entry.index));
        output.push_str(&format!("{} --> {}\n", entry.start_time, entry.end_time));
        output.push_str(&format!("{}\n\n", entry.translated_text));
    }

    output
}

/// Generate ASS output with bilingual text
pub fn generate_ass_bilingual(original_content: &str, entries: &[SubtitleEntry]) -> String {
    let mut output = String::new();
    let mut in_events = false;
    let mut entry_idx = 0;

    for line in original_content.lines() {
        let trimmed = line.trim();

        if trimmed == "[Events]" {
            in_events = true;
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_events = false;
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if !in_events || trimmed.starts_with("Format:") {
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if trimmed.starts_with("Dialogue:") {
            if entry_idx < entries.len() {
                let entry = &entries[entry_idx];
                // Replace the text portion with bilingual text
                let parts: Vec<&str> = trimmed.splitn(10, ',').collect();
                if parts.len() >= 10 {
                    let prefix = parts[..9].join(",");
                    let bilingual_text = format!("{}\\N{}", entry.original_text.replace('\n', "\\N"), entry.translated_text.replace('\n', "\\N"));
                    output.push_str(&format!("{},{}", prefix, bilingual_text));
                } else {
                    output.push_str(line);
                }
                entry_idx += 1;
            } else {
                output.push_str(line);
            }
            output.push('\n');
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

/// Generate VTT output
pub fn generate_vtt(entries: &[SubtitleEntry]) -> String {
    let mut output = String::from("WEBVTT\n\n");

    for entry in entries {
        output.push_str(&format!("{}\n", entry.index));
        output.push_str(&format!("{} --> {}\n", entry.start_time, entry.end_time));
        output.push_str(&format!("{}\n\n", entry.translated_text));
    }

    output
}

/// Generate LRC output with bilingual text
pub fn generate_lrc_bilingual(entries: &[SubtitleEntry]) -> String {
    let mut output = String::new();

    for entry in entries {
        // Original line
        output.push_str(&format!("{}{}\n", entry.start_time, entry.original_text));
        // Translation line (with offset for visual alignment)
        output.push_str(&format!("{}[译] {}\n", entry.start_time, entry.translated_text));
    }

    output
}

/// Export translated subtitle in the original format
pub fn export_subtitle(document: &SubtitleDocument, bilingual: bool) -> String {
    match document.format.as_str() {
        "srt" => generate_srt(&document.entries),
        "vtt" => generate_vtt(&document.entries),
        "lrc" => generate_lrc_bilingual(&document.entries),
        _ => generate_srt(&document.entries),
    }
}
