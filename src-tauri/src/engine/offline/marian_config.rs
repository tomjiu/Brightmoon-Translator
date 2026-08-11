//! Generate the marian config text for a downloaded model pair.
//!
//! Firefox CJK models ship without a marian config.yml (the registry only
//! provides model/vocab/lex files). Bergamot resolves model/vocab/shortlist
//! paths relative to the config file's directory, so we emit plain file names.

use crate::engine::offline::model_catalog::ModelSpec;

/// Split a `.gz` file name into the bare name marian loads.
fn bare(name: &str) -> String {
    name.strip_suffix(".gz").unwrap_or(name).to_string()
}

fn first(spec: &ModelSpec, prefix: &str) -> String {
    spec.files
        .iter()
        .find(|f| f.name.starts_with(prefix))
        .map(|f| bare(&f.name))
        .unwrap_or_default()
}

/// Resolve the two vocab file names for a pair.
///
/// en-zh / en-ja use separate src/trg vocabs (`srcvocab.*.spm.gz`,
/// `trgvocab.*.spm.gz`); zh-en / ja-en use a single shared `vocab.*.spm.gz`
/// referenced twice (source and target vocab are the same file).
fn vocabs(spec: &ModelSpec) -> (String, String) {
    let src = first(spec, "srcvocab.");
    let trg = first(spec, "trgvocab.");
    if !src.is_empty() && !trg.is_empty() {
        (src, trg)
    } else {
        let single = first(spec, "vocab.");
        (single.clone(), single)
    }
}

/// Build the marian config text for a model pair. Safe to write to
/// `<model_dir>/<from>-<to>/config.yml`.
pub fn build_config(spec: &ModelSpec) -> String {
    let model = first(spec, "model.");
    let lex = first(spec, "lex.");
    let (srcv, trgv) = vocabs(spec);

    format!(
        "models:\n  - {model}\nvocabs:\n  - {srcv}\n  - {trgv}\nshortlist:\n  - {lex}\nssplit-mode: paragraph\nmax-length-break: 128\nmini-batch-words: 1024\nalignment: soft\nmax-length-factor: 2.0\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::offline::model_catalog::model_spec;

    #[test]
    fn en_zh_config_lists_three_asset_files() {
        let spec = model_spec("en", "zh").unwrap();
        let cfg = build_config(&spec);
        assert!(cfg.contains("model.enzh.intgemm.alphas.bin"));
        assert!(cfg.contains("srcvocab.enzh.spm"));
        assert!(cfg.contains("trgvocab.enzh.spm"));
        assert!(cfg.contains("lex.50.50.enzh.s2t.bin"));
        assert!(!cfg.contains(".gz"));
        assert!(cfg.contains("mini-batch-words: 1024"));
    }

    #[test]
    fn single_vocab_pair_references_it_twice() {
        let spec = model_spec("zh", "en").unwrap();
        let cfg = build_config(&spec);
        let count = cfg.matches("vocab.zhen.spm").count();
        assert_eq!(count, 2, "single-vocab pairs must list the vocab twice");
        assert!(!cfg.contains("srcvocab"));
        assert!(!cfg.contains("trgvocab"));
    }

    #[test]
    fn ja_en_uses_single_vocab_too() {
        let spec = model_spec("ja", "en").unwrap();
        let cfg = build_config(&spec);
        assert_eq!(cfg.matches("vocab.jaen.spm").count(), 2);
    }
}
