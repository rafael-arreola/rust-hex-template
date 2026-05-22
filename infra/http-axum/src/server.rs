use axum::{Router, extract::DefaultBodyLimit};
use std::net::SocketAddr;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    decompression::RequestDecompressionLayer,
    trace::TraceLayer,
};

use crate::config;
use crate::routes;
use crate::state::AppState;

pub struct ServerLauncher {
    state: AppState,
    http_port: Option<u16>,
}

impl ServerLauncher {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            http_port: None,
        }
    }

    pub fn with_http(mut self, port: u16) -> Self {
        self.http_port = Some(port);
        self
    }

    pub async fn run(self) {
        let env = config::get();

        if let Some(port) = self.http_port {
            let state = self.state.clone();

            let cors = if env.cors_origins == "*" {
                CorsLayer::permissive()
                    .allow_methods(Any)
                    .allow_headers(Any)
            } else {
                let origins: Vec<_> = env
                    .cors_origins
                    .split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();

                CorsLayer::new()
                    .allow_methods(Any)
                    .allow_headers(Any)
                    .allow_origin(origins)
            };

            let rest_router = Router::new()
                .nest("/api/v1", routes::app_router())
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(RequestDecompressionLayer::new())
                .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
                .layer(cors)
                .with_state(state);

            let rest_addr = SocketAddr::from(([0, 0, 0, 0], port));
            tracing::info!("REST Server listening on {}", rest_addr);

            match tokio::net::TcpListener::bind(rest_addr).await {
                Ok(listener) => {
                    if let Err(e) = axum::serve(listener, rest_router)
                        .with_graceful_shutdown(shutdown_signal("REST"))
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

async fn shutdown_signal(name: &str) {
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
        "Signal received, starting graceful shutdown for {}...",
        name
    );
}
