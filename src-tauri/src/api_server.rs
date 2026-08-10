use axum::{
    body::Body,
    extract::State as AxumState,
    http::{header, Request, StatusCode},
    middleware::{self, Next},
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
            .and_then(|v| v.as_str().map(std::string::ToString::to_string))
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
        for field in &["deepl", "deeplx", "baidu", "caiyun"] {
            if let Some(engine_cfg) = engines.get_mut(field) {
                for secret_field in &["apiKey", "secret", "apiToken"] {
                    if let Some(val) = engine_cfg
                        .get(*secret_field)
                        .and_then(|v| v.as_str().map(std::string::ToString::to_string))
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
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
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
    // Mask proxy / sync passwords
    for section in &["proxy", "sync"] {
        if let Some(obj_sec) = v.get_mut(*section) {
            if let Some(val) = obj_sec
                .get("password")
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
            {
                if let Some(obj) = obj_sec.as_object_mut() {
                    obj.insert(
                        "password".into(),
                        serde_json::Value::String(mask_secret(&val)),
                    );
                }
            }
        }
    }
    // Mask API bridge token
    if let Some(token) = v
        .get("apiServerToken")
        .and_then(|x| x.as_str())
        .map(std::string::ToString::to_string)
    {
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                "apiServerToken".into(),
                serde_json::Value::String(mask_secret(&token)),
            );
        }
    }
    v
}

/// Extract bearer / X-Api-Token from request headers.
fn extract_api_token(req: &Request<Body>) -> Option<String> {
    if let Some(auth) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        let auth = auth.trim();
        if let Some(rest) = auth
            .strip_prefix("Bearer ")
            .or_else(|| auth.strip_prefix("bearer "))
        {
            let t = rest.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    req.headers()
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Require valid API token for all routes except GET /health (and CORS preflight).
async fn require_api_token(
    AxumState(state): AxumState<ApiState>,
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let path = req.uri().path();
    let method = req.method();
    if method == axum::http::Method::OPTIONS
        || (method == axum::http::Method::GET && (path == "/health" || path == "/health/"))
    {
        return next.run(req).await;
    }

    let expected = {
        let config = state.config.lock().await;
        config.api_server_token.clone()
    };

    if expected.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                error: "API token not configured; enable bridge in settings".into(),
            }),
        )
            .into_response();
    }

    let provided = extract_api_token(&req);
    match provided {
        Some(t) if constant_time_eq(t.as_bytes(), expected.as_bytes()) => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "Unauthorized: provide Authorization: Bearer <token> or X-Api-Token".into(),
            }),
        )
            .into_response(),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<Mutex<AppConfig>>,
    pub history: Arc<Mutex<HistoryStore>>,
    pub engine_router: Arc<RwLock<engine::Router>>,
    pub cache: Arc<TranslationCache>,
    pub glossary: Arc<Mutex<Glossary>>,
    pub translation_service: Arc<TranslationService>,
    /// Optional handle for control routes (show window, emit hotkey events).
    pub app_handle: Option<tauri::AppHandle>,
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
            app_handle: None,
        }
    }

    pub fn with_app_handle(mut self, app: tauri::AppHandle) -> Self {
        self.app_handle = Some(app);
        self
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

    // Façade: full pipeline (glossary, blacklist, cache, history, metrics)
    match state
        .translation_service
        .run_full(
            crate::models::translation::TranslateChannel::Http,
            &req.text,
            &req.from,
            &req.to,
        )
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

    // Façade: primary engine only
    match state
        .translation_service
        .run_primary(
            crate::models::translation::TranslateChannel::Http,
            &req.text,
            &req.from,
            &req.to,
        )
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
                    error: format!("Failed to serialize config: {e}"),
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
                error: format!("Invalid config: {e}"),
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

use tauri::{Emitter, Manager};

fn control_ok() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

fn control_unavailable() -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiError {
            error: "App handle not available for control routes".into(),
        }),
    )
        .into_response()
}

/// POST /control/show — show + focus main window
async fn control_show(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let Some(app) = state.app_handle.as_ref() else {
        return control_unavailable();
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    control_ok().into_response()
}

/// POST /`control/selection_translate` — same as tray/hotkey selection translate
async fn control_selection_translate(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let Some(app) = state.app_handle.as_ref() else {
        return control_unavailable();
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("trigger-translate-selection", ());
    }
    control_ok().into_response()
}

/// POST /`control/ocr_translate` — same as tray OCR
async fn control_ocr_translate(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let Some(app) = state.app_handle.as_ref() else {
        return control_unavailable();
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("trigger-ocr-screenshot", ());
    }
    control_ok().into_response()
}

/// POST /`control/open_settings` — show main + navigate settings
async fn control_open_settings(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let Some(app) = state.app_handle.as_ref() else {
        return control_unavailable();
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("navigate", "settings");
    }
    control_ok().into_response()
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
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::AUTHORIZATION,
            header::HeaderName::from_static("x-api-token"),
        ]);

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
        .route("/control/show", post(control_show))
        .route(
            "/control/selection_translate",
            post(control_selection_translate),
        )
        .route("/control/ocr_translate", post(control_ocr_translate))
        .route("/control/open_settings", post(control_open_settings))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_token,
        ))
        .layer(cors)
        .with_state(state)
}

/// Ensure a non-empty API token exists before listening (persist if generated).
pub async fn ensure_api_token(state: &ApiState) -> String {
    let mut config = state.config.lock().await;
    if config.api_server_token.trim().is_empty() {
        config.api_server_token = uuid::Uuid::new_v4().to_string();
        config.save();
        tracing::info!("[API] Generated new api_server_token (copy from Advanced settings)");
    }
    config.api_server_token.clone()
}

pub async fn start_server(port: u16, state: ApiState) -> Result<(), String> {
    let _token = ensure_api_token(&state).await;
    let app = create_router(state);
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {addr}: {e}"))?;

    tracing::info!(
        "API server listening on http://{} (auth required except GET /health)",
        addr
    );

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}
