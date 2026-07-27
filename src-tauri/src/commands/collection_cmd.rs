use crate::collection::{push_enabled, push_target, CollectionItem, CollectionPushReport};
use crate::AppState;
use tauri::State;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[tauri::command]
pub async fn collection_push(
    state: State<'_, AppState>,
    word: String,
    translation: String,
    note: Option<String>,
    from_lang: Option<String>,
    to_lang: Option<String>,
) -> Result<CollectionPushReport, String> {
    let collection = {
        let cfg = state.system.config.lock().await;
        cfg.collection.clone()
    };
    let item = CollectionItem {
        word,
        translation,
        note: note.unwrap_or_default(),
        from_lang: from_lang.unwrap_or_else(|| "en".into()),
        to_lang: to_lang.unwrap_or_else(|| "zh".into()),
    };
    Ok(push_enabled(&http_client(), &collection, &item).await)
}

#[tauri::command]
pub async fn collection_test_target(
    state: State<'_, AppState>,
    target: String,
) -> Result<CollectionPushReport, String> {
    let collection = {
        let cfg = state.system.config.lock().await;
        cfg.collection.clone()
    };
    let item = CollectionItem {
        word: "__moon_collection_test__".into(),
        translation: "Moon collection connectivity probe".into(),
        note: String::new(),
        from_lang: "en".into(),
        to_lang: "zh".into(),
    };
    Ok(push_target(&http_client(), &collection, target.trim(), &item).await)
}
