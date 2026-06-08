/**
 * Translation Quality Commands
 *
 * Tauri commands for translation quality scoring and engine comparison
 */

use crate::quality::{EngineScore, TranslationScore};
use crate::AppState;
use tauri::State;

/// Score a translation result
#[tauri::command]
pub async fn score_translation(
    original: String,
    translated: String,
    lang_pair: String,
    state: State<'_, AppState>,
) -> Result<TranslationScore, String> {
    if original.is_empty() || translated.is_empty() {
        return Err("Original and translated text must not be empty".to_string());
    }

    let glossary = state.translation.glossary.lock().await;
    let glossary_entries = glossary.get_all_entries();

    let score = crate::quality::score_translation(
        &original,
        &translated,
        &lang_pair,
        Some(glossary_entries),
    );

    Ok(score)
}

/// Compare translation quality across multiple engines
#[tauri::command]
pub async fn compare_engine_quality(
    original: String,
    lang_pair: String,
    state: State<'_, AppState>,
) -> Result<Vec<EngineScore>, String> {
    if original.is_empty() {
        return Err("Original text must not be empty".to_string());
    }

    // Parse lang_pair
    let parts: Vec<&str> = lang_pair.split('-').collect();
    if parts.len() != 2 {
        return Err("Invalid lang_pair format, expected 'from-to'".to_string());
    }
    let from = parts[0];
    let to = parts[1];

    // Get translations from all engines
    let router = state.translation.engine_router.read().await;
    let response = router.translate_parallel_compare(&original, from, to).await;
    drop(router);

    if response.results.is_empty() {
        return Err("No engines returned results".to_string());
    }

    // Get glossary
    let glossary = state.translation.glossary.lock().await;
    let glossary_entries = glossary.get_all_entries();

    // Score each engine's result
    let mut engine_scores: Vec<EngineScore> = response
        .results
        .iter()
        .map(|result| {
            let score = crate::quality::score_translation(
                &original,
                &result.text,
                &lang_pair,
                Some(glossary_entries),
            );
            EngineScore {
                engine: result.engine.clone(),
                translated: result.text.clone(),
                score,
            }
        })
        .collect();

    // Sort by overall score (highest first)
    engine_scores.sort_by(|a, b| {
        b.score
            .overall
            .partial_cmp(&a.score.overall)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(engine_scores)
}
