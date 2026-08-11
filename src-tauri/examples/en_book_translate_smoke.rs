//! Sample English book paragraphs → zh via Google (no API key).
//!
//! cargo run --manifest-path src-tauri/Cargo.toml --example en_book_translate_smoke -- path1.pdf [path2...]
#![allow(clippy::doc_markdown, clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use moontranslator_lib::engine::google::GoogleEngine;
use moontranslator_lib::engine::TranslationEngine;
use moontranslator_lib::pdf;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: en_book_translate_smoke <pdf>...");
        std::process::exit(2);
    }

    let engine = GoogleEngine::new();
    for path in &args {
        println!("========== {path}");
        match pdf::extract_text_from_pdf(path) {
            Ok(doc) => {
                let full: String = doc
                    .pages
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let samples = pick_english_samples(&full, 3, 180, 420);
                if samples.is_empty() {
                    println!("  (no usable English samples; chars={})", full.len());
                    continue;
                }
                for (i, sample) in samples.iter().enumerate() {
                    println!("\n--- sample {} ({} chars) ---", i + 1, sample.len());
                    println!("EN: {}", one_line(sample, 220));
                    match engine.translate(sample, "en", "zh").await {
                        Ok(zh) => {
                            println!("ZH: {}", one_line(&zh, 280));
                            println!(
                                "  quality_hint: ascii_ratio_src={:.2} cjk_ratio_out={:.2}",
                                ascii_letter_ratio(sample),
                                cjk_ratio(&zh)
                            );
                        }
                        Err(e) => println!("ZH: FAIL {e}"),
                    }
                }
            }
            Err(e) => println!("  extract FAIL: {e}"),
        }
        println!();
    }
}

fn pick_english_samples(full: &str, n: usize, min_len: usize, max_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    // Prefer paragraph-ish chunks
    for para in full.split(['\n', '\u{0c}']) {
        let t = para.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.len() < min_len {
            continue;
        }
        if ascii_letter_ratio(&t) < 0.55 {
            continue;
        }
        // skip pure TOC / page numbers
        let digit_ratio = t.chars().filter(char::is_ascii_digit).count() as f64 / t.len() as f64;
        if digit_ratio > 0.25 {
            continue;
        }
        let chunk = if t.len() > max_len {
            // cut at sentence end if possible
            let slice = &t[..max_len];
            if let Some(pos) = slice.rfind(['.', '!', '?']) {
                slice[..=pos].trim().to_string()
            } else {
                slice.trim().to_string()
            }
        } else {
            t
        };
        if chunk.len() >= min_len {
            out.push(chunk);
        }
        if out.len() >= n {
            break;
        }
    }
    // fallback: sliding from start
    if out.is_empty() && full.len() > min_len {
        let cleaned = full.split_whitespace().collect::<Vec<_>>().join(" ");
        if cleaned.len() >= min_len {
            out.push(cleaned.chars().take(max_len).collect());
        }
    }
    out
}

fn one_line(s: &str, max: usize) -> String {
    let t = s.replace(['\n', '\r'], " ");
    if t.chars().count() <= max {
        t
    } else {
        let mut o: String = t.chars().take(max).collect();
        o.push('…');
        o
    }
}

fn ascii_letter_ratio(s: &str) -> f64 {
    let letters = s.chars().filter(char::is_ascii_alphabetic).count();
    let total = s.chars().filter(|c| !c.is_whitespace()).count().max(1);
    letters as f64 / total as f64
}

fn cjk_ratio(s: &str) -> f64 {
    let cjk = s
        .chars()
        .filter(|c| {
            let u = *c as u32;
            (0x4E00..=0x9FFF).contains(&u) || (0x3400..=0x4DBF).contains(&u)
        })
        .count();
    let total = s.chars().filter(|c| !c.is_whitespace()).count().max(1);
    cjk as f64 / total as f64
}
