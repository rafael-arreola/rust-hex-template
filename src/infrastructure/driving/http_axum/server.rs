pub mod error;
pub mod health;
pub mod middleware;
pub mod response;
pub mod state;
pub mod validation;

use axum::Router;
use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Request, Response, header},
    routing::get,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    decompression::RequestDecompressionLayer,
    trace::TraceLayer,
};

use self::health::{healthz, readyz};
use self::state::AppState;
use crate::infrastructure::driving::http_axum::routes;

#[derive(Clone)]
pub struct ServerLauncher {
    state: AppState,
    http_port: Option<u16>,
    cors_origins: Option<String>,
    drain_timeout: Duration,
}

impl ServerLauncher {
    pub fn new(state: AppState) -> Self {
        Self { state, http_port: None, cors_origins: None, drain_timeout: Duration::from_secs(10) }
    }

    pub fn with_http(mut self, port: u16) -> Self {
        self.http_port = Some(port);
        self
    }

    pub fn with_cors_origins(mut self, cors_origins: String) -> Self {
        self.cors_origins = Some(cors_origins);
        self
    }

    pub fn with_drain_timeout(mut self, secs: u64) -> Self {
        self.drain_timeout = Duration::from_secs(secs);
        self
    }

    pub async fn run(self) {
        if let Some(port) = self.http_port {
            let state = self.state.clone();
            let drain_timeout = self.drain_timeout;

            let cors_origins_str = self.cors_origins.unwrap_or_else(|| "*".to_string());

            let cors = if cors_origins_str == "*" {
                CorsLayer::permissive().allow_methods(Any).allow_headers(Any)
            } else {
                let origins: Vec<_> =
                    cors_origins_str.split(',').filter_map(|s| s.parse().ok()).collect();

                CorsLayer::new().allow_methods(Any).allow_headers(Any).allow_origin(origins)
            };

            let rest_router = Router::new()
                .route("/healthz", get(healthz))
                .route("/readyz", get(readyz))
                .nest("/api/v1", routes::app_router())
                .layer(axum::middleware::from_fn(msgpack_negotiation))
                .layer(axum::middleware::from_fn(middleware::request_id))
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(RequestDecompressionLayer::new())
                .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
                .layer(cors)
                .with_state(state);

            let rest_addr = SocketAddr::from(([0, 0, 0, 0], port));
            tracing::info!(
                "REST Server listening on {} (drain_timeout={:?})",
                rest_addr,
                drain_timeout,
            );

            match tokio::net::TcpListener::bind(rest_addr).await {
                Ok(listener) => {
                    if let Err(e) = axum::serve(listener, rest_router)
                        .with_graceful_shutdown(shutdown_signal("REST", drain_timeout))
                        .await
                    {
                        tracing::error!("Server error: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to bind to {}: {}", rest_addr, e);
                }
            }
        }
    }
}

pub async fn msgpack_negotiation(
    req: Request<Body>,
    next: axum::middleware::Next,
) -> Response<Body> {
    let accept = req.headers().get(header::ACCEPT).and_then(|h| h.to_str().ok()).unwrap_or("");

    let wants_msgpack = accept.contains("application/vnd.msgpack");

    let response = next.run(req).await;

    if wants_msgpack && response.status().is_success() {
        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.contains("application/json"))
            .unwrap_or(false);

        if is_json {
            let (mut parts, body) = response.into_parts();
            if let Ok(bytes) = axum::body::to_bytes(body, 10 * 1024 * 1024).await {
                if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Ok(msgpack_bytes) = rmp_serde::to_vec(&json_val) {
                        parts.headers.insert(
                            header::CONTENT_TYPE,
                            HeaderValue::from_static("application/vnd.msgpack"),
                        );
                        return Response::from_parts(parts, Body::from(msgpack_bytes));
                    }
                }
                return Response::from_parts(parts, Body::from(bytes));
            } else {
                return Response::from_parts(parts, Body::empty());
            }
        }
    }

    response
}

async fn shutdown_signal(name: &str, drain_timeout: Duration) {
    let ctrl_c = async {
        match signal::ctrl_c().await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to install Ctrl+C handler: {}", e);
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!(
        "Signal received, draining for {:?} before shutdown for {}...",
        drain_timeout,
        name,
    );
    tokio::time::sleep(drain_timeout).await;
    tracing::info!("Shutdown complete for {}", name);
}
