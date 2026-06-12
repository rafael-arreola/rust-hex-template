use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::{SpanBackendWithUrl, TracingMiddleware};

/// Builds an HTTP client instrumented for distributed tracing.
///
/// Every outgoing request gets its own span (including method, URL and
/// status) and carries the active trace context via the W3C `traceparent`
/// header, so downstream services continue the same trace.
///
/// Driven adapters that call external services should receive this client
/// injected from `main.rs` instead of building their own.
pub fn instrumented_client() -> ClientWithMiddleware {
    ClientBuilder::new(reqwest::Client::new())
        .with(TracingMiddleware::<SpanBackendWithUrl>::new())
        .build()
}
