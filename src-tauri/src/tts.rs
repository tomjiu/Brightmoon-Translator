use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsVoice {
    pub name: String,
    pub locale: String,
    pub gender: String,
}

// Default voices for common languages
pub fn default_voices() -> Vec<TtsVoice> {
    vec![
        TtsVoice {
            name: "zh-CN-XiaoxiaoNeural".to_string(),
            locale: "zh-CN".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "zh-CN-YunxiNeural".to_string(),
            locale: "zh-CN".to_string(),
            gender: "Male".to_string(),
        },
        TtsVoice {
            name: "zh-CN-YunjianNeural".to_string(),
            locale: "zh-CN".to_string(),
            gender: "Male".to_string(),
        },
        TtsVoice {
            name: "en-US-JennyNeural".to_string(),
            locale: "en-US".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "en-US-GuyNeural".to_string(),
            locale: "en-US".to_string(),
            gender: "Male".to_string(),
        },
        TtsVoice {
            name: "en-GB-SoniaNeural".to_string(),
            locale: "en-GB".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "ja-JP-NanamiNeural".to_string(),
            locale: "ja-JP".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "ko-KR-SunHiNeural".to_string(),
            locale: "ko-KR".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "fr-FR-DeniseNeural".to_string(),
            locale: "fr-FR".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "de-DE-KatjaNeural".to_string(),
            locale: "de-DE".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "es-ES-ElviraNeural".to_string(),
            locale: "es-ES".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "ru-RU-SvetlanaNeural".to_string(),
            locale: "ru-RU".to_string(),
            gender: "Female".to_string(),
        },
        TtsVoice {
            name: "pt-BR-FranciscaNeural".to_string(),
            locale: "pt-BR".to_string(),
            gender: "Female".to_string(),
        },
    ]
}

pub fn get_voice_for_lang(lang: &str) -> &str {
    match lang {
        "zh" => "zh-CN-XiaoxiaoNeural",
        "en" => "en-US-JennyNeural",
        "ja" => "ja-JP-NanamiNeural",
        "ko" => "ko-KR-SunHiNeural",
        "fr" => "fr-FR-DeniseNeural",
        "de" => "de-DE-KatjaNeural",
        "es" => "es-ES-ElviraNeural",
        "ru" => "ru-RU-SvetlanaNeural",
        "pt" => "pt-BR-FranciscaNeural",
        "it" => "it-IT-ElsaNeural",
        "ar" => "ar-SA-ZariyahNeural",
        "th" => "th-TH-PremwadeeNeural",
        "vi" => "vi-VN-HoaiMyNeural",
        _ => "en-US-JennyNeural",
    }
}

const DEFAULT_EDGE_TTS_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_TTS_URL: &str = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={}";

/// Get the Edge TTS token: config value > env var > built-in default
fn get_edge_tts_token(config_token: &str) -> String {
    if !config_token.is_empty() {
        return config_token.to_string();
    }
    if let Ok(env_token) = std::env::var("EDGE_TTS_TOKEN") {
        if !env_token.is_empty() {
            return env_token;
        }
    }
    DEFAULT_EDGE_TTS_TOKEN.to_string()
}

pub async fn synthesize(text: &str, voice: &str) -> anyhow::Result<Vec<u8>> {
    synthesize_with_token(text, voice, "").await
}

/// OpenAI-compatible TTS: POST {base}/audio/speech
pub async fn synthesize_openai(
    text: &str,
    api_key: &str,
    base_url: &str,
    model: &str,
    voice: &str,
    speed: f32,
) -> anyhow::Result<Vec<u8>> {
    if api_key.trim().is_empty() {
        anyhow::bail!("OpenAI TTS api_key is empty");
    }
    let mut base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        base = "https://api.openai.com/v1".into();
    }
    if !base.starts_with("http") {
        base = format!("https://{base}");
    }
    let url = if base.ends_with("/audio/speech") {
        base
    } else {
        format!("{base}/audio/speech")
    };
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": if model.is_empty() { "tts-1" } else { model },
        "voice": if voice.is_empty() { "alloy" } else { voice },
        "speed": if speed <= 0.0 { 1.0 } else { speed },
        "input": text,
    });
    let resp = client
        .post(&url)
        .bearer_auth(api_key.trim())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "OpenAI TTS HTTP {}: {}",
            status,
            err_body.chars().take(200).collect::<String>()
        );
    }
    Ok(resp.bytes().await?.to_vec())
}

/// Fish Audio `OpenAPI` TTS (`POST https://api.fish.audio/v1/tts`).
/// Free tier model: `s2.1-pro-free` (same quality as s2.1-pro, fair-use, no SLA).
/// `voice` / config `reference_id` is a Fish voice model id from the library or your clone.
pub async fn synthesize_fish(
    text: &str,
    api_key: &str,
    model: &str,
    reference_id: &str,
    format: &str,
    speed: f32,
) -> anyhow::Result<Vec<u8>> {
    let key = api_key.trim();
    let key = if key.is_empty() {
        std::env::var("FISH_API_KEY").unwrap_or_default()
    } else {
        key.to_string()
    };
    if key.trim().is_empty() {
        anyhow::bail!("Fish Audio API key is empty (set fishTts.apiKey or FISH_API_KEY)");
    }

    let model = if model.trim().is_empty() {
        "s2.1-pro-free"
    } else {
        model.trim()
    };
    let format = if format.trim().is_empty() {
        "mp3"
    } else {
        format.trim()
    };
    let speed = if speed <= 0.0 {
        1.0
    } else {
        speed.clamp(0.5, 2.0)
    };

    let mut body = serde_json::json!({
        "text": text,
        "format": format,
        "normalize": true,
        "latency": "normal",
        "prosody": {
            "speed": speed,
            "volume": 0,
            "normalize_loudness": true
        }
    });
    let rid = reference_id.trim();
    if !rid.is_empty() {
        body["reference_id"] = serde_json::Value::String(rid.to_string());
    }

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.fish.audio/v1/tts")
        .bearer_auth(key.trim())
        .header("Content-Type", "application/json")
        .header("model", model)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "Fish Audio TTS HTTP {}: {}",
            status,
            err_body.chars().take(240).collect::<String>()
        );
    }
    Ok(resp.bytes().await?.to_vec())
}

/// Sample Fish voice model ids (`reference_id`). Paste any id from fish.audio library.
pub fn default_fish_voices() -> Vec<TtsVoice> {
    vec![TtsVoice {
        name: "12b8a0bf8e0042c3b11e519d11db8b68".to_string(),
        locale: "en".to_string(),
        gender: "Demo".to_string(),
    }]
}

/// Youdao dictvoice (good for words; weak for long text).
pub async fn synthesize_youdao_dictvoice(text: &str, lang: &str) -> anyhow::Result<Vec<u8>> {
    // type=1 UK, type=2 US; default US for en, type=2 otherwise
    let voice_type = if lang.starts_with("en") { "2" } else { "2" };
    let url = format!(
        "https://dict.youdao.com/dictvoice?audio={}&type={}",
        urlencoding::encode(text),
        voice_type
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("Youdao dictvoice HTTP {status}");
    }
    Ok(resp.bytes().await?.to_vec())
}

pub async fn synthesize_with_token(
    text: &str,
    voice: &str,
    config_token: &str,
) -> anyhow::Result<Vec<u8>> {
    let token = get_edge_tts_token(config_token);
    let url = format!(
        "{}&ConnectionId={}",
        EDGE_TTS_URL.replace("{}", &token),
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );

    let (mut ws_stream, _) = connect_async(&url).await?;

    // Send speech config
    let config_msg = "Content-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
        {\"context\":{\"synthesis\":{\"audio\":{\"metadataoptions\":{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"true\"},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}".to_string();
    ws_stream.send(Message::Text(config_msg)).await?;

    // Send SSML
    let ssml = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
        <voice name='{}'>\
        <prosody pitch='+0Hz' rate='+0%' volume='+0%'>\
        {}\
        </prosody></voice></speak>",
        voice,
        xml_escape(text)
    );
    let request_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let ssml_msg = format!(
        "Content-Type:application/ssml+xml\r\nPath:ssml\r\nX-RequestId:{}\r\nX-Timestamp:{}\r\n\r\n{}",
        request_id,
        chrono::Utc::now().format("%a %b %d %Y %H:%M:%S GMT"),
        ssml
    );
    ws_stream.send(Message::Text(ssml_msg)).await?;

    // Collect audio chunks
    let mut audio_data = Vec::new();

    while let Some(msg) = ws_stream.next().await {
        match msg? {
            Message::Binary(data)
                // Extract audio from binary message
                // Format: header length (2 bytes) + header + audio data
                if data.len() > 2 => {
                    let header_len = u16::from_be_bytes([data[0], data[1]]) as usize;
                    if data.len() > 2 + header_len {
                        audio_data.extend_from_slice(&data[2 + header_len..]);
                    }
                },
            Message::Text(text)
                if text.contains("Path:turn.end") => {
                    break;
                },
            _ => {},
        }
    }

    // Clean up
    ws_stream.close(None).await?;

    Ok(audio_data)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
