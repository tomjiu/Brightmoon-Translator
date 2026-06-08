//! Text alignment utilities for matching source and translated text segments.
//!
//! Provides paragraph-level alignment for building translation memory entries
//! from source/translated text pairs.

use serde::{Deserialize, Serialize};

/// An aligned pair of source and translated text segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignedSegment {
    pub source: String,
    pub target: String,
    /// Alignment confidence score (0.0 to 1.0)
    pub confidence: f64,
}

/// Align source and translated text at paragraph level.
///
/// Splits both texts by paragraph boundaries and attempts to match them.
/// Uses multiple heuristics for alignment:
/// 1. Count-based matching (same number of paragraphs)
/// 2. Length-ratio matching (proportional lengths)
/// 3. Sentence boundary detection
pub fn align_paragraphs(source: &str, target: &str) -> Vec<AlignedSegment> {
    let source_paragraphs = split_paragraphs(source);
    let target_paragraphs = split_paragraphs(target);

    if source_paragraphs.is_empty() || target_paragraphs.is_empty() {
        return Vec::new();
    }

    // If same count, direct 1:1 alignment
    if source_paragraphs.len() == target_paragraphs.len() {
        return source_paragraphs
            .iter()
            .zip(target_paragraphs.iter())
            .map(|(s, t)| AlignedSegment {
                source: s.clone(),
                target: t.clone(),
                confidence: 0.9,
            })
            .collect();
    }

    // Otherwise, use ratio-based alignment
    align_by_ratio(&source_paragraphs, &target_paragraphs)
}

/// Split text into paragraphs by common delimiters.
fn split_paragraphs(text: &str) -> Vec<String> {
    text.split(|c: char| c == '\n' || c == '\r')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Align paragraphs using length-ratio matching.
///
/// Groups shorter segments together or splits longer segments
/// to match the expected count.
fn align_by_ratio(source: &[String], target: &[String]) -> Vec<AlignedSegment> {
    let src_count = source.len();
    let tgt_count = target.len();

    if src_count == 0 || tgt_count == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();

    // Simple approach: align proportionally
    let ratio = src_count as f64 / tgt_count as f64;

    if ratio >= 1.0 {
        // More source paragraphs than target - group source paragraphs
        let mut tgt_idx = 0;
        let mut src_group = Vec::new();

        for (i, src) in source.iter().enumerate() {
            src_group.push(src.clone());
            let expected_tgt = ((i + 1) as f64 / ratio) as usize;
            if expected_tgt > tgt_idx && tgt_idx < tgt_count {
                result.push(AlignedSegment {
                    source: src_group.join("\n"),
                    target: target[tgt_idx].clone(),
                    confidence: 0.7,
                });
                src_group.clear();
                tgt_idx += 1;
            }
        }

        // Handle remaining
        if !src_group.is_empty() && tgt_idx < tgt_count {
            result.push(AlignedSegment {
                source: src_group.join("\n"),
                target: target[tgt_idx].clone(),
                confidence: 0.5,
            });
        }
    } else {
        // More target paragraphs than source - group target paragraphs
        let mut src_idx = 0;
        let mut tgt_group = Vec::new();

        for (i, tgt) in target.iter().enumerate() {
            tgt_group.push(tgt.clone());
            let expected_src = ((i + 1) as f64 * ratio) as usize;
            if expected_src > src_idx && src_idx < src_count {
                result.push(AlignedSegment {
                    source: source[src_idx].clone(),
                    target: tgt_group.join("\n"),
                    confidence: 0.7,
                });
                tgt_group.clear();
                src_idx += 1;
            }
        }

        // Handle remaining
        if !tgt_group.is_empty() && src_idx < src_count {
            result.push(AlignedSegment {
                source: source[src_idx].clone(),
                target: tgt_group.join("\n"),
                confidence: 0.5,
            });
        }
    }

    result
}

/// Align text at sentence level within a single paragraph pair.
///
/// Useful for fine-grained alignment when paragraph counts don't match.
pub fn align_sentences(source: &str, target: &str) -> Vec<AlignedSegment> {
    let source_sentences = split_sentences(source);
    let target_sentences = split_sentences(target);

    if source_sentences.is_empty() || target_sentences.is_empty() {
        return Vec::new();
    }

    if source_sentences.len() == target_sentences.len() {
        return source_sentences
            .iter()
            .zip(target_sentences.iter())
            .map(|(s, t)| AlignedSegment {
                source: s.clone(),
                target: t.clone(),
                confidence: 0.85,
            })
            .collect();
    }

    // Use ratio-based alignment for mismatched counts
    align_by_ratio(&source_sentences, &target_sentences)
}

/// Split text into sentences by common sentence boundaries.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if ch == '.' || ch == '!' || ch == '?' || ch == '\n' || ch == '。' || ch == '！' || ch == '？' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_equal_paragraphs() {
        let source = "Hello\nWorld";
        let target = "你好\n世界";
        let result = align_paragraphs(source, target);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].source, "Hello");
        assert_eq!(result[0].target, "你好");
        assert_eq!(result[1].source, "World");
        assert_eq!(result[1].target, "世界");
    }

    #[test]
    fn test_align_single_paragraph() {
        let result = align_paragraphs("Hello World", "你好世界");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, "Hello World");
        assert_eq!(result[0].target, "你好世界");
    }

    #[test]
    fn test_align_empty() {
        let result = align_paragraphs("", "你好");
        assert!(result.is_empty());
    }

    #[test]
    fn test_split_sentences() {
        let sentences = split_sentences("Hello. World! How are you?");
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_align_sentences() {
        let result = align_sentences("Hello. World.", "你好。世界。");
        assert_eq!(result.len(), 2);
    }
}
