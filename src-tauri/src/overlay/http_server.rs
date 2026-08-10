use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::html_builder;

/// Shared state for the overlay HTTP server.
/// Holds the current overlay HTML content that gets served to the webview.
pub struct OverlayServerState {
    /// Current overlay HTML (full document)
    pub html: Arc<RwLock<String>>,
}

impl Default for OverlayServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayServerState {
    pub fn new() -> Self {
        Self {
            html: Arc::new(RwLock::new(html_builder::build_shell_html())),
        }
    }

    /// Update the HTML content served by the HTTP server.
    pub async fn set_html(&self, html: String) {
        *self.html.write().await = html;
    }

    /// Get the current HTML content.
    pub async fn get_html(&self) -> String {
        self.html.read().await.clone()
    }
}

/// Overlay HTTP server handle. Keeps the server running and provides
/// the base URL for overlay windows to load from.
pub struct OverlayHttpServer {
    /// Base URL like "<http://127.0.0.1:19830>"
    pub base_url: String,
    /// Port the server is listening on
    pub port: u16,
    /// Shared state
    pub state: Arc<OverlayServerState>,
}

impl OverlayHttpServer {
    /// Start the overlay HTTP server on an available port.
    /// Returns the server handle with the base URL.
    pub async fn start() -> Result<Self, String> {
        let state = Arc::new(OverlayServerState::new());

        // Find an available port
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|e| format!("Failed to bind: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get port: {e}"))?
            .port();

        // Drop the listener so axum can bind to the same port
        drop(listener);

        let state_clone = Arc::clone(&state);
        let base_url = format!("http://127.0.0.1:{port}");

        // Spawn the axum server
        tokio::spawn(async move {
            // Restrict CORS to localhost origins only (this is a local overlay server)
            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::predicate(
                    |origin, _parts| {
                        let o = origin.as_bytes();
                        o.starts_with(b"http://localhost")
                            || o.starts_with(b"http://127.0.0.1")
                            || o.starts_with(b"tauri://localhost")
                    },
                ))
                .allow_methods(tower_http::cors::Any)
                .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::ACCEPT]);

            let app = axum::Router::new()
                .route(
                    "/overlay",
                    axum::routing::get({
                        let state = Arc::clone(&state_clone);
                        move || {
                            let state = Arc::clone(&state);
                            async move {
                                let html = state.get_html().await;
                                axum::response::Html(html)
                            }
                        }
                    }),
                )
                .route(
                    "/overlay/content",
                    axum::routing::get({
                        let state = Arc::clone(&state_clone);
                        move || {
                            let state = Arc::clone(&state);
                            async move {
                                let html = state.get_html().await;
                                axum::response::Html(html)
                            }
                        }
                    }),
                )
                .layer(cors);

            let addr = format!("127.0.0.1:{port}");
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Overlay HTTP server failed to bind: {}", e);
                    return;
                },
            };

            tracing::info!("Overlay HTTP server listening on {}", addr);

            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Overlay HTTP server error: {}", e);
            }
        });

        Ok(Self {
            base_url,
            port,
            state,
        })
    }

    /// Get the URL for the overlay webview to load.
    pub fn overlay_url(&self) -> String {
        format!("{}/overlay", self.base_url)
    }

    /// Update the overlay HTML content. The webview will need to
    /// navigate or `eval()` to pick up the new content.
    pub async fn update_html(&self, html: String) {
        self.state.set_html(html).await;
    }
}
