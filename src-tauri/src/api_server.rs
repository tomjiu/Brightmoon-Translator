use axum::{
    extract::State as AxumState,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::{Any, CorsLayer};

use crate::config::AppConfig;
use crate::engine::{self, TranslateResponse};
use crate::glossary::Glossary;
use crate::memory::HistoryStore;
use crate::services::TranslationService;
use crate::TranslationCache;

#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<Mutex<AppConfig>>,
    pub history: Arc<Mutex<HistoryStore>>,
    pub engine_router: Arc<RwLock<engine::Router>>,
    pub cache: Arc<TranslationCache>,
    pub glossary: Arc<Mutex<Glossary>>,
    pub translation_service: Arc<TranslationService>,
}

impl From<&crate::AppState> for ApiState {
    fn from(state: &crate::AppState) -> Self {
        Self {
            config: state.config.clone(),
            history: state.history.clone(),
            engine_router: state.engine_router.clone(),
            cache: state.cache.clone(),
            glossary: state.glossary.clone(),
            translation_service: state.translation_service.clone(),
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

#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
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

    // Use TranslationService for the full pipeline (glossary, blacklist, cache, history, metrics)
    match state.translation_service.translate(&req.text, &req.from, &req.to).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Translation failed: {}", e),
            }),
        )
            .into_response(),
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

    #[derive(Serialize)]
    struct PrimaryResult {
        engine: String,
        text: String,
    }

    // Use TranslationService for the full pipeline
    match state.translation_service.translate_primary(&req.text, &req.from, &req.to).await {
        Ok(translated) => (
            StatusCode::OK,
            Json(PrimaryResult {
                engine: "primary".to_string(),
                text: translated,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: format!("Translation failed: {}", e),
            }),
        )
            .into_response(),
    }
}

// GET /config
async fn get_config(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let config = state.config.lock().await;
    Json(config.clone()).into_response()
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
        }
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
            *config = new_config.clone();

            // Hot-reload: rebuild engine router with new config
            let new_router = engine::Router::new(&new_config);
            let mut router = state.engine_router.write().await;
            *router = new_router;

            Json(new_config).into_response()
        }
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

pub fn create_router(state: ApiState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/translate", post(translate))
        .route("/translate/primary", post(translate_primary))
        .route("/config", get(get_config).post(update_config))
        .route("/history", get(get_history))
        .route("/engines", get(get_engines))
        .layer(cors)
        .with_state(state)
}

pub async fn start_server(port: u16, state: ApiState) -> Result<(), String> {
    let app = create_router(state);
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

    println!("API server listening on http://{}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}
