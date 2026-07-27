use crate::collection::CollectionPushReport;
use crate::memory::WordBookItem;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_wordbook(state: State<'_, AppState>) -> Result<Vec<WordBookItem>, String> {
    let wordbook = state.document.wordbook.lock().await;
    Ok(wordbook.get_all())
}

#[tauri::command]
pub async fn add_wordbook_entry(
    state: State<'_, AppState>,
    word: String,
    translation: String,
    from_lang: String,
    to_lang: String,
    note: Option<String>,
) -> Result<CollectionPushReport, String> {
    let note_str = note.unwrap_or_default();
    {
        let wordbook = state.document.wordbook.lock().await;
        wordbook.add(&word, &translation, &from_lang, &to_lang, &note_str)?;
    }

    // Optional remote collection (Eudic/Anki/Shanbay). Failures must not undo local save.
    let (auto_push, collection) = {
        let cfg = state.system.config.lock().await;
        (cfg.collection.auto_push_on_save, cfg.collection.clone())
    };
    if !auto_push {
        return Ok(CollectionPushReport {
            results: Vec::new(),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let item = crate::collection::CollectionItem {
        word,
        translation,
        note: note_str,
        from_lang,
        to_lang,
    };
    let report = crate::collection::push_enabled(&client, &collection, &item).await;
    for r in &report.results {
        if r.ok {
            tracing::info!("collection {}: {}", r.target, r.message);
        } else {
            tracing::warn!("collection {} failed: {}", r.target, r.message);
        }
    }
    Ok(report)
}

#[tauri::command]
pub async fn update_wordbook_note(
    state: State<'_, AppState>,
    id: String,
    note: String,
) -> Result<(), String> {
    let wordbook = state.document.wordbook.lock().await;
    wordbook.update_note(&id, &note)
}

#[tauri::command]
pub async fn delete_wordbook_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let wordbook = state.document.wordbook.lock().await;
    wordbook.remove(&id);
    Ok(())
}

#[tauri::command]
pub async fn batch_delete_wordbook(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let wordbook = state.document.wordbook.lock().await;
    wordbook.batch_remove(&ids);
    Ok(())
}

#[tauri::command]
pub async fn clear_wordbook(state: State<'_, AppState>) -> Result<(), String> {
    let wordbook = state.document.wordbook.lock().await;
    wordbook.clear();
    Ok(())
}

#[tauri::command]
pub async fn search_wordbook(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<WordBookItem>, String> {
    let wordbook = state.document.wordbook.lock().await;
    if query.trim().is_empty() {
        Ok(wordbook.get_all())
    } else {
        Ok(wordbook.search(&query))
    }
}

#[tauri::command]
pub async fn export_wordbook_csv(state: State<'_, AppState>) -> Result<String, String> {
    let wordbook = state.document.wordbook.lock().await;
    let items = wordbook.get_all();

    let mut csv = String::from("word,translation,from,to,note,timestamp\n");
    for item in items {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            escape_csv(&item.word),
            escape_csv(&item.translation),
            item.from_lang,
            item.to_lang,
            escape_csv(&item.note),
            item.timestamp
        ));
    }

    Ok(csv)
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
