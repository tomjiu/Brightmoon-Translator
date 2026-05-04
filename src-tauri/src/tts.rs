use base64::Engine;
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
        TtsVoice { name: "zh-CN-XiaoxiaoNeural".to_string(), locale: "zh-CN".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "zh-CN-YunxiNeural".to_string(), locale: "zh-CN".to_string(), gender: "Male".to_string() },
        TtsVoice { name: "zh-CN-YunjianNeural".to_string(), locale: "zh-CN".to_string(), gender: "Male".to_string() },
        TtsVoice { name: "en-US-JennyNeural".to_string(), locale: "en-US".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "en-US-GuyNeural".to_string(), locale: "en-US".to_string(), gender: "Male".to_string() },
        TtsVoice { name: "en-GB-SoniaNeural".to_string(), locale: "en-GB".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "ja-JP-NanamiNeural".to_string(), locale: "ja-JP".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "ko-KR-SunHiNeural".to_string(), locale: "ko-KR".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "fr-FR-DeniseNeural".to_string(), locale: "fr-FR".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "de-DE-KatjaNeural".to_string(), locale: "de-DE".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "es-ES-ElviraNeural".to_string(), locale: "es-ES".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "ru-RU-SvetlanaNeural".to_string(), locale: "ru-RU".to_string(), gender: "Female".to_string() },
        TtsVoice { name: "pt-BR-FranciscaNeural".to_string(), locale: "pt-BR".to_string(), gender: "Female".to_string() },
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

const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_TTS_URL: &str = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={}";

pub async fn synthesize(text: &str, voice: &str) -> anyhow::Result<Vec<u8>> {
    let url = format!("{}&ConnectionId={}",
        EDGE_TTS_URL.replace("{}", TRUSTED_CLIENT_TOKEN),
        uuid::Uuid::new_v4().to_string().replace("-", "")
    );

    let (mut ws_stream, _) = connect_async(&url).await?;

    // Send speech config
    let config_msg = format!(
        "Content-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
        {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"true\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}"
    );
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
    let request_id = uuid::Uuid::new_v4().to_string().replace("-", "");
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
            Message::Binary(data) => {
                // Extract audio from binary message
                // Format: header length (2 bytes) + header + audio data
                if data.len() > 2 {
                    let header_len = u16::from_be_bytes([data[0], data[1]]) as usize;
                    if data.len() > 2 + header_len {
                        audio_data.extend_from_slice(&data[2 + header_len..]);
                    }
                }
            }
            Message::Text(text) => {
                if text.contains("Path:turn.end") {
                    break;
                }
            }
            _ => {}
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
