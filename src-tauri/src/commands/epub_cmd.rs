use crate::epub_reader::{self, EpubDocument, TranslatedChapter, TranslatedEpub};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn open_epub(file_path: String) -> Result<EpubDocument, String> {
    epub_reader::extract_text_from_epub(&file_path)
}

#[tauri::command]
pub async fn translate_epub(
    state: State<'_, AppState>,
    file_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedEpub, String> {
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
        .translation_service
        .translate_batch(&chapters_to_translate, &from_lang, &to_lang, 2)
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
