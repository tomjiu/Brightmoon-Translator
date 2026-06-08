use crate::glossary::GlossaryEntry;
use crate::security;
use crate::tbx;
use crate::tmx;
use crate::AppState;
use tauri::command;

#[command]
pub async fn get_glossary(
    state: tauri::State<'_, AppState>,
    lang_pair: String,
) -> Result<Vec<GlossaryEntry>, String> {
    let glossary = state.translation.glossary.lock().await;
    Ok(glossary.get_entries(&lang_pair))
}

#[command]
pub async fn get_all_glossary(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, Vec<GlossaryEntry>>, String> {
    let glossary = state.translation.glossary.lock().await;
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
    let mut glossary = state.translation.glossary.lock().await;
    glossary.add_entry(
        lang_pair,
        GlossaryEntry {
            source,
            target,
            context,
        },
    ).await;
    Ok(())
}

#[command]
pub async fn remove_glossary_entry(
    state: tauri::State<'_, AppState>,
    lang_pair: String,
    source: String,
) -> Result<bool, String> {
    let mut glossary = state.translation.glossary.lock().await;
    Ok(glossary.remove_entry(&lang_pair, &source).await)
}

/// Import glossary entries from TMX file content.
/// Returns (imported_count, skipped_count).
#[command]
pub async fn import_glossary_tmx(
    xml: String,
    state: tauri::State<'_, AppState>,
) -> Result<(usize, usize), String> {
    security::validate_text_length(&xml, 10_000_000)?; // 10MB limit

    let data = tmx::parse_tmx(&xml).map_err(|e| format!("TMX parse error: {}", e))?;

    let mut glossary = state.translation.glossary.lock().await;
    let mut imported = 0;
    let skipped = 0;

    for unit in &data.units {
        let lang_pair = format!("{}-{}", unit.source_lang, unit.target_lang);
        let entry = GlossaryEntry {
            source: unit.source_text.clone(),
            target: unit.target_text.clone(),
            context: unit.note.clone(),
        };
        glossary.add_entry(lang_pair, entry).await;
        imported += 1;
    }

    Ok((imported, skipped))
}

/// Export glossary entries to TMX format.
#[command]
pub async fn export_glossary_tmx(
    lang_pair: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let glossary = state.translation.glossary.lock().await;
    let all_entries = glossary.get_all_entries();

    let mut units = Vec::new();

    for (pair, entries) in all_entries {
        // Filter by lang_pair if specified
        if let Some(ref filter) = lang_pair {
            if pair != filter {
                continue;
            }
        }

        let parts: Vec<&str> = pair.split('-').collect();
        let source_lang = parts.first().copied().unwrap_or("en");
        let target_lang = parts.get(1).copied().unwrap_or("zh");

        for entry in entries {
            units.push(tmx::TmxTranslationUnit {
                source_text: entry.source.clone(),
                target_text: entry.target.clone(),
                source_lang: source_lang.to_string(),
                target_lang: target_lang.to_string(),
                creation_date: None,
                change_date: None,
                creation_user: None,
                note: entry.context.clone(),
            });
        }
    }

    tmx::export_tmx(&units, "en", "MoonTranslator").map_err(|e| format!("TMX export error: {}", e))
}

/// Import glossary entries from TBX file content.
/// Returns (imported_count, skipped_count).
#[command]
pub async fn import_glossary_tbx(
    xml: String,
    state: tauri::State<'_, AppState>,
) -> Result<(usize, usize), String> {
    security::validate_text_length(&xml, 10_000_000)?; // 10MB limit

    let data = tbx::parse_tbx(&xml).map_err(|e| format!("TBX parse error: {}", e))?;

    let mut glossary = state.translation.glossary.lock().await;
    let mut imported = 0;

    for entry in &data.entries {
        let lang_pair = format!("{}-{}", entry.source_lang, entry.target_lang);
        let glossary_entry = GlossaryEntry {
            source: entry.source_term.clone(),
            target: entry.target_term.clone(),
            context: entry.subject_field.clone().or_else(|| entry.note.clone()),
        };
        glossary.add_entry(lang_pair, glossary_entry).await;
        imported += 1;
    }

    Ok((imported, 0))
}

/// Export glossary entries to TBX format.
#[command]
pub async fn export_glossary_tbx(
    lang_pair: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let glossary = state.translation.glossary.lock().await;
    let all_entries = glossary.get_all_entries();

    let mut entries = Vec::new();

    for (pair, pair_entries) in all_entries {
        if let Some(ref filter) = lang_pair {
            if pair != filter {
                continue;
            }
        }

        let parts: Vec<&str> = pair.split('-').collect();
        let source_lang = parts.first().copied().unwrap_or("en");
        let target_lang = parts.get(1).copied().unwrap_or("zh");

        for entry in pair_entries {
            entries.push(tbx::TbxTermEntry {
                source_term: entry.source.clone(),
                target_term: entry.target.clone(),
                source_lang: source_lang.to_string(),
                target_lang: target_lang.to_string(),
                subject_field: None,
                source_definition: None,
                target_definition: None,
                note: entry.context.clone(),
                transaction_type: None,
            });
        }
    }

    tbx::export_tbx(&entries, "en", "zh").map_err(|e| format!("TBX export error: {}", e))
}

/// Align source and translated text at paragraph level.
#[command]
pub async fn align_text(
    source: String,
    target: String,
) -> Result<Vec<crate::alignment::AlignedSegment>, String> {
    security::validate_text_length(&source, 1_000_000)?;
    security::validate_text_length(&target, 1_000_000)?;

    Ok(crate::alignment::align_paragraphs(&source, &target))
}
