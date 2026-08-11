use super::TranslationEngine;
use async_trait::async_trait;
use reqwest::Client;

/// Google free endpoint. Prefer a direct (no-proxy) client first; if that fails
/// and a proxy client was provided, retry once via proxy (common GFW pattern).
pub struct GoogleEngine {
    /// Usually built with `.no_proxy()` for direct access.
    direct: Client,
    /// Shared router client (honors user proxy settings). None = direct only.
    via_proxy: Option<Client>,
}

impl Default for GoogleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleEngine {
    pub fn new() -> Self {
        Self {
            direct: Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .unwrap_or_else(|_| Client::new()),
            via_proxy: None,
        }
    }

    /// `proxy_client` is the shared Router client (may include system/user proxy).
    pub fn with_clients(direct: Client, proxy_client: Client) -> Self {
        Self {
            direct,
            via_proxy: Some(proxy_client),
        }
    }

    pub fn with_client(mut self, client: Client) -> Self {
        self.via_proxy = Some(client);
        self
    }

    async fn translate_once(
        client: &Client,
        text: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<String> {
        let from_code = if from == "auto" { "auto" } else { from };
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            from_code,
            to,
            urlencoding::encode(text)
        );

        let resp = client.get(&url).send().await?;
        super::check_response(&resp, "Google")?;

        let body: serde_json::Value = resp.json().await?;

        let translated = body[0]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item[0].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .ok_or_else(|| anyhow::anyhow!("Google API returned unexpected response format"))?;

        if translated.is_empty() {
            return Err(anyhow::anyhow!("Google API returned empty translation"));
        }

        Ok(translated)
    }
}

#[async_trait]
impl TranslationEngine for GoogleEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> anyhow::Result<String> {
        match Self::translate_once(&self.direct, text, from, to).await {
            Ok(t) => Ok(t),
            Err(direct_err) => {
                if let Some(ref proxy) = self.via_proxy {
                    tracing::debug!(
                        "[Google] direct failed ({}), retrying via proxy client",
                        direct_err
                    );
                    Self::translate_once(proxy, text, from, to)
                        .await
                        .map_err(|proxy_err| {
                            anyhow::anyhow!(
                                "Google unreachable (direct: {direct_err}; proxy: {proxy_err})"
                            )
                        })
                } else {
                    Err(direct_err)
                }
            },
        }
    }

    fn name(&self) -> &'static str {
        "Google"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
