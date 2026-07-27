//! External vocabulary collection adapters (pot/STranslate collection as first-party).

pub mod anki;
pub mod eudic;
pub mod maimemo;
pub mod shanbay;
pub mod youdao;

use crate::models::config::CollectionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItem {
    pub word: String,
    pub translation: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub from_lang: String,
    #[serde(default)]
    pub to_lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionTargetResult {
    pub target: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPushReport {
    pub results: Vec<CollectionTargetResult>,
}

pub async fn push_enabled(
    client: &reqwest::Client,
    cfg: &CollectionConfig,
    item: &CollectionItem,
) -> CollectionPushReport {
    let mut results = Vec::new();
    if cfg.eudic.enabled {
        results.push(eudic::push(client, &cfg.eudic, item).await);
    }
    if cfg.anki.enabled {
        results.push(anki::push(client, &cfg.anki, item).await);
    }
    if cfg.shanbay.enabled {
        results.push(shanbay::push(client, &cfg.shanbay, item).await);
    }
    if cfg.youdao.enabled {
        results.push(youdao::push(client, &cfg.youdao, item).await);
    }
    if cfg.maimemo.enabled {
        results.push(maimemo::push(client, &cfg.maimemo, item).await);
    }
    CollectionPushReport { results }
}

/// Push to a single named target (`eudic` | `anki` | `shanbay` | `youdao` | `maimemo`).
pub async fn push_target(
    client: &reqwest::Client,
    cfg: &CollectionConfig,
    target: &str,
    item: &CollectionItem,
) -> CollectionPushReport {
    let result = match target {
        "eudic" => eudic::push(client, &cfg.eudic, item).await,
        "anki" => anki::push(client, &cfg.anki, item).await,
        "shanbay" => shanbay::push(client, &cfg.shanbay, item).await,
        "youdao" => youdao::push(client, &cfg.youdao, item).await,
        "maimemo" => maimemo::push(client, &cfg.maimemo, item).await,
        other => CollectionTargetResult {
            target: other.to_string(),
            ok: false,
            message: format!("Unknown collection target: {other}"),
        },
    };
    CollectionPushReport {
        results: vec![result],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_pushes_nothing() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let report = rt.block_on(push_enabled(
            &reqwest::Client::new(),
            &CollectionConfig::default(),
            &CollectionItem {
                word: "hello".into(),
                translation: "你好".into(),
                note: String::new(),
                from_lang: "en".into(),
                to_lang: "zh".into(),
            },
        ));
        assert!(report.results.is_empty());
    }
}
