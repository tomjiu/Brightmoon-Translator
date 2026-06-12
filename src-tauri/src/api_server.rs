use axum::{
    extract::State as AxumState,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::cache::TranslationCache;
use crate::capabilities::handle_browser_request;
use crate::config::AppConfig;
use crate::engine;
use crate::glossary::Glossary;
use crate::memory::HistoryStore;
use crate::models::browser_protocol::BrowserTranslateRequest;
use crate::models::error::{ApiError, TranslationError};
use crate::models::glossary::GlossaryEntry;
use crate::services::TranslationService;

/// Mask a secret string, keeping first 4 and last 4 bytes visible.
/// Safe for ASCII secrets (API keys, tokens). Uses byte slicing for performance.
fn mask_secret(s: &str) -> String {
    let len = s.len();
    if len <= 12 {
        return "*".repeat(len);
    }
    format!("{}...{}", &s[..4], &s[len - 4..])
}

/// Return a sanitized copy of config with all secret fields masked.
/// Used for API responses to avoid leaking credentials to external callers.
fn sanitize_config(config: &AppConfig) -> serde_json::Value {
    let mut v = serde_json::to_value(config).unwrap_or_default();
    // Mask LLM keys
    if let Some(llm) = v.get_mut("llm") {
        if let Some(key) = llm
            .get_mut("apiKey")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            if let Some(obj) = llm.as_object_mut() {
                obj.insert(
                    "apiKey".into(),
                    serde_json::Value::String(mask_secret(&key)),
                );
            }
        }
        if let Some(keys) = llm.get_mut("apiKeys").and_then(|v| v.as_array().cloned()) {
            let masked: Vec<_> = keys
                .iter()
                .map(|k| {
                    k.as_str()
                        .map(|s| serde_json::Value::String(mask_secret(s)))
                        .unwrap_or(k.clone())
                })
                .collect();
            if let Some(obj) = llm.as_object_mut() {
                obj.insert("apiKeys".into(), serde_json::Value::Array(masked));
            }
        }
    }
    // Mask engine secrets
    if let Some(engines) = v.get_mut("engines") {
        for field in &["deepl", "deeplx", "baidu"] {
            if let Some(engine_cfg) = engines.get_mut(field) {
                for secret_field in &["apiKey", "secret"] {
                    if let Some(val) = engine_cfg
                        .get(*secret_field)
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                    {
                        if let Some(obj) = engine_cfg.as_object_mut() {
                            obj.insert(
                                secret_field.to_string(),
                                serde_json::Value::String(mask_secret(&val)),
                            );
                        }
                    }
                }
            }
        }
        if let Some(youdao) = engines.get_mut("youdao") {
            if let Some(val) = youdao
                .get("ocrAppSecret")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
            {
                if let Some(obj) = youdao.as_object_mut() {
                    obj.insert(
                        "ocrAppSecret".into(),
                        serde_json::Value::String(mask_secret(&val)),
                    );
                }
            }
        }
    }
    // Mask proxy password
    if let Some(proxy) = v.get_mut("proxy") {
        if let Some(val) = proxy
            .get("password")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            if let Some(obj) = proxy.as_object_mut() {
                obj.insert(
                    "password".into(),
                    serde_json::Value::String(mask_secret(&val)),
                );
            }
        }
    }
    v
}

#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<Mutex<AppConfig>>,
    pub history: Arc<Mutex<HistoryStore>>,
    pub engine_router: Arc<RwLock<engine::Router>>,
    pub cache: Arc<TranslationCache>,
    pub glossary: Arc<Mutex<Glossary>>,
    pub translation_service: Arc<TranslationService>,
}

impl ApiState {
    pub fn from_app_state(state: &crate::AppState) -> Self {
        Self {
            config: state.system.config.clone(),
            history: state.document.history.clone(),
            engine_router: state.translation.engine_router.clone(),
            cache: state.translation.cache.clone(),
            glossary: state.translation.glossary.clone(),
            translation_service: state.translation.service.clone(),
        }
    }
}

#[derive(Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    #[serde(default = "default_from")]
    pub from: String,
    #[serde(default = "default_to")]
    pub to: String,
    #[serde(default)]
    pub stream: bool,
}

fn default_from() -> String {
    "auto".to_string()
}

fn default_to() -> String {
    "zh".to_string()
}

// POST /translate
async fn translate(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<TranslateRequest>,
) -> impl IntoResponse {
    if req.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Text is empty".to_string(),
            }),
        )
            .into_response();
    }

    if req.text.len() > 50_000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Text exceeds maximum length of 50,000 characters".to_string(),
            }),
        )
            .into_response();
    }

    // Use TranslationService for the full pipeline (glossary, blacklist, cache, history, metrics)
    match state
        .translation_service
        .translate(&req.text, &req.from, &req.to)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            let status = match &e {
                TranslationError::NoEngine | TranslationError::AllEnginesFailed { .. } => {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                TranslationError::InvalidInput(_) => StatusCode::BAD_REQUEST,
                TranslationError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(ApiError::from(&e))).into_response()
        },
    }
}

// POST /translate/primary - Translate with primary engine only
async fn translate_primary(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<TranslateRequest>,
) -> impl IntoResponse {
    if req.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Text is empty".to_string(),
            }),
        )
            .into_response();
    }

    if req.text.len() > 50_000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Text exceeds maximum length of 50,000 characters".to_string(),
            }),
        )
            .into_response();
    }

    #[derive(Serialize)]
    struct PrimaryResult {
        engine: String,
        text: String,
    }

    // Use TranslationService for the full pipeline
    match state
        .translation_service
        .translate_primary(&req.text, &req.from, &req.to)
        .await
    {
        Ok(translated) => (
            StatusCode::OK,
            Json(PrimaryResult {
                engine: "primary".to_string(),
                text: translated,
            }),
        )
            .into_response(),
        Err(e) => {
            let status = match &e {
                TranslationError::NoEngine | TranslationError::AllEnginesFailed { .. } => {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                TranslationError::InvalidInput(_) => StatusCode::BAD_REQUEST,
                TranslationError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(ApiError::from(&e))).into_response()
        },
    }
}

// GET /config - Returns sanitized config (secrets masked)
async fn get_config(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let config = state.config.lock().await;
    Json(sanitize_config(&config)).into_response()
}

// POST /config - Partial update
async fn update_config(
    AxumState(state): AxumState<ApiState>,
    Json(updates): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut config = state.config.lock().await;

    // Merge updates into config
    let config_json = match serde_json::to_value(&*config) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: format!("Failed to serialize config: {}", e),
                }),
            )
                .into_response();
        },
    };

    let mut merged = config_json;
    if let (Some(obj), Some(updates_obj)) = (merged.as_object_mut(), updates.as_object()) {
        for (key, value) in updates_obj {
            obj.insert(key.clone(), value.clone());
        }
    }

    match serde_json::from_value::<AppConfig>(merged) {
        Ok(new_config) => {
            new_config.save();

            // Hot-reload: rebuild engine router with new config
            let new_router = engine::Router::new(&new_config);
            let response = Json(sanitize_config(&new_config)).into_response();

            let mut router = state.engine_router.write().await;
            *router = new_router;
            *config = new_config; // move instead of clone

            response
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: format!("Invalid config: {}", e),
            }),
        )
            .into_response(),
    }
}

// GET /history
async fn get_history(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let history = state.history.lock().await;
    let items = history.get_all();
    Json(items).into_response()
}

// GET /engines
async fn get_engines(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let router = state.engine_router.read().await;
    let engines: Vec<String> = router
        .engines_iter()
        .map(|e| e.name().to_string())
        .collect();
    drop(router);

    #[derive(Serialize)]
    struct EnginesResponse {
        engines: Vec<String>,
        count: usize,
    }

    Json(EnginesResponse {
        count: engines.len(),
        engines,
    })
    .into_response()
}

// GET /health
async fn health() -> impl IntoResponse {
    #[derive(Serialize)]
    struct HealthResponse {
        status: String,
        version: String,
    }

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// POST /browser/translate - Browser extension translation via real TranslationService
async fn browser_translate(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<BrowserTranslateRequest>,
) -> impl IntoResponse {
    let config = state.config.lock().await;
    match handle_browser_request(&req, &state.translation_service, &config).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => {
            let status = match &e.error {
                TranslationError::NoEngine | TranslationError::AllEnginesFailed { .. } => {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                TranslationError::InvalidInput(_) => StatusCode::BAD_REQUEST,
                TranslationError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(e)).into_response()
        },
    }
}

// GET /glossary - List all glossary entries
async fn get_glossary(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let glossary = state.glossary.lock().await;
    Json(glossary.get_all_entries().clone()).into_response()
}

#[derive(Deserialize)]
struct AddGlossaryRequest {
    #[serde(rename = "langPair")]
    lang_pair: String,
    source: String,
    target: String,
    #[serde(default)]
    context: Option<String>,
}

// POST /glossary - Add a glossary entry
async fn add_glossary_entry(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<AddGlossaryRequest>,
) -> impl IntoResponse {
    let mut glossary = state.glossary.lock().await;
    glossary
        .add_entry(
            req.lang_pair,
            GlossaryEntry {
                source: req.source,
                target: req.target,
                context: req.context,
            },
        )
        .await;
    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
struct RemoveGlossaryRequest {
    #[serde(rename = "langPair")]
    lang_pair: String,
    source: String,
}

// DELETE /glossary - Remove a glossary entry
async fn remove_glossary_entry(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<RemoveGlossaryRequest>,
) -> impl IntoResponse {
    let mut glossary = state.glossary.lock().await;
    if glossary.remove_entry(&req.lang_pair, &req.source).await {
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

// GET /blacklist - Get current translation blacklist
async fn get_blacklist(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let config = state.config.lock().await;
    Json(serde_json::json!({ "words": config.translation_blacklist })).into_response()
}

#[derive(Deserialize)]
struct UpdateBlacklistRequest {
    words: Vec<String>,
}

// POST /blacklist - Set translation blacklist
async fn update_blacklist(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<UpdateBlacklistRequest>,
) -> impl IntoResponse {
    let mut config = state.config.lock().await;
    config.translation_blacklist = req.words;
    config.save();
    StatusCode::OK.into_response()
}

// GET /cache/stats - Get cache statistics
async fn cache_stats(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let stats = state.cache.stats().await;
    Json(stats).into_response()
}

// POST /cache/clear - Clear translation cache
async fn clear_cache(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    state.cache.clear().await;
    StatusCode::OK.into_response()
}

pub fn create_router(state: ApiState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _parts| {
            // Allow browser extensions and localhost origins
            let o = origin.as_bytes();
            o.starts_with(b"chrome-extension://")
                || o.starts_with(b"moz-extension://")
                || o.starts_with(b"http://localhost")
                || o.starts_with(b"http://127.0.0.1")
        }))
        .allow_methods(tower_http::cors::Any)
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT]);

    Router::new()
        .route("/health", get(health))
        .route("/translate", post(translate))
        .route("/translate/primary", post(translate_primary))
        .route("/config", get(get_config).post(update_config))
        .route("/history", get(get_history))
        .route("/engines", get(get_engines))
        .route("/browser/translate", post(browser_translate))
        .route(
            "/glossary",
            get(get_glossary)
                .post(add_glossary_entry)
                .delete(remove_glossary_entry),
        )
        .route("/blacklist", get(get_blacklist).post(update_blacklist))
        .route("/cache/stats", get(cache_stats))
        .route("/cache/clear", post(clear_cache))
        .layer(cors)
        .with_state(state)
}

pub async fn start_server(port: u16, state: ApiState) -> Result<(), String> {
    let app = create_router(state);
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    tracing::info!("API server listening on http://{}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}
