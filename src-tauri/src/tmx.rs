//! TMX (Translation Memory eXchange) parser and exporter.
//!
//! Supports TMX 1.4 and 2.0 formats for importing/exporting translation memory entries.
//! Uses quick-xml for XML parsing.

use anyhow::{Context, Result};
use chrono::Utc;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::Writer;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// A single TMX translation unit containing source and target segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmxTranslationUnit {
    /// Source language text
    pub source_text: String,
    /// Target language text
    pub target_text: String,
    /// Source language code (e.g., "en", "ja")
    pub source_lang: String,
    /// Target language code (e.g., "zh", "ko")
    pub target_lang: String,
    /// Optional creation date (ISO 8601 or TMX date format)
    pub creation_date: Option<String>,
    /// Optional change date
    pub change_date: Option<String>,
    /// Optional creation user
    pub creation_user: Option<String>,
    /// Optional note
    pub note: Option<String>,
}

/// Parsed TMX file data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmxData {
    /// TMX version ("1.4" or "2.0")
    pub version: String,
    /// Source language defined in header (if any)
    pub header_srclang: Option<String>,
    /// Translation units
    pub units: Vec<TmxTranslationUnit>,
}

/// Case-insensitive language match (`en` ↔ `en-US`).
fn lang_matches(lang: &str, srclang: &str) -> bool {
    if lang.is_empty() || srclang.is_empty() {
        return false;
    }
    let a = lang.to_ascii_lowercase();
    let b = srclang.to_ascii_lowercase();
    a == b
        || a.starts_with(&format!("{b}-"))
        || b.starts_with(&format!("{a}-"))
        || a.split('-').next() == b.split('-').next()
}

/// Parse a TMX file from XML string.
///
/// Supports both TMX 1.4 and 2.0 formats. Extracts translation units (tu)
/// with their source and target variants (tuv).
pub fn parse_tmx(xml: &str) -> Result<TmxData> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut version = String::new();
    let mut header_srclang: Option<String> = None;
    let mut units = Vec::new();

    // State tracking
    let mut in_tu = false;
    let mut in_tuv = false;
    let mut in_seg = false;
    let mut in_note = false;
    let mut current_lang = String::new();
    let mut current_source = String::new();
    let mut current_target = String::new();
    let mut current_note = Option::<String>::None;
    let mut current_creation_date: Option<String> = None;
    let mut current_change_date: Option<String> = None;
    let mut current_creation_user: Option<String> = None;
    let mut is_source_tuv = false;
    let mut tuv_index_in_tu: u32 = 0;
    let mut current_tuv_is_source_lang: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(decl)) => {
                // XML declaration - continue
                let _ = decl;
            },
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "tmx" => {
                        // Extract version attribute
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "version" {
                                version = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    },
                    "header" => {
                        // Extract srclang from header
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "srclang" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                if !val.is_empty() && val != "*" {
                                    header_srclang = Some(val);
                                }
                            }
                        }
                    },
                    "tu" => {
                        in_tu = true;
                        current_source.clear();
                        current_target.clear();
                        current_note = None;
                        current_creation_date = None;
                        current_change_date = None;
                        current_creation_user = None;
                        tuv_index_in_tu = 0;
                        current_tuv_is_source_lang = None;
                    },
                    "tuv" => {
                        in_tuv = true;
                        current_lang.clear();
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "xml:lang" || key == "lang" {
                                current_lang = val;
                            }
                        }
                        is_source_tuv = if let Some(ref src) = header_srclang {
                            lang_matches(&current_lang, src)
                        } else {
                            // First tuv is source when header has no srclang
                            tuv_index_in_tu == 0
                        };
                        if is_source_tuv {
                            current_tuv_is_source_lang = Some(current_lang.clone());
                        }
                        tuv_index_in_tu += 1;
                    },
                    "seg" => {
                        in_seg = true;
                    },
                    "note" => {
                        in_note = true;
                    },
                    "prop" => {
                        // TMX 2.0 properties
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "type" {
                                match val.as_str() {
                                    "x-creation-date" | "creationdate" => {
                                        // Will be set from text content
                                    },
                                    "x-change-date" | "changedate" => {
                                        // Will be set from text content
                                    },
                                    _ => {},
                                }
                            }
                        }
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_seg && in_tuv {
                    if is_source_tuv {
                        current_source.push_str(&text);
                    } else {
                        current_target.push_str(&text);
                    }
                } else if in_note && in_tu {
                    current_note = Some(text);
                }
            },
            Ok(Event::CData(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if in_seg && in_tuv {
                    if is_source_tuv {
                        current_source.push_str(&text);
                    } else {
                        current_target.push_str(&text);
                    }
                }
            },
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "tu" => {
                        in_tu = false;
                        // Determine which is source and which is target
                        if !current_source.is_empty() && !current_target.is_empty() {
                            let src_lang = header_srclang
                                .clone()
                                .or_else(|| current_tuv_is_source_lang.clone())
                                .unwrap_or_else(|| "en".to_string());
                            units.push(TmxTranslationUnit {
                                source_text: current_source.clone(),
                                target_text: current_target.clone(),
                                source_lang: src_lang,
                                target_lang: if current_lang.is_empty() {
                                    "zh".to_string()
                                } else {
                                    current_lang.clone()
                                },
                                creation_date: current_creation_date.clone(),
                                change_date: current_change_date.clone(),
                                creation_user: current_creation_user.clone(),
                                note: current_note.clone(),
                            });
                        }
                    },
                    "tuv" => {
                        in_tuv = false;
                    },
                    "seg" => {
                        in_seg = false;
                    },
                    "note" => {
                        in_note = false;
                    },
                    _ => {},
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!("TMX parse error: {e}"));
            },
            _ => {},
        }
        buf.clear();
    }

    // If we couldn't determine source/target from lang attributes, try header srclang
    // and fix up the units
    if units.is_empty() && !version.is_empty() {
        // No units found with strict parsing, try a more lenient approach
        return parse_tmx_lenient(xml, &version);
    }

    Ok(TmxData {
        version,
        header_srclang,
        units,
    })
}

/// More lenient TMX parser that handles various real-world TMX files.
fn parse_tmx_lenient(xml: &str, version: &str) -> Result<TmxData> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut header_srclang: Option<String> = None;
    let mut units = Vec::new();

    let mut _in_tu = false;
    let mut in_tuv = false;
    let mut in_seg = false;
    let mut tuv_count = 0;
    let mut current_lang = String::new();
    let mut source_lang = String::new();
    let mut target_lang = String::new();
    let mut current_source = String::new();
    let mut current_target = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref())
                    .to_string()
                    .to_lowercase();
                match tag_name.as_str() {
                    "header" => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            if key == "srclang" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                if !val.is_empty() && val != "*" {
                                    header_srclang = Some(val);
                                }
                            }
                        }
                    },
                    "tu" => {
                        _in_tu = true;
                        tuv_count = 0;
                        source_lang.clear();
                        target_lang.clear();
                        current_source.clear();
                        current_target.clear();
                    },
                    "tuv" => {
                        in_tuv = true;
                        tuv_count += 1;
                        current_lang.clear();
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref())
                                .to_string()
                                .to_lowercase();
                            if key == "xml:lang" || key == "lang" || key == "xmllang" {
                                current_lang = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        if tuv_count == 1 {
                            source_lang.clone_from(&current_lang);
                        } else {
                            target_lang.clone_from(&current_lang);
                        }
                    },
                    "seg" => {
                        in_seg = true;
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_seg && in_tuv {
                    if tuv_count == 1 {
                        current_source.push_str(&text);
                    } else {
                        current_target.push_str(&text);
                    }
                }
            },
            Ok(Event::CData(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if in_seg && in_tuv {
                    if tuv_count == 1 {
                        current_source.push_str(&text);
                    } else {
                        current_target.push_str(&text);
                    }
                }
            },
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref())
                    .to_string()
                    .to_lowercase();
                match tag_name.as_str() {
                    "tu" => {
                        _in_tu = false;
                        if !current_source.is_empty() && !current_target.is_empty() {
                            units.push(TmxTranslationUnit {
                                source_text: current_source.trim().to_string(),
                                target_text: current_target.trim().to_string(),
                                source_lang: if source_lang.is_empty() {
                                    header_srclang.clone().unwrap_or_else(|| "en".to_string())
                                } else {
                                    source_lang.clone()
                                },
                                target_lang: if target_lang.is_empty() {
                                    "zh".to_string()
                                } else {
                                    target_lang.clone()
                                },
                                creation_date: None,
                                change_date: None,
                                creation_user: None,
                                note: None,
                            });
                        }
                    },
                    "tuv" => {
                        in_tuv = false;
                    },
                    "seg" => {
                        in_seg = false;
                    },
                    _ => {},
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!("TMX parse error: {e}"));
            },
            _ => {},
        }
        buf.clear();
    }

    Ok(TmxData {
        version: version.to_string(),
        header_srclang,
        units,
    })
}

/// Export translation units to TMX 1.4 format XML.
///
/// Generates a valid TMX XML document with proper headers and translation units.
pub fn export_tmx(
    units: &[TmxTranslationUnit],
    source_lang: &str,
    creation_tool: &str,
) -> Result<String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    // TMX root element
    let mut tmx_start = BytesStart::new("tmx");
    tmx_start.push_attribute(("version", "1.4"));
    writer.write_event(Event::Start(tmx_start))?;

    // Header
    let now = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut header = BytesStart::new("header");
    header.push_attribute(("creationtool", creation_tool));
    header.push_attribute(("creationtoolversion", "1.0"));
    header.push_attribute(("datatype", "PlainText"));
    header.push_attribute(("segtype", "sentence"));
    header.push_attribute(("adminlang", "en"));
    header.push_attribute(("srclang", source_lang));
    header.push_attribute(("creationdate", now.as_str()));
    writer.write_event(Event::Start(header))?;
    writer.write_event(Event::End(BytesEnd::new("header")))?;

    // Body
    writer.write_event(Event::Start(BytesStart::new("body")))?;

    for unit in units {
        // Translation unit
        writer.write_event(Event::Start(BytesStart::new("tu")))?;

        // Source TUV
        let mut src_tuv = BytesStart::new("tuv");
        let src_lang = if unit.source_lang.is_empty() {
            source_lang.to_string()
        } else {
            unit.source_lang.clone()
        };
        src_tuv.push_attribute(("xml:lang", src_lang.as_str()));
        writer.write_event(Event::Start(src_tuv))?;
        writer.write_event(Event::Start(BytesStart::new("seg")))?;
        writer.write_event(Event::Text(BytesText::new(&unit.source_text)))?;
        writer.write_event(Event::End(BytesEnd::new("seg")))?;
        writer.write_event(Event::End(BytesEnd::new("tuv")))?;

        // Target TUV
        let mut tgt_tuv = BytesStart::new("tuv");
        tgt_tuv.push_attribute(("xml:lang", unit.target_lang.as_str()));
        if let Some(ref date) = unit.change_date {
            tgt_tuv.push_attribute(("creationdate", date.as_str()));
        }
        writer.write_event(Event::Start(tgt_tuv))?;
        writer.write_event(Event::Start(BytesStart::new("seg")))?;
        writer.write_event(Event::Text(BytesText::new(&unit.target_text)))?;
        writer.write_event(Event::End(BytesEnd::new("seg")))?;
        if let Some(ref note) = unit.note {
            writer.write_event(Event::Start(BytesStart::new("note")))?;
            writer.write_event(Event::Text(BytesText::new(note)))?;
            writer.write_event(Event::End(BytesEnd::new("note")))?;
        }
        writer.write_event(Event::End(BytesEnd::new("tuv")))?;

        writer.write_event(Event::End(BytesEnd::new("tu")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("body")))?;
    writer.write_event(Event::End(BytesEnd::new("tmx")))?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).context("TMX output is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tmx_14() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tmx version="1.4">
  <header creationtool="Test" datatype="PlainText" segtype="sentence" adminlang="en" srclang="en"/>
  <body>
    <tu>
      <tuv xml:lang="en"><seg>Hello World</seg></tuv>
      <tuv xml:lang="zh"><seg>你好世界</seg></tuv>
    </tu>
    <tu>
      <tuv xml:lang="en"><seg>Good morning</seg></tuv>
      <tuv xml:lang="zh"><seg>早上好</seg></tuv>
    </tu>
  </body>
</tmx>"#;

        let data = parse_tmx(xml).unwrap();
        assert_eq!(data.version, "1.4");
        assert_eq!(data.units.len(), 2);
        assert_eq!(data.units[0].source_text, "Hello World");
        assert_eq!(data.units[0].target_text, "你好世界");
        assert_eq!(data.units[0].source_lang, "en");
        assert_eq!(data.units[1].source_text, "Good morning");
        assert_eq!(data.units[1].target_text, "早上好");
    }

    #[test]
    fn test_is_source_tuv_uses_header_srclang() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tmx version="1.4">
  <header creationtool="Test" datatype="PlainText" segtype="sentence" adminlang="en" srclang="ja"/>
  <body>
    <tu>
      <tuv xml:lang="en"><seg>Should be target</seg></tuv>
      <tuv xml:lang="ja"><seg>ソース</seg></tuv>
    </tu>
  </body>
</tmx>"#;
        let data = parse_tmx(xml).unwrap();
        assert_eq!(data.units.len(), 1);
        assert_eq!(data.units[0].source_text, "ソース");
        assert_eq!(data.units[0].target_text, "Should be target");
        assert_eq!(data.units[0].source_lang, "ja");
    }

    #[test]
    fn test_export_tmx() {
        let units = vec![
            TmxTranslationUnit {
                source_text: "Hello".to_string(),
                target_text: "你好".to_string(),
                source_lang: "en".to_string(),
                target_lang: "zh".to_string(),
                creation_date: None,
                change_date: None,
                creation_user: None,
                note: None,
            },
            TmxTranslationUnit {
                source_text: "World".to_string(),
                target_text: "世界".to_string(),
                source_lang: "en".to_string(),
                target_lang: "zh".to_string(),
                creation_date: None,
                change_date: None,
                creation_user: None,
                note: None,
            },
        ];

        let xml = export_tmx(&units, "en", "MoonTranslator").unwrap();
        assert!(xml.contains("tmx"));
        assert!(xml.contains("Hello"));
        assert!(xml.contains("你好"));
        assert!(xml.contains("World"));
        assert!(xml.contains("世界"));
    }

    #[test]
    fn test_roundtrip() {
        let units = vec![TmxTranslationUnit {
            source_text: "Test".to_string(),
            target_text: "测试".to_string(),
            source_lang: "en".to_string(),
            target_lang: "zh".to_string(),
            creation_date: None,
            change_date: None,
            creation_user: None,
            note: None,
        }];

        let xml = export_tmx(&units, "en", "TestTool").unwrap();
        let parsed = parse_tmx(&xml).unwrap();
        assert_eq!(parsed.units.len(), 1);
        assert_eq!(parsed.units[0].source_text, "Test");
        assert_eq!(parsed.units[0].target_text, "测试");
    }
}
