//! Manual end-to-end smoke for the offline (bergamot) translation engine.
//!
//! Run with: `cargo test --test offline_smoke -- --ignored --nocapture`
//!
//! Requires: reachable Firefox Model Registry (network) + the native bergamot
//! libs built under `src-tauri/native/lib/` (otherwise `load_model` errors).

use moontranslator_lib::engine::offline::OfflineEngine;
use moontranslator_lib::engine::TranslationEngine;

#[tokio::test]
#[ignore = "manual E2E smoke: downloads ~50MB and needs native bergamot libs"]
async fn offline_end_to_end_smoke() {
    let base = std::env::temp_dir().join("mt-offline-smoke");

    let engine = OfflineEngine::new(base.to_str());
    assert!(!engine.available_pairs().await.contains(&"en-zh".to_string()));

    let t0 = std::time::Instant::now();
    engine
        .download_model("en-zh", None::<fn(moontranslator_lib::engine::offline::DownloadProgress)>)
        .await
        .expect("download en-zh pair");
    eprintln!("[smoke] download en-zh: {:?}", t0.elapsed());
    assert!(engine.is_model_downloaded("en", "zh"));

    let t1 = std::time::Instant::now();
    let out = engine
        .translate("The weather is nice today.", "en", "zh")
        .await
        .expect("translate en->zh");
    let elapsed = t1.elapsed();
    eprintln!("[smoke] translate en->zh ({elapsed:?}): {out}");
    assert!(!out.trim().is_empty(), "translation was empty");
    assert!(
        !out.to_lowercase().contains("the weather"),
        "output looks like the untranslated source: {out}"
    );

    eprintln!(
        "[smoke] OK — model dir {}",
        engine.model_dir().display()
    );
}