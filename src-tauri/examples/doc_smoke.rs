//! Document extract smoke against real files (e.g. E:\book).
//!
//! Usage:
//!   cargo run --manifest-path src-tauri/Cargo.toml --example doc_smoke -- "E:\book\foo.pdf" "E:\book\bar.epub"
#![allow(clippy::doc_markdown, clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

#[allow(clippy::case_sensitive_file_extension_comparisons)] // 已 to_ascii_lowercase 规范化
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: doc_smoke <file> [file...]");
        std::process::exit(2);
    }

    let mut failed = 0usize;
    for path in &args {
        let lower = path.to_ascii_lowercase();
        print!("SMOKE {path} ... ");
        let result = if lower.ends_with(".pdf") {
            moontranslator_lib::pdf::extract_text_from_pdf(path).map(|d| {
                format!(
                    "pdf pages={} scanned={} chars={}",
                    d.total_pages,
                    d.is_scanned,
                    d.pages.iter().map(|p| p.text.len()).sum::<usize>()
                )
            })
        } else if lower.ends_with(".epub") {
            moontranslator_lib::epub_reader::extract_text_from_epub(path).map(|d| {
                format!(
                    "epub title={:?} chapters={} chars={}",
                    d.title,
                    d.total_chapters,
                    d.chapters.iter().map(|c| c.text.len()).sum::<usize>()
                )
            })
        } else if lower.ends_with(".docx") {
            moontranslator_lib::docx::extract_text_from_docx(path).map(|d| {
                format!(
                    "docx title={:?} paras={} words={}",
                    d.title, d.total_paragraphs, d.total_words
                )
            })
        } else if lower.ends_with(".pptx") {
            moontranslator_lib::pptx::extract_text_from_pptx(path).map(|d| {
                format!(
                    "pptx slides={} blocks={}",
                    d.total_slides,
                    d.slides.iter().map(|s| s.text_blocks.len()).sum::<usize>()
                )
            })
        } else if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
            moontranslator_lib::excel::extract_text_from_excel(path).map(|d| {
                format!(
                    "excel sheets={} cells={} words={}",
                    d.total_sheets, d.total_cells, d.total_words
                )
            })
        } else if lower.ends_with(".srt")
            || lower.ends_with(".ass")
            || lower.ends_with(".ssa")
            || lower.ends_with(".vtt")
            || lower.ends_with(".lrc")
        {
            moontranslator_lib::subtitle::extract_text_from_subtitle(path).map(|d| {
                format!(
                    "subtitle format={} entries={}",
                    d.format, d.total_entries
                )
            })
        } else {
            Err(format!("unsupported extension: {path}"))
        };

        match result {
            Ok(summary) => println!("OK {summary}"),
            Err(e) => {
                println!("FAIL {e}");
                failed += 1;
            }
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
}
