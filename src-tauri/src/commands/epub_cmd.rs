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

    let mut translated_chapters = Vec::new();

    for chapter in &doc.chapters {
        if chapter.text.trim().is_empty() {
            translated_chapters.push(TranslatedChapter {
                chapter_number: chapter.chapter_number,
                title: chapter.title.clone(),
                original_text: chapter.text.clone(),
                translated_text: String::new(),
            });
            continue;
        }

        // Translate using primary engine
        let router = state.engine_router.read().await;
        let translated = router
            .translate_primary(&chapter.text, &from_lang, &to_lang)
            .await
            .unwrap_or_else(|e| {
                eprintln!("Failed to translate chapter {}: {}", chapter.chapter_number, e);
                String::new()
            });
        drop(router);

        translated_chapters.push(TranslatedChapter {
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
            original_text: chapter.text.clone(),
            translated_text: translated,
        });
    }

    Ok(TranslatedEpub {
        title: doc.title,
        chapters: translated_chapters,
        total_chapters: doc.total_chapters,
    })
}
