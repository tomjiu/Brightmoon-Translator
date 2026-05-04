use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

// ============================================================
// Key structures
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyEntry {
    k: String,
    #[serde(default)]
    id: String,
}

// ============================================================
// Key store (persistent)
// ============================================================

fn keys_cache_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("moontranslator");
    std::fs::create_dir_all(&path).ok();
    path.push("youdao_keys.json");
    path
}

fn default_keys() -> HashMap<String, KeyEntry> {
    let mut m = HashMap::new();
    // AI翻译链
    m.insert("ai_pre".into(), KeyEntry { k: "EZAmCfVOH2CrBGMtPrtIPUzyv3bheLdk".into(), id: "ai-translate-llm-pre".into() });
    m.insert("ai_translate".into(), KeyEntry { k: "LqMQV3ZdE2X6DYYyc6TNsVbHgCGk7XzG".into(), id: "ai-translate-llm".into() });
    m.insert("ai_write".into(), KeyEntry { k: "xuiC95RuooxC8Q51UJtdod1plLUhdAmt".into(), id: "ai-write".into() });
    m.insert("ai_direction".into(), KeyEntry { k: "I5WacgKEZaloWBiDnE1fThnzxYWN30PH".into(), id: "ai-translate-direction".into() });
    // 文本翻译链
    m.insert("text_credential".into(), KeyEntry { k: "kSy5gtKA4yRUxAVPJPrdYKZ0jBKyd3t1".into(), id: "translate-webmain-key-getter".into() });
    m.insert("text_key".into(), KeyEntry { k: "yU5nT5dK3eZ1pI4j".into(), id: "webfanyi-key-getter-2025".into() });
    // 词典/VIP链
    m.insert("dict_pc".into(), KeyEntry { k: "JCHyRTglPuUCUhAf7FLaqnAcT8AAfYjG".into(), id: "dict_pc_dictvip".into() });
    m.insert("dict_mac".into(), KeyEntry { k: "GO0m3l3ZDdHX1OXwUV7gdPhpd7jg9omF".into(), id: "dict_mac_dictvip".into() });
    m.insert("dict_web".into(), KeyEntry { k: "yehAzpwGBe7YFHStfrKwVJsKOLSi6qXq".into(), id: "dict_web_dictvip".into() });
    m.insert("dict_vip".into(), KeyEntry { k: "PDdE4DR40ACSVW0KBIc3P1jgD31tbihD".into(), id: "dict-vip".into() });
    m.insert("paycenter".into(), KeyEntry { k: "wYjRD9wZbrh6PfvWNVUo60VQeaWCt9un".into(), id: "webdict_paycenter".into() });
    m.insert("bill_server".into(), KeyEntry { k: "ChWW2p7XZVMivnPC0iNwaDxOJyhiKU3P".into(), id: "dict-bill-server".into() });
    m.insert("minor_search".into(), KeyEntry { k: "8XdqRK6tvAQAtRB349Wdmkzxr2A5fqDJ".into(), id: "minor-search-server".into() });
    // OCR
    m.insert("ocr".into(), KeyEntry { k: "VPaHE3kX_vl4BhgYiu2n".into(), id: String::new() });
    m
}

fn load_keys() -> HashMap<String, KeyEntry> {
    let path = keys_cache_path();
    let mut keys = default_keys();
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cached) = serde_json::from_str::<HashMap<String, KeyEntry>>(&data) {
                for (k, v) in cached {
                    keys.insert(k, v);
                }
            }
        }
    }
    keys
}

fn save_keys(keys: &HashMap<String, KeyEntry>) {
    let path = keys_cache_path();
    if let Ok(data) = serde_json::to_string_pretty(keys) {
        std::fs::write(path, data).ok();
    }
}

// ============================================================
// Signing algorithms
// ============================================================

fn md5_hex(input: &str) -> String {
    let digest = md5::compute(input.as_bytes());
    format!("{:x}", digest)
}

/// simple_sign: only signs client, mysticTime, product + key
fn simple_sign(params: &mut HashMap<String, String>, key: &str) {
    let point_params = ["client", "mysticTime", "product"];
    let mut parts = Vec::new();
    for p in &point_params {
        let val = params.get(*p).map(|s| s.as_str()).unwrap_or("");
        parts.push(format!("{}={}", p, val));
    }
    parts.push(format!("key={}", key));
    let raw = parts.join("&");
    let sig = md5_hex(&raw);
    params.insert("sign".into(), sig);
    params.insert("pointParam".into(), point_params.join(","));
}

/// v3_sign: sorts ALL non-empty params alphabetically, appends key, MD5
fn v3_sign(params: &mut HashMap<String, String>, key: &str) {
    let mut valid_keys: Vec<String> = params
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, _)| k.clone())
        .collect();
    valid_keys.sort();
    valid_keys.push("key".into());

    let mut parts = Vec::new();
    for k in &valid_keys {
        let val = if k == "key" { key.to_string() } else { params.get(k).cloned().unwrap_or_default() };
        parts.push(format!("{}={}", k, val));
    }
    let raw = parts.join("&");
    let sig = md5_hex(&raw);
    params.insert("key".into(), key.to_string());
    params.insert("sign".into(), sig);
    let point = valid_keys[..valid_keys.len() - 1].join(",");
    params.insert("pointParam".into(), point);
}

// ============================================================
// Youdao Engine
// ============================================================

pub struct YoudaoEngine {
    client: Client,
    keys: RwLock<HashMap<String, KeyEntry>>,
    text_secret: Mutex<Option<String>>,
    text_token: Mutex<Option<String>>,
    cdn_synced: Mutex<bool>,
}

impl YoudaoEngine {
    pub fn new() -> Self {
        Self {
            // Don't store cookies to avoid rate limiting tracking
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .no_proxy()
                .build()
                .unwrap_or_else(|_| Client::new()),
            keys: RwLock::new(load_keys()),
            text_secret: Mutex::new(None),
            text_token: Mutex::new(None),
            cdn_synced: Mutex::new(false),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    /// Update keys from CDN (called lazily on first use)
    async fn ensure_cdn_synced(&self) {
        {
            let synced = self.cdn_synced.lock().unwrap();
            if *synced {
                return;
            }
        }
        self.sync_keys_from_cdn().await;
        let mut synced = self.cdn_synced.lock().unwrap();
        *synced = true;
    }

    async fn sync_keys_from_cdn(&self) {
        let cdn_version = self.detect_cdn_version().await;
        let cdn_keys = self.update_keys_from_cdn(&cdn_version).await;
        if !cdn_keys.is_empty() {
            let mut keys = self.keys.write().await;
            let id_to_name: HashMap<String, String> = keys
                .iter()
                .filter(|(_, v)| !v.id.is_empty())
                .map(|(name, v)| (v.id.clone(), name.clone()))
                .collect();

            let mut changed = false;
            for (kid, k) in &cdn_keys {
                if let Some(name) = id_to_name.get(kid) {
                    if let Some(entry) = keys.get(name) {
                        if entry.k != *k {
                            keys.insert(name.clone(), KeyEntry { k: k.clone(), id: kid.clone() });
                            changed = true;
                        }
                    }
                } else {
                    keys.insert(format!("cdn_{}", kid), KeyEntry { k: k.clone(), id: kid.clone() });
                    changed = true;
                }
            }
            if changed {
                save_keys(&keys);
            }
        }
    }

    async fn detect_cdn_version(&self) -> String {
        let Ok(resp) = self.client.get("https://fanyi.youdao.com")
            .header("User-Agent", "Mozilla/5.0 Chrome/120")
            .timeout(std::time::Duration::from_secs(10))
            .send().await
        else {
            return "0.9.2".into();
        };
        let Ok(text) = resp.text().await else {
            return "0.9.2".into();
        };
        let re = regex::Regex::new(r"translation-website/(\d+\.\d+\.\d+)/js/app").unwrap();
        if let Some(caps) = re.captures(&text) {
            return caps[1].to_string();
        }
        "0.9.2".into()
    }

    async fn update_keys_from_cdn(&self, cdn_version: &str) -> HashMap<String, String> {
        let cdn_base = format!("https://shared.ydstatic.com/dict/translation-website/{}/js", cdn_version);

        let Ok(resp) = self.client.get("https://fanyi.youdao.com")
            .header("User-Agent", "Mozilla/5.0 Chrome/120")
            .timeout(std::time::Duration::from_secs(10))
            .send().await
        else {
            return HashMap::new();
        };
        let Ok(text) = resp.text().await else {
            return HashMap::new();
        };

        let re_js = regex::Regex::new(r"(chunk-vendors\.[a-f0-9]+\.js|app\.[a-f0-9]+\.js)").unwrap();
        let js_files: Vec<String> = re_js.captures_iter(&text).map(|c| c[1].to_string()).collect();

        let mut found: HashMap<String, String> = HashMap::new();

        for file in &js_files {
            let url = format!("{}/{}", cdn_base, file);
            let Ok(resp) = self.client.get(&url)
                .header("User-Agent", "Mozilla/5.0 Chrome/120")
                .timeout(std::time::Duration::from_secs(60))
                .send().await
            else {
                continue;
            };
            let Ok(data) = resp.bytes().await else {
                continue;
            };
            let data_str = String::from_utf8_lossy(&data);

            // Pattern 1: keyId: "xxx" then find 32-char keys nearby
            let re_kid = regex::Regex::new(r#"(?i)(keyId|keyid)\s*[:=]\s*["']([a-zA-Z0-9_-]+)["']"#).unwrap();
            for m in re_kid.find_iter(&data_str) {
                let start = m.start();
                let end = std::cmp::min(start + 400, data_str.len());
                let chunk = &data_str[start..end];
                if let Some(kid_caps) = re_kid.captures(chunk) {
                    let kid = kid_caps[2].to_string();
                    let re_key = regex::Regex::new(r#"["']([A-Za-z0-9]{32})["']"#).unwrap();
                    if let Some(km) = re_key.captures(chunk) {
                        found.entry(kid).or_insert(km[1].to_string());
                    }
                }
            }

            // Pattern 2: "xxx":"yyy" where yyy is 32 chars
            let re_pair = regex::Regex::new(r#"["']([a-zA-Z0-9_-]{4,64})["'][:=]\s*["']([A-Za-z0-9]{32})["']"#).unwrap();
            for caps in re_pair.captures_iter(&data_str) {
                let kid = caps[1].to_string();
                let k = caps[2].to_string();
                found.entry(kid).or_insert(k);
            }

            // Pattern 3: 16-char credential key near webfanyi-key-getter-2025
            if let Some(idx) = data_str.find("webfanyi-key-getter-2025") {
                let start = idx.saturating_sub(100);
                let end = std::cmp::min(idx + 300, data_str.len());
                let chunk = &data_str[start..end];
                let re_cred = regex::Regex::new(r#"["']([A-Za-z0-9+/=]{12,20})["']"#).unwrap();
                if let Some(km) = re_cred.captures(chunk) {
                    let k = km[1].to_string();
                    if k.len() >= 12 {
                        found.entry("webfanyi-key-getter-2025".into()).or_insert(k);
                    }
                }
            }
        }

        found
    }

    fn mystic_time() -> String {
        let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        dur.as_millis().to_string()
    }

    fn gen_uuid() -> String {
        format!("yd_{}_{}", Self::mystic_time(), std::process::id())
    }

    async fn get_key(&self, name: &str) -> Option<String> {
        let keys = self.keys.read().await;
        keys.get(name).map(|e| e.k.clone())
    }

    async fn get_key_id(&self, name: &str) -> Option<String> {
        let keys = self.keys.read().await;
        keys.get(name).map(|e| e.id.clone())
    }

    /// Ensure we have a dynamic text translation key+token
    async fn ensure_text_key(&self) -> bool {
        {
            let ts = self.text_secret.lock().unwrap();
            if ts.is_some() {
                return true;
            }
        }

        // Ensure CDN keys are synced first
        self.ensure_cdn_synced().await;

        let Some(cred_k) = self.get_key("text_credential").await else { return false };
        let Some(cred_id) = self.get_key_id("text_credential").await else { return false };

        let mut params = HashMap::new();
        params.insert("client".into(), "webmain".into());
        params.insert("product".into(), "webfanyi".into());
        params.insert("appVersion".into(), "12.0.0".into());
        params.insert("vendor".into(), "web".into());
        params.insert("keyfrom".into(), "webfanyi.webmain".into());
        params.insert("mid".into(), "1".into());
        params.insert("screen".into(), "1".into());
        params.insert("model".into(), "1".into());
        params.insert("imei".into(), "1".into());
        params.insert("network".into(), "wifi".into());
        params.insert("abtest".into(), "0".into());
        params.insert("yduuid".into(), "abcdefg".into());
        params.insert("keyid".into(), cred_id.clone());
        params.insert("targetKeyid".into(), "translate-webfanyi-webmain".into());
        params.insert("mysticTime".into(), Self::mystic_time());

        v3_sign(&mut params, &cred_k);

        let Ok(resp) = self.client.post("https://dict-trans.youdao.com/translate/key")
            .form(&params)
            .send().await
        else {
            return false;
        };

        #[derive(Deserialize)]
        struct KeyResp {
            code: i32,
            data: Option<KeyData>,
        }
        #[derive(Deserialize)]
        struct KeyData {
            #[serde(rename = "secretKey")]
            secret_key: String,
            token: Option<String>,
        }

        let Ok(body) = resp.json::<KeyResp>().await else { return false };
        if body.code == 0 {
            if let Some(data) = body.data {
                let mut ts = self.text_secret.lock().unwrap();
                let mut tt = self.text_token.lock().unwrap();
                *ts = Some(data.secret_key);
                *tt = data.token;
                return true;
            }
        }
        false
    }

    /// Translate using the text translation chain (no known daily limit)
    async fn translate_text_chain(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        // Always fetch fresh keys like Python version to avoid stale keys
        let Some(cred_k) = self.get_key("text_credential").await else {
            return Err(anyhow::anyhow!("Failed to get Youdao text_credential key"));
        };
        let Some(cred_id) = self.get_key_id("text_credential").await else {
            return Err(anyhow::anyhow!("Failed to get Youdao text_credential id"));
        };

        // Fetch dynamic secret + token for each call
        let mut key_params = HashMap::new();
        key_params.insert("client".into(), "webmain".into());
        key_params.insert("product".into(), "webfanyi".into());
        key_params.insert("appVersion".into(), "12.0.0".into());
        key_params.insert("vendor".into(), "web".into());
        key_params.insert("keyfrom".into(), "webfanyi.webmain".into());
        key_params.insert("mid".into(), "1".into());
        key_params.insert("screen".into(), "1".into());
        key_params.insert("model".into(), "1".into());
        key_params.insert("imei".into(), "1".into());
        key_params.insert("network".into(), "wifi".into());
        key_params.insert("abtest".into(), "0".into());
        key_params.insert("yduuid".into(), "abcdefg".into());
        key_params.insert("keyid".into(), cred_id);
        key_params.insert("targetKeyid".into(), "translate-webfanyi-webmain".into());
        key_params.insert("mysticTime".into(), Self::mystic_time());

        v3_sign(&mut key_params, &cred_k);

        #[derive(Deserialize)]
        struct KeyResp {
            code: i32,
            data: Option<KeyData>,
        }
        #[derive(Deserialize)]
        struct KeyData {
            #[serde(rename = "secretKey")]
            secret_key: String,
            token: Option<String>,
        }

        let key_resp = self.client.post("https://dict-trans.youdao.com/translate/key")
            .form(&key_params)
            .send().await?;

        let key_body = key_resp.json::<KeyResp>().await?;
        if key_body.code != 0 {
            return Err(anyhow::anyhow!("Failed to get Youdao text key, code: {}", key_body.code));
        }

        let key_data = key_body.data.ok_or_else(|| anyhow::anyhow!("No key data"))?;
        let secret = key_data.secret_key;
        let token = key_data.token.unwrap_or_default();

        let encoded_text = urlencoding::encode(text).to_string();

        let mut params = HashMap::new();
        params.insert("product".into(), "webfanyi".into());
        params.insert("appVersion".into(), "1".into());
        params.insert("client".into(), "webmain".into());
        params.insert("mid".into(), "1".into());
        params.insert("vendor".into(), "web".into());
        params.insert("screen".into(), "1".into());
        params.insert("model".into(), "1".into());
        params.insert("imei".into(), "1".into());
        params.insert("network".into(), "wifi".into());
        params.insert("keyfrom".into(), "webfanyi.webmain".into());
        params.insert("keyid".into(), "translate-webfanyi-webmain".into());
        params.insert("mysticTime".into(), Self::mystic_time());
        params.insert("yduuid".into(), "abcdefg".into());
        params.insert("modelName".into(), "llmLite".into());
        params.insert("useTerm".into(), "false".into());
        params.insert("i".into(), encoded_text);
        params.insert("from".into(), from.to_string());
        params.insert("to".into(), to.to_string());
        params.insert("signSecretKey".into(), secret.clone());
        params.insert("keyId".into(), "translate-webfanyi-webmain".into());
        params.insert("token".into(), token);
        params.insert("source".into(), "webmain".into());

        v3_sign(&mut params, &secret);

        let resp = self.client.post("https://dict-trans.youdao.com/webtranslate/sse")
            .form(&params)
            .send().await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("Youdao text API error: {}", status));
        }

        let body = resp.text().await?;
        let mut result = String::new();

        for line in body.lines() {
            let line = line.trim();
            if !line.starts_with("data:") {
                continue;
            }
            let payload = &line[5..].trim();
            if payload.is_empty() {
                continue;
            }
            if let Ok(d) = serde_json::from_str::<serde_json::Value>(payload) {
                if let Some(chunk) = d.get("transIncre").and_then(|v| v.as_str()) {
                    result.push_str(chunk);
                }
            }
        }

        if result.is_empty() {
            return Err(anyhow::anyhow!("Youdao text returned empty result"));
        }

        Ok(result)
    }

    /// Map our language codes to Youdao codes
    fn map_lang(lang: &str) -> &str {
        match lang {
            "zh" => "zh-CHS",
            "ja" => "ja",
            "ko" => "ko",
            "en" => "en",
            "fr" => "fr",
            "de" => "de",
            "es" => "es",
            "ru" => "ru",
            "pt" => "pt",
            "it" => "it",
            "ar" => "ar",
            "th" => "th",
            "vi" => "vi",
            "auto" => "auto",
            _ => "auto",
        }
    }
}

#[async_trait]
impl TranslationEngine for YoudaoEngine {
    fn name(&self) -> &str {
        "Youdao"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        let y_from = Self::map_lang(from);
        let y_to = Self::map_lang(to);
        self.translate_text_chain(text, y_from, y_to).await
    }
}
