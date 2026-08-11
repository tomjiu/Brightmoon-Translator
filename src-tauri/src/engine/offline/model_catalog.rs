//! Data-driven model catalog backed by the Mozilla Firefox Model Registry.
//!
//! The registry (<https://storage.googleapis.com/moz-fx-translations-data--303e-prod-translations-data/db/models.json>)
//! is English-centric: only `X->en` and `en->X` pairs exist. Chinese has direct
//! en-zh / zh-en; other CJK/ru/ko pairs reach Chinese only via an English pivot.
//! Entries below are frozen from the Task 1 spike (SHA-256 verified, see
//! `spike/VERDICT.md`). New pairs must be added only after downloading and
//! verifying the model hash against the registry.

use serde::{Deserialize, Serialize};

const REGISTRY_BASE: &str =
    "https://storage.googleapis.com/moz-fx-translations-data--303e-prod-translations-data";

/// One downloadable file of a model (mirrors the registry `files` object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    /// File name as stored on disk (still `.gz`; decompressed on download).
    pub name: String,
    /// Registry-relative path.
    pub path: String,
    /// Full download URL.
    pub url: String,
    /// Approximate uncompressed size in bytes (used for progress when the
    /// server omits Content-Length). 0 = unknown.
    pub size_bytes: u64,
}

/// A downloadable language-pair model. Keyed by `id` = "{from}-{to}".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub from: String,
    pub to: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub size_label: String,
    /// SHA-256 of the *uncompressed* model binary (from the registry).
    pub sha256: String,
    pub files: Vec<ModelFile>,
    pub release_status: String,
}

impl ModelSpec {
    #[allow(clippy::cast_precision_loss)]
    fn size_label(bytes: u64) -> String {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        if mb >= 1024.0 {
            format!("{:.1}GB", mb / 1024.0)
        } else {
            format!("{mb:.0}MB")
        }
    }

    /// Total uncompressed size of all files.
    fn total_size(files: &[ModelFile]) -> u64 {
        files.iter().map(|f| f.size_bytes).sum()
    }
}

fn file(dir: &str, name: &str, size: u64) -> ModelFile {
    let path = format!("models/{dir}/exported/{name}");
    ModelFile {
        name: name.to_string(),
        url: format!("{REGISTRY_BASE}/{path}"),
        path,
        size_bytes: size,
    }
}

fn spec(id: &str, from: &str, to: &str, display: &str, _dir: &str, sha256: &str, release: &str, files: Vec<ModelFile>) -> ModelSpec {
    let total = ModelSpec::total_size(&files);
    ModelSpec {
        id: id.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        display_name: display.to_string(),
        size_bytes: total,
        size_label: ModelSpec::size_label(total),
        sha256: sha256.to_string(),
        files,
        release_status: release.to_string(),
    }
}

/// All registry entries. Frozen from the Task 1 spike + registry (2026-08-10).
#[allow(clippy::vec_init_then_push)]
pub fn registry_entries() -> Vec<ModelSpec> {
    // dir, model name, vocab name(s), lex name, uncompressed model hash
    let mut out = Vec::new();

    // en->zh (Desktop Release, arch=base-memory) — spike verified
    out.push(spec(
        "en-zh", "en", "zh", "English → Chinese",
        "en-zh/llmaat_finetune10M_qe8_f2_ByQcSxGXQRqGi-UTxYE43g",
        "4e5accc141373565ddc8fa1565bceaa8d0c3482a82cab8131c719ebcc6c2157c",
        "Release",
        vec![
            file("en-zh/llmaat_finetune10M_qe8_f2_ByQcSxGXQRqGi-UTxYE43g", "model.enzh.intgemm.alphas.bin.gz", 43_850_000),
            file("en-zh/llmaat_finetune10M_qe8_f2_ByQcSxGXQRqGi-UTxYE43g", "srcvocab.enzh.spm.gz", 800_000),
            file("en-zh/llmaat_finetune10M_qe8_f2_ByQcSxGXQRqGi-UTxYE43g", "trgvocab.enzh.spm.gz", 850_000),
            file("en-zh/llmaat_finetune10M_qe8_f2_ByQcSxGXQRqGi-UTxYE43g", "lex.50.50.enzh.s2t.bin.gz", 4_500_000),
        ],
    ));

    // zh->en (base, cjk_icu_base_LQeOIbF7…) — spike verified
    out.push(spec(
        "zh-en", "zh", "en", "Chinese → English",
        "zh-en/cjk_icu_base_LQeOIbF7Sbq3XA8lsRPotw",
        "3535442962ec8f4a553cc19b206befcac689ee9cddaea44fa91e21527fc30ac2",
        "Release",
        vec![
            file("zh-en/cjk_icu_base_LQeOIbF7Sbq3XA8lsRPotw", "model.zhen.intgemm.alphas.bin.gz", 59_500_000),
            file("zh-en/cjk_icu_base_LQeOIbF7Sbq3XA8lsRPotw", "vocab.zhen.spm.gz", 1_400_000),
            file("zh-en/cjk_icu_base_LQeOIbF7Sbq3XA8lsRPotw", "lex.50.50.zhen.s2t.bin.gz", 9_200_000),
        ],
    ));

    // en->ja (Desktop Release, base-memory) — spike verified
    out.push(spec(
        "en-ja", "en", "ja", "English → Japanese",
        "en-ja/llmaat_finetune10M_qe8_f2_ApiGGQIwTKuF9i_k3n9Q2Q",
        "59ae659f9bb63e4f81f474fe3c03d3f4499434b5f9e779fab7c12a45f31fd562",
        "Release",
        vec![
            file("en-ja/llmaat_finetune10M_qe8_f2_ApiGGQIwTKuF9i_k3n9Q2Q", "model.enja.intgemm.alphas.bin.gz", 43_850_000),
            file("en-ja/llmaat_finetune10M_qe8_f2_ApiGGQIwTKuF9i_k3n9Q2Q", "srcvocab.enja.spm.gz", 800_000),
            file("en-ja/llmaat_finetune10M_qe8_f2_ApiGGQIwTKuF9i_k3n9Q2Q", "trgvocab.enja.spm.gz", 850_000),
            file("en-ja/llmaat_finetune10M_qe8_f2_ApiGGQIwTKuF9i_k3n9Q2Q", "lex.50.50.enja.s2t.bin.gz", 4_500_000),
        ],
    ));

    // ja->en (Desktop Release, base) — spike verified
    out.push(spec(
        "ja-en", "ja", "en", "Japanese → English",
        "ja-en/cjk_icu_base_U4VUAW3STh-bF0Sr-dX69g",
        "a9bf800679bba570520e1161d7b4fbfcb957add32ca35812134add85689752ad",
        "Release Desktop",
        vec![
            file("ja-en/cjk_icu_base_U4VUAW3STh-bF0Sr-dX69g", "model.jaen.intgemm.alphas.bin.gz", 59_500_000),
            file("ja-en/cjk_icu_base_U4VUAW3STh-bF0Sr-dX69g", "vocab.jaen.spm.gz", 1_400_000),
            file("ja-en/cjk_icu_base_U4VUAW3STh-bF0Sr-dX69g", "lex.50.50.jaen.s2t.bin.gz", 9_300_000),
        ],
    ));

    // ko->en (Desktop Release, base)
    out.push(spec(
        "ko-en", "ko", "en", "Korean → English",
        "ko-en/cjk_icu_base_BnKgBdd0Rzq87oUYN3L9-A",
        "1c902d6f7a8d7e3efe6ff4f7d4960a369957bca4ce2ce4a6e8572c231d525090",
        "Release Desktop",
        vec![
            file("ko-en/cjk_icu_base_BnKgBdd0Rzq87oUYN3L9-A", "model.koen.intgemm.alphas.bin.gz", 59_500_000),
            file("ko-en/cjk_icu_base_BnKgBdd0Rzq87oUYN3L9-A", "vocab.koen.spm.gz", 1_400_000),
            file("ko-en/cjk_icu_base_BnKgBdd0Rzq87oUYN3L9-A", "lex.50.50.koen.s2t.bin.gz", 9_300_000),
        ],
    ));

    // en->ko (Desktop Release, base)
    out.push(spec(
        "en-ko", "en", "ko", "English → Korean",
        "en-ko/cjk_hplt2_Fzrv_XPwTs6KVNkktUeuOA",
        "1c310a79b61b8824b2eb26b045db043d92722f3e66ea06998f7c89f48da9f6bc",
        "Release Desktop",
        vec![
            file("en-ko/cjk_hplt2_Fzrv_XPwTs6KVNkktUeuOA", "model.enko.intgemm.alphas.bin.gz", 43_850_000),
            file("en-ko/cjk_hplt2_Fzrv_XPwTs6KVNkktUeuOA", "vocab.enko.spm.gz", 800_000),
            file("en-ko/cjk_hplt2_Fzrv_XPwTs6KVNkktUeuOA", "lex.50.50.enko.s2t.bin.gz", 4_500_000),
        ],
    ));

    // ru->en (only available: tiny/Release)
    out.push(spec(
        "ru-en", "ru", "en", "Russian → English",
        "ru-en/spring-2024_QrcdYgbwS7e7xbhtOSdoNQ",
        "b1d85c13cfbb05e1d326dd6f0fb5ef270a2011b547450260f96567a93f446c94",
        "Release",
        vec![
            file("ru-en/spring-2024_QrcdYgbwS7e7xbhtOSdoNQ", "model.ruen.intgemm.alphas.bin.gz", 25_500_000),
            file("ru-en/spring-2024_QrcdYgbwS7e7xbhtOSdoNQ", "vocab.ruen.spm.gz", 800_000),
            file("ru-en/spring-2024_QrcdYgbwS7e7xbhtOSdoNQ", "lex.50.50.ruen.s2t.bin.gz", 2_500_000),
        ],
    ));

    // en->ru (Desktop Release, base)
    out.push(spec(
        "en-ru", "en", "ru", "English → Russian",
        "en-ru/student_base_AYqN3ysXRp2EGkEqeaA5Rg",
        "0ef9a209c5edc46692750e7505b3695655b1c7c3ec73058b641201ef18c481ce",
        "Release Desktop",
        vec![
            file("en-ru/student_base_AYqN3ysXRp2EGkEqeaA5Rg", "model.enru.intgemm.alphas.bin.gz", 43_850_000),
            file("en-ru/student_base_AYqN3ysXRp2EGkEqeaA5Rg", "vocab.enru.spm.gz", 800_000),
            file("en-ru/student_base_AYqN3ysXRp2EGkEqeaA5Rg", "lex.50.50.enru.s2t.bin.gz", 4_500_000),
        ],
    ));

    out
}

/// Direct model pair lookup.
pub fn model_spec(from: &str, to: &str) -> Option<ModelSpec> {
    registry_entries()
        .into_iter()
        .find(|s| s.from == from && s.to == to)
}

/// Look up a model spec by pair id ("en-zh").
pub fn model_spec_by_id(id: &str) -> Option<ModelSpec> {
    registry_entries().into_iter().find(|s| s.id == id)
}

/// Resolve a `from -> to` translation chain as a list of pair ids.
///
/// The Firefox registry is English-centric, so any pair that has no direct
/// model (e.g. ja->zh) pivots through English: `X->en` then `en->Y`.
pub fn translation_chain(from: &str, to: &str) -> Option<Vec<String>> {
    if model_spec(from, to).is_some() {
        return Some(vec![format!("{from}-{to}")]);
    }
    if model_spec(from, "en").is_some() && model_spec("en", to).is_some() {
        return Some(vec![format!("{from}-en"), format!("en-{to}")]);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_zh_and_zh_en_entries_exist() {
        assert!(model_spec("en", "zh").is_some());
        assert!(model_spec("zh", "en").is_some());
    }

    #[test]
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn entry_points_at_firefox_registry_with_sha256() {
        let spec = model_spec("en", "zh").unwrap();
        assert!(spec.files[0].url.starts_with(REGISTRY_BASE));
        assert_eq!(spec.sha256.len(), 64);
        assert!(spec.size_bytes > 30_000_000);
        assert_eq!(spec.files.len(), 4);
        assert!(spec.files.iter().all(|f| f.name.ends_with(".gz")));
    }

    #[test]
    fn pivot_pairs_resolve_through_english() {
        let chain = translation_chain("ja", "zh");
        assert_eq!(chain, Some(vec!["ja-en".to_string(), "en-zh".to_string()]));
    }

    #[test]
    fn direct_pair_is_single_step_chain() {
        assert_eq!(translation_chain("en", "zh"), Some(vec!["en-zh".to_string()]));
    }

    #[test]
    fn unsupported_pair_has_no_chain() {
        assert_eq!(translation_chain("xx", "zz"), None);
    }

    #[test]
    fn size_labels_are_human_readable() {
        assert!(ModelSpec::size_label(50_000_000).contains("MB"));
        assert!(ModelSpec::size_label(2_000_000_000).contains("GB"));
    }

    #[test]
    fn all_catalog_hashes_are_64_hex() {
        for spec in registry_entries() {
            assert_eq!(spec.sha256.len(), 64, "bad hash for {}", spec.id);
            assert!(spec.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(!spec.files.is_empty());
            assert!(spec.files.iter().all(|f| f.url.starts_with(REGISTRY_BASE)));
        }
    }
}
