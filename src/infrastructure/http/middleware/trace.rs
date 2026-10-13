use axum::{
    body::Body,
    extract::MatchedPath,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use opentelemetry::Context;
use opentelemetry::propagation::{Extractor, TextMapPropagator};
use opentelemetry::trace::{
    FutureExt as OtelFutureExt, SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId,
    TraceState,
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::time::Instant;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Opens the request span; W3C/Cloud Trace context is the only correlation identifier.
pub async fn trace_context(req: Request<Body>, next: Next) -> Response {
    let method = match req.method().as_str() {
        method @ ("GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "CONNECT" | "OPTIONS" | "TRACE"
        | "PATCH") => method,
        _ => "OTHER",
    };
    let route = req.extensions().get::<MatchedPath>().map_or("<unmatched>", MatchedPath::as_str);
    let span = tracing::info_span!(
        "http.request",
        otel.kind = "server",
        http.request.method = %method,
        http.route = %route,
        http.response.status_code = tracing::field::Empty,
        http.server.duration_ms = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
    );

    // A missing remote parent starts a root request trace, not a child of a bootstrap span.
    let parent = extract_remote_context(req.headers()).unwrap_or_default();
    if let Err(error) = span.set_parent(parent) {
        tracing::error!(%error, "Failed to attach the request trace context");
    }

    let otel_context = span.context();
    let request_span = span.clone();
    async move {
        let started = Instant::now();
        let response = next.run(req).await;

        let status = response.status();
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        request_span.record("http.response.status_code", status.as_u16());
        request_span.record("http.server.duration_ms", elapsed_ms);
        if status.is_server_error() {
            request_span.record("otel.status_code", "ERROR");
        }
        tracing::info!(status = status.as_u16(), elapsed_ms, "HTTP request completed");

        response
    }
    .with_context(otel_context)
    .instrument(span)
    .await
}

// OpenTelemetry borrows headers for this extraction only; cloning every header would add
// per-request work without changing ownership or preventing a lifetime from escaping.
struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        let mut values = self.0.get_all(key).iter();
        let value = values.next()?;
        if values.next().is_some() { None } else { value.to_str().ok() }
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|key| key.as_str()).collect()
    }
}

fn extract_remote_context(headers: &HeaderMap) -> Option<Context> {
    let context = TraceContextPropagator::new()
        .extract_with_context(&Context::new(), &HeaderExtractor(headers));
    if context.span().span_context().is_valid() {
        return Some(context);
    }
    parse_cloud_trace_context(headers)
}

fn parse_cloud_trace_context(headers: &HeaderMap) -> Option<Context> {
    let extractor = HeaderExtractor(headers);
    let value = extractor.get("x-cloud-trace-context")?;
    let (trace_id_hex, rest) = value.split_once('/')?;
    if trace_id_hex.len() != 32 || value.len() > 128 {
        return None;
    }
    let mut options = rest.split(';');
    let span_id_decimal = options.next()?;
    if !span_id_decimal.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut sampled = None;
    for option in options {
        if let Some(value) = option.trim().strip_prefix("o=") {
            if sampled.is_some() {
                return None;
            }
            sampled = Some(match value {
                "0" => false,
                "1" => true,
                _ => return None,
            });
        }
    }

    let trace_id = TraceId::from_hex(trace_id_hex).ok()?;
    let span_id_number: u64 = span_id_decimal.parse().ok()?;
    if trace_id == TraceId::INVALID || span_id_number == 0 {
        return None;
    }
    let flags = if sampled == Some(true) { TraceFlags::SAMPLED } else { TraceFlags::default() };
    let span_context = SpanContext::new(
        trace_id,
        SpanId::from_bytes(span_id_number.to_be_bytes()),
        flags,
        true,
        TraceState::default(),
    );
    Some(Context::new().with_remote_span_context(span_context))
}
