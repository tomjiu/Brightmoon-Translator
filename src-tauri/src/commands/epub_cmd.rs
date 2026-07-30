use crate::epub_reader::{self, EpubDocument, TranslatedChapter, TranslatedEpub};
use crate::security;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn open_epub(file_path: String) -> Result<EpubDocument, String> {
    security::validate_file_path(&file_path)?;
    epub_reader::extract_text_from_epub(&file_path)
}

#[tauri::command]
pub async fn translate_epub(
    state: State<'_, AppState>,
    file_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedEpub, String> {
    security::validate_file_path(&file_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    let doc = epub_reader::extract_text_from_epub(&file_path)?;

    // Collect non-empty chapters for batch translation
    let chapters_to_translate: Vec<(usize, &str)> = doc
        .chapters
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.text.trim().is_empty())
        .map(|(i, c)| (i, c.text.trim()))
        .collect();

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .run_batch(
            crate::models::translation::TranslateChannel::Document,
            &chapters_to_translate,
            &from_lang,
            &to_lang,
            2,
        )
        .await;

    // Build translated chapters
    let mut translated_chapters: Vec<TranslatedChapter> = doc
        .chapters
        .iter()
        .map(|c| TranslatedChapter {
            chapter_number: c.chapter_number,
            title: c.title.clone(),
            original_text: c.text.clone(),
            translated_text: String::new(),
        })
        .collect();

    // Apply results
    for result in batch_results {
        if let Some(chapter) = translated_chapters.get_mut(result.index) {
            chapter.translated_text = result.translated;
        }
    }

    Ok(TranslatedEpub {
        title: doc.title,
        chapters: translated_chapters,
        total_chapters: doc.total_chapters,
    })
}

/// Save a bilingual EPUB: re-opens the original EPUB, injects translated text
/// into each chapter's HTML while preserving the original formatting, and writes
/// a new .epub file that can be opened in any EPUB reader.
#[tauri::command]
pub async fn save_bilingual_epub(
    original_path: String,
    output_path: String,
    translated_chapters: Vec<TranslatedChapter>,
) -> Result<(), String> {
    security::validate_file_path(&original_path)?;
    security::validate_output_path(&output_path)?;

    // Re-open the original EPUB to get chapter HTML content
    let doc = epub_reader::extract_text_from_epub(&original_path)?;

    // Run EPUB creation in a blocking thread since it does file I/O
    let orig = original_path.clone();
    let out = output_path.clone();
    let chs = translated_chapters;
    tokio::task::spawn_blocking(move || {
        epub_reader::create_bilingual_epub(&orig, &out, &chs, &doc.chapters)
    })
    .await
    .map_err(|e| format!("EPUB creation join error: {}", e))?
}
