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
    /// Original file text (needed for ASS/SSA export that rewrites Dialogue lines).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_content: Option<String>,
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
            let text = parts[9..]
                .join(",")
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
            _ => {},
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
        let end_time = time_parts[1]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

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
        raw_content: Some(content),
    })
}

fn export_cue_text(entry: &SubtitleEntry, bilingual: bool) -> String {
    let translated = entry.translated_text.trim();
    let original = entry.original_text.trim();
    if bilingual && !original.is_empty() && !translated.is_empty() {
        format!("{}\n{}", original, translated)
    } else if !translated.is_empty() {
        translated.to_string()
    } else {
        original.to_string()
    }
}

/// Generate SRT output from translated entries
pub fn generate_srt(entries: &[SubtitleEntry], bilingual: bool) -> String {
    let mut output = String::new();

    for entry in entries {
        output.push_str(&format!("{}\n", entry.index));
        output.push_str(&format!("{} --> {}\n", entry.start_time, entry.end_time));
        output.push_str(&format!("{}\n\n", export_cue_text(entry, bilingual)));
    }

    output
}

/// Generate ASS output with bilingual text
///
/// S5-11: previously this function did a raw `replace('\n', "\\N")` on
/// original/translated text and concatenated them with `\N`. That broke on:
///   - `\r\n` line endings (the `\r` survived and corrupted the line)
///   - `{` / `}` in text (ASS interprets these as override-tag delimiters)
///   - empty translations (produced a trailing `\N` with nothing after it)
/// We now route through `escape_ass_text` which handles all three cases.
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
                // Replace the text portion with bilingual text.
                // splitn(10, ',') — Text is the 10th field and is allowed to
                // contain commas (it's the last field on the line).
                let parts: Vec<&str> = trimmed.splitn(10, ',').collect();
                if parts.len() >= 10 {
                    let prefix = parts[..9].join(",");
                    let bilingual_text = build_ass_bilingual_text(
                        &entry.original_text,
                        &entry.translated_text,
                    );
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

/// S5-11: build the ASS Text field for a bilingual cue.
///
/// - Empty translation → just the (escaped) original, no trailing `\N`.
/// - Empty original → just the (escaped) translation.
/// - Both present → `original\Ntranslation`.
fn build_ass_bilingual_text(original: &str, translated: &str) -> String {
    let orig = escape_ass_text(original);
    let trans = escape_ass_text(translated);
    match (orig.is_empty(), trans.is_empty()) {
        (true, true) => String::new(),
        (true, false) => trans,
        (false, true) => orig,
        (false, false) => format!("{}\\N{}", orig, trans),
    }
}

/// S5-11: escape a text string for safe inclusion in an ASS Dialogue Text
/// field.
///
/// ASS / SSA spec mandates:
///   - `\r\n` and `\n` → `\N` (hard line break)
///   - `\r` alone → `\N` (defensive: treat lone CR as a line break too)
///   - `{` and `}` → these delimit override tags (`{\b1}bold{\b0}`);
///     literal braces in subtitle text must be escaped or they'll be
///     silently swallowed / mis-parsed by libass. We wrap them with
///     `{\}` escape — the canonical ASS way to emit a literal brace is
///     actually just to avoid the override block; the simplest portable
///     fix is to replace `{` with `\{` is NOT standard. Instead we use
///     the `\\h` (hard space) approach is wrong too. The correct approach
///     per libass: there is no escape for `{` `}` — you must not emit
///     them raw. We replace them with full-width braces `｛｝` which are
///     visually similar and safe. This matches what most subtitle editors
///     (Aegisub, Subtitle Edit) do when sanitizing pasted text.
fn escape_ass_text(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\r' => {
                // Lone CR or CR+LF → hard line break.
                // (The following '\n' if present will be consumed by the
                // iterator and also map to '\N', but we de-duplicate below.)
                s.push_str("\\N");
            },
            '\n' => {
                s.push_str("\\N");
            },
            '{' => {
                s.push('｛');
            },
            '}' => {
                s.push('｝');
            },
            _ => s.push(ch),
        }
    }
    // Collapse doubled `\N\N` that came from `\r\n` sequences.
    s.replace("\\N\\N", "\\N")
}

/// Generate VTT output
pub fn generate_vtt(entries: &[SubtitleEntry], bilingual: bool) -> String {
    let mut output = String::from("WEBVTT\n\n");

    for entry in entries {
        output.push_str(&format!("{}\n", entry.index));
        output.push_str(&format!("{} --> {}\n", entry.start_time, entry.end_time));
        output.push_str(&format!("{}\n\n", export_cue_text(entry, bilingual)));
    }

    output
}

/// Generate LRC output (optionally bilingual)
pub fn generate_lrc(entries: &[SubtitleEntry], bilingual: bool) -> String {
    let mut output = String::new();

    for entry in entries {
        if bilingual && !entry.original_text.is_empty() {
            output.push_str(&format!("{}{}\n", entry.start_time, entry.original_text));
        }
        let text = if entry.translated_text.is_empty() {
            &entry.original_text
        } else {
            &entry.translated_text
        };
        if bilingual {
            output.push_str(&format!("{}[译] {}\n", entry.start_time, text));
        } else {
            output.push_str(&format!("{}{}\n", entry.start_time, text));
        }
    }

    output
}

/// Export subtitle from in-memory entries (must include translations).
pub fn export_subtitle(document: &SubtitleDocument, bilingual: bool) -> String {
    match document.format.as_str() {
        "srt" => generate_srt(&document.entries, bilingual),
        "vtt" => generate_vtt(&document.entries, bilingual),
        "lrc" => generate_lrc(&document.entries, bilingual),
        "ass" | "ssa" => {
            if let Some(raw) = document.raw_content.as_deref().filter(|s| !s.is_empty()) {
                generate_ass_bilingual(raw, &document.entries)
            } else {
                // No raw ASS skeleton — emit SRT rather than lying about format structure
                generate_srt(&document.entries, bilingual)
            }
        }
        _ => generate_srt(&document.entries, bilingual),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(index: usize, original: &str, translated: &str) -> SubtitleEntry {
        SubtitleEntry {
            index,
            start_time: "00:00:01,000".into(),
            end_time: "00:00:02,000".into(),
            original_text: original.into(),
            translated_text: translated.into(),
        }
    }

    #[test]
    fn export_srt_keeps_translations() {
        let doc = SubtitleDocument {
            entries: vec![sample_entry(1, "Hello", "你好")],
            total_entries: 1,
            format: "srt".into(),
            raw_content: None,
        };
        let out = export_subtitle(&doc, false);
        assert!(
            out.contains("你好"),
            "export must use in-memory translation: {out}"
        );
        assert!(!out.contains("Hello"));
    }

    #[test]
    fn export_srt_bilingual_includes_both() {
        let doc = SubtitleDocument {
            entries: vec![sample_entry(1, "Hello", "你好")],
            total_entries: 1,
            format: "srt".into(),
            raw_content: None,
        };
        let out = export_subtitle(&doc, true);
        assert!(out.contains("Hello"));
        assert!(out.contains("你好"));
    }

    #[test]
    fn export_ass_uses_ass_generator() {
        let raw = "[Script Info]\nTitle: test\n\n[Events]\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n";
        let doc = SubtitleDocument {
            entries: vec![sample_entry(1, "Hello", "你好")],
            total_entries: 1,
            format: "ass".into(),
            raw_content: Some(raw.into()),
        };
        let out = export_subtitle(&doc, true);
        assert!(out.contains("Dialogue:"), "expected ASS dialogue line: {out}");
        assert!(
            out.contains("你好") || out.contains(r"\N"),
            "expected translation in ASS: {out}"
        );
    }

    // ── S5-11: ASS bilingual robustness ──────────────────────────────────

    #[test]
    fn ass_escape_newlines_to_hard_break() {
        assert_eq!(escape_ass_text("line1\nline2"), "line1\\Nline2");
        // \r\n should collapse to a single \N, not \N\N
        assert_eq!(escape_ass_text("line1\r\nline2"), "line1\\Nline2");
        // lone \r
        assert_eq!(escape_ass_text("a\rb"), "a\\Nb");
    }

    #[test]
    fn ass_escape_braces_to_fullwidth() {
        // { and } would be interpreted as override-tag delimiters by libass.
        assert_eq!(escape_ass_text("{bold}"), "｛bold｝");
    }

    #[test]
    fn ass_escape_preserves_plain_text() {
        assert_eq!(escape_ass_text("Hello, world!"), "Hello, world!");
        // commas are fine — Text is the last field, commas don't split it
        assert_eq!(escape_ass_text("a,b,c"), "a,b,c");
    }

    #[test]
    fn ass_bilingual_text_empty_translation() {
        // No trailing \N when translation is empty
        let text = build_ass_bilingual_text("Hello", "");
        assert_eq!(text, "Hello");
        assert!(!text.contains("\\N"));
    }

    #[test]
    fn ass_bilingual_text_empty_original() {
        let text = build_ass_bilingual_text("", "你好");
        assert_eq!(text, "你好");
        assert!(!text.contains("\\N"));
    }

    #[test]
    fn ass_bilingual_text_both_present() {
        let text = build_ass_bilingual_text("Hello", "你好");
        assert_eq!(text, "Hello\\N你好");
    }

    #[test]
    fn ass_bilingual_text_both_empty() {
        assert_eq!(build_ass_bilingual_text("", ""), "");
    }

    #[test]
    fn ass_bilingual_with_multiline_original() {
        // Multiline original should be escaped, not break the Dialogue line
        let text = build_ass_bilingual_text("line1\nline2", "翻译");
        assert_eq!(text, "line1\\Nline2\\N翻译");
    }

    #[test]
    fn ass_generate_handles_crlf_input() {
        // Original ASS file with \r\n line endings — must still parse Dialogue lines
        let raw = "[Script Info]\r\nTitle: test\r\n\r\n[Events]\r\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\r\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\r\n";
        let entries = vec![sample_entry(1, "Hello", "你好")];
        let out = generate_ass_bilingual(raw, &entries);
        assert!(out.contains("你好"), "translation must appear: {out}");
        assert!(
            !out.contains("\r\n你好"),
            "translation should not have stray \\r: {out}"
        );
    }

    #[test]
    fn ass_generate_with_override_tags_in_text() {
        // Text containing { } should be sanitized, not interpreted as tags
        let raw = "[Events]\nFormat: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,original\n";
        let entries = vec![sample_entry(1, "original", "翻译{tag}")];
        let out = generate_ass_bilingual(raw, &entries);
        assert!(
            out.contains("｛tag｝"),
            "braces should be fullwidth-escaped: {out}"
        );
        assert!(
            !out.contains("翻译{tag}"),
            "raw braces must not survive: {out}"
        );
    }
}
