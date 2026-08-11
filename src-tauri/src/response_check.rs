//! Batch / multi-segment response validation (`AiNiee` `ResponseChecker` style).
//! Used when LLM returns numbered or multi-line segment batches.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseCheckOptions {
    /// Fail when any segment translation is empty.
    pub reject_empty: bool,
    /// Fail when translation is identical to source (length ≥ 3 segments).
    pub reject_identical: bool,
    /// Fail when newline counts diverge between source and translation.
    pub check_newlines: bool,
    /// Fail when source and response segment counts differ.
    pub check_count: bool,
}

impl ResponseCheckOptions {
    pub fn strict() -> Self {
        Self {
            reject_empty: true,
            reject_identical: true,
            check_newlines: true,
            check_count: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseCheckResult {
    pub ok: bool,
    pub message: String,
}

/// Validate parallel source/translation segments (same length expected).
pub fn check_segments(
    sources: &[String],
    translations: &[String],
    opts: &ResponseCheckOptions,
) -> ResponseCheckResult {
    if opts.check_count && sources.len() != translations.len() {
        return ResponseCheckResult {
            ok: false,
            message: format!(
                "【行数错误】源 {} 段 / 译 {} 段",
                sources.len(),
                translations.len()
            ),
        };
    }

    let n = sources.len().min(translations.len());
    for i in 0..n {
        let src = sources[i].trim();
        let tr = translations[i].trim();

        if opts.reject_empty && tr.is_empty() && !src.is_empty() {
            return ResponseCheckResult {
                ok: false,
                message: format!("【空译文】第 {} 段", i + 1),
            };
        }

        if opts.check_newlines {
            let src_nl = count_newlines(src);
            let tr_nl = count_newlines(tr);
            if src_nl != tr_nl {
                return ResponseCheckResult {
                    ok: false,
                    message: format!("【换行符数】第 {} 段 源 {} / 译 {}", i + 1, src_nl, tr_nl),
                };
            }
        }
    }

    if opts.reject_identical && n >= 3 {
        let mut equal = 0usize;
        for i in 0..n {
            if sources[i].trim() == translations[i].trim() {
                equal += 1;
            }
        }
        if equal == n {
            return ResponseCheckResult {
                ok: false,
                message: "【返回原文】全部段落与原文相同".to_string(),
            };
        }
    }

    ResponseCheckResult {
        ok: true,
        message: "检查无误".to_string(),
    }
}

/// Parse `1. xxx` / `2. yyy` style LLM batch replies into ordered segments.
pub fn parse_numbered_response(response: &str, expected: usize) -> Option<Vec<String>> {
    if expected == 0 {
        return Some(Vec::new());
    }
    let mut items: Vec<(usize, String)> = Vec::new();
    let mut current_num: Option<usize> = None;
    let mut current_buf = String::new();

    for line in response.lines() {
        let trimmed = line.trim();
        if let Some((num, rest)) = split_numbered_prefix(trimmed) {
            if let Some(n) = current_num {
                items.push((n, current_buf.trim().to_string()));
            }
            current_num = Some(num);
            current_buf = rest.to_string();
        } else if current_num.is_some() {
            if !current_buf.is_empty() {
                current_buf.push('\n');
            }
            current_buf.push_str(trimmed);
        }
    }
    if let Some(n) = current_num {
        items.push((n, current_buf.trim().to_string()));
    }

    if items.is_empty() {
        return None;
    }

    items.sort_by_key(|(n, _)| *n);
    let mut out = Vec::with_capacity(expected);
    for i in 1..=expected {
        match items.iter().find(|(n, _)| *n == i) {
            Some((_, t)) => out.push(t.clone()),
            None => return None,
        }
    }
    Some(out)
}

fn split_numbered_prefix(line: &str) -> Option<(usize, &str)> {
    let mut digit_end = 0usize;
    for (idx, ch) in line.char_indices() {
        if ch.is_ascii_digit() {
            digit_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if digit_end == 0 {
        return None;
    }
    let rest_all = &line[digit_end..];
    let mut chars = rest_all.chars();
    let sep = chars.next()?;
    // ASCII . ) or CJK enumeration comma U+3001
    if sep != '.' && sep != ')' && sep != '\u{3001}' {
        return None;
    }
    let num: usize = line[..digit_end].parse().ok()?;
    let rest = chars.as_str().trim_start();
    Some((num, rest))
}

fn count_newlines(s: &str) -> usize {
    s.matches('\n').count() + s.matches("\\n").count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_count_mismatch() {
        let r = check_segments(
            &["a".into(), "b".into()],
            &["x".into()],
            &ResponseCheckOptions::strict(),
        );
        assert!(!r.ok);
        assert!(r.message.contains("行数"));
    }

    #[test]
    fn test_check_empty() {
        let r = check_segments(
            &["hello".into()],
            &[String::new()],
            &ResponseCheckOptions {
                reject_empty: true,
                ..Default::default()
            },
        );
        assert!(!r.ok);
    }

    #[test]
    fn test_check_newlines() {
        let r = check_segments(
            &["a\nb".into()],
            &["x".into()],
            &ResponseCheckOptions {
                check_newlines: true,
                check_count: true,
                ..Default::default()
            },
        );
        assert!(!r.ok);
        assert!(r.message.contains("换行"));
    }

    #[test]
    fn test_reject_all_identical() {
        let src = vec!["a".into(), "b".into(), "c".into()];
        let r = check_segments(&src, &src, &ResponseCheckOptions::strict());
        assert!(!r.ok);
        assert!(r.message.contains("原文"));
    }

    #[test]
    fn test_parse_numbered() {
        let raw = "1. 你好\n2. 世界\n3. 测试";
        let parsed = parse_numbered_response(raw, 3).unwrap();
        assert_eq!(parsed, vec!["你好", "世界", "测试"]);
    }

    #[test]
    fn test_parse_numbered_multiline() {
        let raw = "1. lineA\ncont\n2. lineB";
        let parsed = parse_numbered_response(raw, 2).unwrap();
        assert_eq!(parsed[0], "lineA\ncont");
        assert_eq!(parsed[1], "lineB");
    }

    #[test]
    fn test_ok_path() {
        let r = check_segments(
            &["a".into(), "b".into()],
            &["x".into(), "y".into()],
            &ResponseCheckOptions::strict(),
        );
        assert!(r.ok);
    }
}
