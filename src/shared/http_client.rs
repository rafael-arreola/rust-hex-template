use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::{SpanBackendWithUrl, TracingMiddleware};
use std::time::Duration;

/// Time budget for a whole outbound request (connect + send + response).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Time budget for establishing the TCP/TLS connection alone.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Builds an HTTP client instrumented for distributed tracing.
///
/// Every outgoing request gets its own span (including method, URL and
/// status) and carries the active trace context via the W3C `traceparent`
/// header, so downstream services continue the same trace.
///
/// Driven adapters that call external services should receive this client
/// injected from `main.rs` instead of building their own.
pub fn instrumented_client() -> ClientWithMiddleware {
    client_with_timeout(DEFAULT_TIMEOUT)
}

/// Same as [`instrumented_client`], with an explicit per-request budget for
/// dependencies that are legitimately slower (or must be stricter).
///
/// `reqwest` applies **no timeout by default**: a dependency that accepts the
/// connection and then goes silent would pin the calling task forever, so
/// every client this template hands out sets one.
pub fn client_with_timeout(timeout: Duration) -> ClientWithMiddleware {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .build()
        // The builder only fails on a broken TLS backend, which would equally
        // break `Client::new()`; fall back rather than propagate to callers.
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build HTTP client with timeouts: {e}");
            reqwest::Client::new()
        });

    ClientBuilder::new(client).with(TracingMiddleware::<SpanBackendWithUrl>::new()).build()
}
