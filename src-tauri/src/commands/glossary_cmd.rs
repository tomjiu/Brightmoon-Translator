use crate::glossary::GlossaryEntry;
use crate::AppState;
use tauri::command;

#[command]
pub async fn get_glossary(
    state: tauri::State<'_, AppState>,
    lang_pair: String,
) -> Result<Vec<GlossaryEntry>, String> {
    let glossary = state.glossary.lock().await;
    Ok(glossary.get_entries(&lang_pair))
}

#[command]
pub async fn get_all_glossary(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, Vec<GlossaryEntry>>, String> {
    let glossary = state.glossary.lock().await;
    Ok(glossary.get_all_entries().clone())
}

#[command]
pub async fn add_glossary_entry(
    state: tauri::State<'_, AppState>,
    lang_pair: String,
    source: String,
    target: String,
    context: Option<String>,
) -> Result<(), String> {
    let mut glossary = state.glossary.lock().await;
    glossary.add_entry(lang_pair, GlossaryEntry {
        source,
        target,
        context,
    });
    Ok(())
}

#[command]
pub async fn remove_glossary_entry(
    state: tauri::State<'_, AppState>,
    lang_pair: String,
    source: String,
) -> Result<bool, String> {
    let mut glossary = state.glossary.lock().await;
    Ok(glossary.remove_entry(&lang_pair, &source))
}
