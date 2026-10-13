//! Axum transport kit. This module starts the server; its public surface is re-exported here so
//! endpoints never depend on the kit's internal layout.

mod auth;
mod context;
mod error;
mod format;
pub mod health;
mod middleware;
mod request;
mod response;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::{net::TcpListener, signal, sync::oneshot, time};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowHeaders, Any, CorsLayer},
    decompression::RequestDecompressionLayer,
};

use crate::domain::core::error::{DomainError, DomainResult};
use error::HttpRejection;

pub use auth::{Policy, RequirePermissionExt, auth_middleware};
pub use context::ContextRequestExt;
pub use error::ApiError;
pub use request::{PaginationQuery, PathParam, ValidatedBody, ValidatedQuery};
pub use response::{ApiResponse, Paginated, paginated};

/// Transport settings supplied by the composition root; defaults live in configuration only.
pub struct ServerOptions {
    pub port: u16,
    pub drain_timeout: Duration,
    pub request_timeout: Duration,
    pub body_limit: usize,
    /// Disables response negotiation only; the validating extractor still accepts MessagePack.
    pub msgpack_enabled: bool,
}

/// Transport-only launcher. The composition root supplies a router with its state already bound.
pub struct ServerLauncher {
    router: Router,
    options: ServerOptions,
}

impl ServerLauncher {
    pub fn new(router: Router, options: ServerOptions) -> Self {
        Self { router, options }
    }

    /// Signal registration, bind and serving errors reach the process root.
    #[tracing::instrument(skip_all)]
    pub async fn run(self) -> DomainResult<()> {
        let Self { router, options } = self;
        // Origin policy belongs to the edge (load balancer). Reflecting preflight headers, rather
        // than using '*', still admits Authorization under Fetch.
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(AllowHeaders::mirror_request());

        // Execution: trace -> compression -> CORS -> negotiation -> timeout -> encoding check
        // -> decompression -> body limit -> handler/fallback. The limit applies to the decoded
        // body when an extractor reads it, after authentication; streaming routes need their own
        // composition. CORS also decorates synthetic negotiation/timeout errors.
        let mut app = router
            .fallback(not_found)
            .method_not_allowed_fallback(method_not_allowed)
            .layer(DefaultBodyLimit::max(options.body_limit))
            .layer(RequestDecompressionLayer::new())
            .layer(axum::middleware::from_fn(middleware::content_encoding))
            .layer(axum::middleware::from_fn_with_state(
                options.request_timeout,
                middleware::request_timeout,
            ));

        if options.msgpack_enabled {
            app = app.layer(axum::middleware::from_fn(format::negotiation));
        }
        let app = app
            .layer(cors)
            .layer(CompressionLayer::new())
            .layer(axum::middleware::from_fn(middleware::trace_context));

        let shutdown = shutdown_signal()?;
        let address = SocketAddr::from(([0, 0, 0, 0], options.port));
        let listener = TcpListener::bind(address).await.map_err(|error| {
            DomainError::internal(format!("Failed to bind HTTP listener: {error}"))
        })?;
        let bound_address = listener.local_addr().map_err(|error| {
            DomainError::internal(format!("Failed to inspect HTTP listener address: {error}"))
        })?;
        tracing::info!(%bound_address, drain_timeout = ?options.drain_timeout, "HTTP server listening");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                // Sender cancellation also closes the listener when the launcher is dropped.
                match shutdown_rx.await {
                    Ok(()) => {}
                    Err(_) => tracing::debug!("HTTP launcher dropped before shutdown notification"),
                }
            })
            .into_future();
        tokio::pin!(server);

        tokio::select! {
            result = &mut server => {
                return result.map_err(|error| {
                    DomainError::internal(format!("HTTP server failed: {error}"))
                });
            }
            result = shutdown => result?,
        }

        health::start_draining();
        tracing::info!("Shutdown signal received, draining HTTP connections");
        shutdown_tx.send(()).map_err(|_| {
            DomainError::internal("HTTP server closed before receiving the shutdown notification")
        })?;

        match time::timeout(options.drain_timeout, &mut server).await {
            Ok(result) => {
                result.map_err(|error| {
                    DomainError::internal(format!("HTTP server failed during drain: {error}"))
                })?;
                tracing::info!("HTTP shutdown complete");
            }
            Err(_) => tracing::warn!(
                drain_timeout = ?options.drain_timeout,
                "HTTP drain budget exhausted; process shutdown must terminate remaining connections",
            ),
        }
        Ok(())
    }
}

async fn not_found() -> ApiError {
    HttpRejection::RouteNotFound.into()
}

/// Axum's MethodRouter supplies `Allow`; this handler must not replace or invent that header.
async fn method_not_allowed() -> ApiError {
    HttpRejection::MethodNotAllowed.into()
}

#[cfg(unix)]
fn shutdown_signal() -> DomainResult<impl Future<Output = DomainResult<()>>> {
    let mut interrupt =
        signal::unix::signal(signal::unix::SignalKind::interrupt()).map_err(|error| {
            DomainError::internal(format!("Failed to install SIGINT handler: {error}"))
        })?;
    let mut terminate =
        signal::unix::signal(signal::unix::SignalKind::terminate()).map_err(|error| {
            DomainError::internal(format!("Failed to install SIGTERM handler: {error}"))
        })?;
    Ok(async move {
        let received = tokio::select! {
            received = interrupt.recv() => received,
            received = terminate.recv() => received,
        };
        received.ok_or_else(|| DomainError::internal("Shutdown signal stream closed unexpectedly"))
    })
}

#[cfg(not(unix))]
fn shutdown_signal() -> DomainResult<impl Future<Output = DomainResult<()>>> {
    Ok(async {
        signal::ctrl_c().await.map_err(|error| {
            DomainError::internal(format!("Failed to install Ctrl+C handler: {error}"))
        })
    })
}
