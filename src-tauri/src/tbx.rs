//! TBX (`TermBase` eXchange) parser and exporter.
//!
//! Supports TBX-Basic and TBX-Min formats for importing/exporting terminology entries.
//! Uses quick-xml for XML parsing.

use anyhow::{Context, Result};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// A single TBX term entry containing source and target terms with optional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TbxTermEntry {
    /// Source term
    pub source_term: String,
    /// Target term
    pub target_term: String,
    /// Source language code
    pub source_lang: String,
    /// Target language code
    pub target_lang: String,
    /// Subject field (optional)
    pub subject_field: Option<String>,
    /// Definition in source language (optional)
    pub source_definition: Option<String>,
    /// Definition in target language (optional)
    pub target_definition: Option<String>,
    /// Notes (optional)
    pub note: Option<String>,
    /// Transaction type (e.g., "origination", "modification")
    pub transaction_type: Option<String>,
}

/// Parsed TBX file data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TbxData {
    /// TBX dialect ("TBX-Basic", "TBX-Min", "TBX-Core", etc.)
    pub dialect: String,
    /// Source language (from header)
    pub source_lang: Option<String>,
    /// Target language (from header)
    pub target_lang: Option<String>,
    /// Term entries
    pub entries: Vec<TbxTermEntry>,
}

/// Parse a TBX file from XML string.
///
/// Supports TBX-Basic and TBX-Min formats. Extracts terminology entries
/// with source and target terms.
pub fn parse_tbx(xml: &str) -> Result<TbxData> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut dialect = String::from("TBX-Basic");
    let mut header_source_lang: Option<String> = None;
    let mut header_target_lang: Option<String> = None;
    let mut entries = Vec::new();

    // State tracking
    let mut _in_body = false;
    let mut _in_term_entry = false;
    let mut in_lang_set = false;
    let mut in_tig = false;
    let mut in_term = false;
    let mut in_term_note = false;
    let mut in_descrip = false;
    let mut in_admin = false;

    let mut current_lang = String::new();
    let mut is_source = true;
    let mut current_source_term = String::new();
    let mut current_target_term = String::new();
    let mut current_source_lang = String::new();
    let mut current_target_lang = String::new();
    let mut current_subject_field: Option<String> = None;
    let mut current_source_definition: Option<String> = None;
    let mut current_target_definition: Option<String> = None;
    let mut current_note: Option<String> = None;
    let mut descrip_type = String::new();
    let mut lang_set_count = 0;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref())
                    .to_string()
                    .to_lowercase();
                match tag_name.as_str() {
                    "martif" | "tbx" => {
                        // Extract dialect/type
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            if key == "type" || key == "dialect" {
                                dialect = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    },
                    "header" => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "sourcelanguage" | "srcLang" => {
                                    header_source_lang = Some(val);
                                },
                                "targetlanguage" | "tgtLang" => {
                                    header_target_lang = Some(val);
                                },
                                _ => {},
                            }
                        }
                    },
                    "body" => {
                        _in_body = true;
                    },
                    "termentry" | "conceptEntry" => {
                        _in_term_entry = true;
                        current_source_term.clear();
                        current_target_term.clear();
                        current_source_lang.clear();
                        current_target_lang.clear();
                        current_subject_field = None;
                        current_source_definition = None;
                        current_target_definition = None;
                        current_note = None;
                        lang_set_count = 0;
                    },
                    "langset" | "languageSection" => {
                        in_lang_set = true;
                        lang_set_count += 1;
                        current_lang.clear();
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            if key == "xml:lang" || key == "lang" || key == "language" {
                                current_lang = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        is_source = lang_set_count == 1;
                    },
                    "tig" | "termSection" => {
                        in_tig = true;
                    },
                    "term" => {
                        in_term = true;
                    },
                    "termnote" | "termNote" => {
                        in_term_note = true;
                    },
                    "descrip" | "descripGrp" => {
                        in_descrip = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            if key == "type" {
                                descrip_type = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    },
                    "admin" => {
                        in_admin = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            if key == "type" {
                                descrip_type = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_term && in_tig && in_lang_set {
                    if is_source {
                        current_source_term.push_str(&text);
                        if current_source_lang.is_empty() {
                            current_source_lang = current_lang.clone();
                        }
                    } else {
                        current_target_term.push_str(&text);
                        if current_target_lang.is_empty() {
                            current_target_lang = current_lang.clone();
                        }
                    }
                } else if in_term_note && in_tig {
                    // Term notes are metadata, skip for now
                    let _ = text;
                } else if in_descrip {
                    match descrip_type.to_lowercase().as_str() {
                        "subjectField" | "subject" => {
                            current_subject_field = Some(text);
                        },
                        "definition" => {
                            if is_source {
                                current_source_definition = Some(text);
                            } else {
                                current_target_definition = Some(text);
                            }
                        },
                        _ => {},
                    }
                } else if in_admin && descrip_type.to_lowercase() == "note" {
                    current_note = Some(text);
                }
            },
            Ok(Event::CData(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if in_term && in_tig && in_lang_set {
                    if is_source {
                        current_source_term.push_str(&text);
                    } else {
                        current_target_term.push_str(&text);
                    }
                }
            },
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref())
                    .to_string()
                    .to_lowercase();
                match tag_name.as_str() {
                    "termentry" | "conceptEntry" => {
                        _in_term_entry = false;
                        if !current_source_term.is_empty() && !current_target_term.is_empty() {
                            entries.push(TbxTermEntry {
                                source_term: current_source_term.trim().to_string(),
                                target_term: current_target_term.trim().to_string(),
                                source_lang: if current_source_lang.is_empty() {
                                    header_source_lang
                                        .clone()
                                        .unwrap_or_else(|| "en".to_string())
                                } else {
                                    current_source_lang.clone()
                                },
                                target_lang: if current_target_lang.is_empty() {
                                    header_target_lang
                                        .clone()
                                        .unwrap_or_else(|| "zh".to_string())
                                } else {
                                    current_target_lang.clone()
                                },
                                subject_field: current_subject_field.clone(),
                                source_definition: current_source_definition.clone(),
                                target_definition: current_target_definition.clone(),
                                note: current_note.clone(),
                                transaction_type: None,
                            });
                        }
                    },
                    "langset" | "languageSection" => {
                        in_lang_set = false;
                    },
                    "tig" | "termSection" => {
                        in_tig = false;
                    },
                    "term" => {
                        in_term = false;
                    },
                    "termnote" | "termNote" => {
                        in_term_note = false;
                    },
                    "descrip" | "descripGrp" => {
                        in_descrip = false;
                        descrip_type.clear();
                    },
                    "admin" => {
                        in_admin = false;
                        descrip_type.clear();
                    },
                    "body" => {
                        _in_body = false;
                    },
                    _ => {},
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!("TBX parse error: {e}"));
            },
            _ => {},
        }
        buf.clear();
    }

    Ok(TbxData {
        dialect,
        source_lang: header_source_lang,
        target_lang: header_target_lang,
        entries,
    })
}

/// Export terminology entries to TBX-Basic format XML.
///
/// Generates a valid TBX XML document with proper headers and term entries.
pub fn export_tbx(
    entries: &[TbxTermEntry],
    _source_lang: &str,
    _target_lang: &str,
) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    // TBX root element
    let mut martif = BytesStart::new("martif");
    martif.push_attribute(("type", "TBX-Basic"));
    martif.push_attribute(("xml:lang", "en"));
    writer.write_event(Event::Start(martif))?;

    // Header
    writer.write_event(Event::Start(BytesStart::new("martifHeader")))?;

    let file_desc = BytesStart::new("fileDesc");
    writer.write_event(Event::Start(file_desc))?;

    let source_desc = BytesStart::new("sourceDesc");
    writer.write_event(Event::Start(source_desc))?;
    writer.write_event(Event::Start(BytesStart::new("p")))?;
    writer.write_event(Event::Text(BytesText::new("Exported from MoonTranslator")))?;
    writer.write_event(Event::End(BytesEnd::new("p")))?;
    writer.write_event(Event::End(BytesEnd::new("sourceDesc")))?;

    writer.write_event(Event::End(BytesEnd::new("fileDesc")))?;

    // Encoding description
    writer.write_event(Event::Start(BytesStart::new("encodingDesc")))?;
    let mut p_type = BytesStart::new("p");
    p_type.push_attribute(("type", "DCSName"));
    writer.write_event(Event::Start(p_type))?;
    writer.write_event(Event::Text(BytesText::new("TBX-Basic")))?;
    writer.write_event(Event::End(BytesEnd::new("p")))?;
    writer.write_event(Event::End(BytesEnd::new("encodingDesc")))?;

    writer.write_event(Event::End(BytesEnd::new("martifHeader")))?;

    // Body
    writer.write_event(Event::Start(BytesStart::new("body")))?;

    for (idx, entry) in entries.iter().enumerate() {
        // Term entry
        let mut term_entry = BytesStart::new("termEntry");
        term_entry.push_attribute(("id", format!("t{}", idx + 1).as_str()));
        writer.write_event(Event::Start(term_entry))?;

        // Subject field if present
        if let Some(ref subject) = entry.subject_field {
            let mut descrip = BytesStart::new("descrip");
            descrip.push_attribute(("type", "subjectField"));
            writer.write_event(Event::Start(descrip))?;
            writer.write_event(Event::Text(BytesText::new(subject)))?;
            writer.write_event(Event::End(BytesEnd::new("descrip")))?;
        }

        // Source language set
        let mut src_lang_set = BytesStart::new("langSet");
        src_lang_set.push_attribute(("xml:lang", entry.source_lang.as_str()));
        writer.write_event(Event::Start(src_lang_set))?;

        writer.write_event(Event::Start(BytesStart::new("tig")))?;
        writer.write_event(Event::Start(BytesStart::new("term")))?;
        writer.write_event(Event::Text(BytesText::new(&entry.source_term)))?;
        writer.write_event(Event::End(BytesEnd::new("term")))?;

        // Source definition
        if let Some(ref def) = entry.source_definition {
            let mut descrip = BytesStart::new("descrip");
            descrip.push_attribute(("type", "definition"));
            writer.write_event(Event::Start(descrip))?;
            writer.write_event(Event::Text(BytesText::new(def)))?;
            writer.write_event(Event::End(BytesEnd::new("descrip")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("tig")))?;
        writer.write_event(Event::End(BytesEnd::new("langSet")))?;

        // Target language set
        let mut tgt_lang_set = BytesStart::new("langSet");
        tgt_lang_set.push_attribute(("xml:lang", entry.target_lang.as_str()));
        writer.write_event(Event::Start(tgt_lang_set))?;

        writer.write_event(Event::Start(BytesStart::new("tig")))?;
        writer.write_event(Event::Start(BytesStart::new("term")))?;
        writer.write_event(Event::Text(BytesText::new(&entry.target_term)))?;
        writer.write_event(Event::End(BytesEnd::new("term")))?;

        // Target definition
        if let Some(ref def) = entry.target_definition {
            let mut descrip = BytesStart::new("descrip");
            descrip.push_attribute(("type", "definition"));
            writer.write_event(Event::Start(descrip))?;
            writer.write_event(Event::Text(BytesText::new(def)))?;
            writer.write_event(Event::End(BytesEnd::new("descrip")))?;
        }

        // Note
        if let Some(ref note) = entry.note {
            writer.write_event(Event::Start(BytesStart::new("admin")))?;
            writer.write_event(Event::Text(BytesText::new(note)))?;
            writer.write_event(Event::End(BytesEnd::new("admin")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("tig")))?;
        writer.write_event(Event::End(BytesEnd::new("langSet")))?;

        writer.write_event(Event::End(BytesEnd::new("termEntry")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("body")))?;
    writer.write_event(Event::End(BytesEnd::new("martif")))?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).context("TBX output is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tbx_basic() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<martif type="TBX-Basic" xml:lang="en">
  <martifHeader>
    <fileDesc>
      <sourceDesc><p>Test</p></sourceDesc>
    </fileDesc>
  </martifHeader>
  <body>
    <termEntry id="t1">
      <langSet xml:lang="en">
        <tig><term>computer</term></tig>
      </langSet>
      <langSet xml:lang="zh">
        <tig><term>计算机</term></tig>
      </langSet>
    </termEntry>
  </body>
</martif>"#;

        let data = parse_tbx(xml).unwrap();
        assert_eq!(data.dialect, "TBX-Basic");
        assert_eq!(data.entries.len(), 1);
        assert_eq!(data.entries[0].source_term, "computer");
        assert_eq!(data.entries[0].target_term, "计算机");
    }

    #[test]
    fn test_export_tbx() {
        let entries = vec![TbxTermEntry {
            source_term: "software".to_string(),
            target_term: "软件".to_string(),
            source_lang: "en".to_string(),
            target_lang: "zh".to_string(),
            subject_field: Some("computing".to_string()),
            source_definition: None,
            target_definition: None,
            note: None,
            transaction_type: None,
        }];

        let xml = export_tbx(&entries, "en", "zh").unwrap();
        assert!(xml.contains("martif"));
        assert!(xml.contains("software"));
        assert!(xml.contains("软件"));
        assert!(xml.contains("computing"));
    }

    #[test]
    fn test_roundtrip() {
        let entries = vec![TbxTermEntry {
            source_term: "API".to_string(),
            target_term: "应用程序接口".to_string(),
            source_lang: "en".to_string(),
            target_lang: "zh".to_string(),
            subject_field: Some("programming".to_string()),
            source_definition: Some("Application Programming Interface".to_string()),
            target_definition: None,
            note: Some("Common abbreviation".to_string()),
            transaction_type: None,
        }];

        let xml = export_tbx(&entries, "en", "zh").unwrap();
        let parsed = parse_tbx(&xml).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].source_term, "API");
        assert_eq!(parsed.entries[0].target_term, "应用程序接口");
    }
}
