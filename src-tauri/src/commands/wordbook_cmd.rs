use crate::memory::WordBookItem;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_wordbook(state: State<'_, AppState>) -> Result<Vec<WordBookItem>, String> {
    let wordbook = state.wordbook.lock().await;
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
) -> Result<(), String> {
    let wordbook = state.wordbook.lock().await;
    wordbook.add(&word, &translation, &from_lang, &to_lang, note.as_deref().unwrap_or(""))
}

#[tauri::command]
pub async fn update_wordbook_note(
    state: State<'_, AppState>,
    id: String,
    note: String,
) -> Result<(), String> {
    let wordbook = state.wordbook.lock().await;
    wordbook.update_note(&id, &note)
}

#[tauri::command]
pub async fn delete_wordbook_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let wordbook = state.wordbook.lock().await;
    wordbook.remove(&id);
    Ok(())
}

#[tauri::command]
pub async fn batch_delete_wordbook(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    let wordbook = state.wordbook.lock().await;
    wordbook.batch_remove(&ids);
    Ok(())
}

#[tauri::command]
pub async fn clear_wordbook(state: State<'_, AppState>) -> Result<(), String> {
    let wordbook = state.wordbook.lock().await;
    wordbook.clear();
    Ok(())
}

#[tauri::command]
pub async fn search_wordbook(state: State<'_, AppState>, query: String) -> Result<Vec<WordBookItem>, String> {
    let wordbook = state.wordbook.lock().await;
    if query.trim().is_empty() {
        Ok(wordbook.get_all())
    } else {
        Ok(wordbook.search(&query))
    }
}

#[tauri::command]
pub async fn export_wordbook_csv(state: State<'_, AppState>) -> Result<String, String> {
    let wordbook = state.wordbook.lock().await;
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
