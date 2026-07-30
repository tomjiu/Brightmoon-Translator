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
/// 2. Proportional distribution (mismatched counts, e.g. 3:5 → 2:2:1)
/// 3. Sentence-level fallback (when counts differ by 2x or more)
pub fn align_paragraphs(source: &str, target: &str) -> Vec<AlignedSegment> {
    let source_paragraphs = split_paragraphs(source);
    let target_paragraphs = split_paragraphs(target);

    if source_paragraphs.is_empty() || target_paragraphs.is_empty() {
        return Vec::new();
    }

    let src_count = source_paragraphs.len();
    let tgt_count = target_paragraphs.len();

    // If same count, direct 1:1 alignment
    if src_count == tgt_count {
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

    // When counts differ by 2x or more, try sentence-level alignment first
    let ratio = src_count.max(tgt_count) as f64 / src_count.min(tgt_count) as f64;
    if ratio >= 2.0 {
        let sentence_result = align_sentences(source, target);
        if !sentence_result.is_empty() {
            return sentence_result;
        }
    }

    // Otherwise, use proportional distribution
    align_by_ratio(&source_paragraphs, &target_paragraphs)
}

/// Split text into paragraphs by common delimiters.
fn split_paragraphs(text: &str) -> Vec<String> {
    text.split(|c: char| c == '\n' || c == '\r')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Align paragraphs using proportional distribution.
///
/// Uses ceil-division to evenly distribute segments across mismatched counts.
/// For example, 3 source and 5 target paragraphs → 2:2:1 distribution.
fn align_by_ratio(source: &[String], target: &[String]) -> Vec<AlignedSegment> {
    let src_count = source.len();
    let tgt_count = target.len();

    if src_count == 0 || tgt_count == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();

    if src_count <= tgt_count {
        // More (or equal) target paragraphs than source.
        // Distribute targets proportionally across sources.
        let mut tgt_idx = 0;
        for (src_idx, src) in source.iter().enumerate() {
            let remaining_src = src_count - src_idx;
            let remaining_tgt = tgt_count - tgt_idx;
            // ceil division: how many targets this source should get
            let count = (remaining_tgt + remaining_src - 1) / remaining_src;

            let end = (tgt_idx + count).min(tgt_count);
            let group = target[tgt_idx..end].join("\n");

            if !group.is_empty() {
                let confidence = if count == 1 { 0.85 } else { 0.65 };
                result.push(AlignedSegment {
                    source: src.clone(),
                    target: group,
                    confidence,
                });
                tgt_idx = end;
            }
        }
    } else {
        // More source paragraphs than target.
        // Distribute sources proportionally across targets.
        let mut src_idx = 0;
        for (tgt_idx, tgt) in target.iter().enumerate() {
            let remaining_tgt = tgt_count - tgt_idx;
            let remaining_src = src_count - src_idx;
            let count = (remaining_src + remaining_tgt - 1) / remaining_tgt;

            let end = (src_idx + count).min(src_count);
            let group = source[src_idx..end].join("\n");

            if !group.is_empty() {
                let confidence = if count == 1 { 0.85 } else { 0.65 };
                result.push(AlignedSegment {
                    source: group,
                    target: tgt.clone(),
                    confidence,
                });
                src_idx = end;
            }
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
        if ch == '.'
            || ch == '!'
            || ch == '?'
            || ch == '\n'
            || ch == '。'
            || ch == '！'
            || ch == '？'
        {
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
    fn test_align_3_to_5_paragraphs() {
        // 3 source paragraphs, 5 target paragraphs (translator split some)
        let source = "First paragraph.\nSecond paragraph.\nThird paragraph.";
        let target = "第一段。\n第二段A。\n第二段B。\n第三段A。\n第三段B。";
        let result = align_paragraphs(source, target);

        // Should produce 3 aligned segments (one per source paragraph)
        // with 2:2:1 distribution of target paragraphs
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].source, "First paragraph.");
        assert!(result[0].target.contains("第一段"));
        assert_eq!(result[1].source, "Second paragraph.");
        assert!(result[1].target.contains("第二段A"));
        assert!(result[1].target.contains("第二段B"));
        assert_eq!(result[2].source, "Third paragraph.");
        assert!(result[2].target.contains("第三段A"));
    }

    #[test]
    fn test_align_5_to_3_paragraphs() {
        // 5 source paragraphs, 3 target paragraphs
        let source = "P1.\nP2.\nP3.\nP4.\nP5.";
        let target = "T1.\nT2.\nT3.";
        let result = align_paragraphs(source, target);

        // Should produce 3 aligned segments (one per target paragraph)
        // with 2:2:1 distribution of source paragraphs
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].target, "T1.");
        assert!(result[0].source.contains("P1."));
        assert!(result[0].source.contains("P2."));
        assert_eq!(result[1].target, "T2.");
        assert_eq!(result[2].target, "T3.");
    }

    #[test]
    fn test_align_1_to_5_paragraphs() {
        // 1 source paragraph, 5 target paragraphs (ratio 5.0, triggers sentence fallback)
        let source = "Hello. World. How are you?";
        let target = "你好。\n世界。\n你好吗？\n我很好。\n谢谢。";
        let result = align_paragraphs(source, target);

        // Should fall back to sentence-level alignment (3 sentences)
        assert!(!result.is_empty());
        // Each result has source as a sentence, target as a paragraph
        assert_eq!(result.len(), 3);
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

    #[test]
    fn test_align_3_to_5_proportional() {
        // Verify 3:5 produces 2:2:1 distribution
        let source: Vec<String> = (0..3).map(|i| format!("S{}", i)).collect();
        let target: Vec<String> = (0..5).map(|i| format!("T{}", i)).collect();
        let result = align_by_ratio(&source, &target);

        assert_eq!(result.len(), 3);
        // source[0] gets 2 targets (ceil(5/3)=2)
        assert_eq!(result[0].source, "S0");
        assert_eq!(result[0].target, "T0\nT1");
        // source[1] gets 2 targets (ceil(3/2)=2)
        assert_eq!(result[1].source, "S1");
        assert_eq!(result[1].target, "T2\nT3");
        // source[2] gets 1 target (remaining)
        assert_eq!(result[2].source, "S2");
        assert_eq!(result[2].target, "T4");
    }
}
