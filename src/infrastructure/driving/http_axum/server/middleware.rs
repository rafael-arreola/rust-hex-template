use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    response::Response,
};
use opentelemetry::Context;
use opentelemetry::propagation::Extractor;
use opentelemetry::trace::{
    FutureExt as OtelFutureExt, SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId,
    TraceState,
};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Per-request trace context middleware.
///
/// - Extracts the remote trace context using the W3C `traceparent` standard
///   (via the global OpenTelemetry propagator), falling back to GCP's
///   `X-Cloud-Trace-Context` header.
/// - If a remote context is present, the request span joins that trace;
///   otherwise the OpenTelemetry layer assigns a fresh `trace_id`.
/// - Propagates or generates an `X-Request-Id` (UUIDv7) and records it as a
///   declared field on the request span so it lands in every log line.
pub async fn trace_context(mut req: Request<Body>, next: axum::middleware::Next) -> Response<Body> {
    let header_name = HeaderName::from_static("x-request-id");

    let request_id = req
        .headers()
        .get(&header_name)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    req.extensions_mut().insert(RequestId(request_id.clone()));

    let span = tracing::info_span!(
        "http.request",
        http.method = %req.method(),
        http.target = %req.uri().path(),
        request_id = %request_id,
    );

    if let Some(remote_context) = extract_remote_context(req.headers())
        && let Err(e) = span.set_parent(remote_context)
    {
        tracing::debug!("Failed to attach remote trace context: {}", e);
    }

    // Attach the span's OTel context as the task-local current context so
    // natively instrumented clients (e.g. the MongoDB driver) parent their
    // spans to this request's trace.
    let otel_context = span.context();
    let mut response = next.run(req).with_context(otel_context).instrument(span).await;

    if response.status() != StatusCode::NOT_MODIFIED {
        response.headers_mut().insert(
            header_name,
            HeaderValue::from_str(&request_id).unwrap_or(HeaderValue::from_static("unknown")),
        );
    }

    response
}

/// Newtype stored in request extensions so downstream handlers can retrieve it.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|key| key.as_str()).collect()
    }
}

/// Returns the remote trace context carried by the request headers, or `None`
/// when absent/invalid (in which case the span starts a brand-new trace).
fn extract_remote_context(headers: &HeaderMap) -> Option<Context> {
    let context = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(headers))
    });

    if context.span().span_context().is_valid() {
        return Some(context);
    }

    parse_cloud_trace_context(headers)
}

/// Fallback for GCP's legacy `X-Cloud-Trace-Context: TRACE_ID/SPAN_ID;o=1`
/// header, still emitted by Google load balancers alongside `traceparent`.
fn parse_cloud_trace_context(headers: &HeaderMap) -> Option<Context> {
    let value = headers.get("x-cloud-trace-context")?.to_str().ok()?;

    let (trace_id_hex, rest) = value.split_once('/')?;
    let span_id_decimal = rest.split(';').next()?;

    let trace_id = TraceId::from_hex(trace_id_hex).ok()?;
    let span_id_number: u64 = span_id_decimal.parse().ok()?;
    if trace_id == TraceId::INVALID || span_id_number == 0 {
        return None;
    }

    let span_context = SpanContext::new(
        trace_id,
        SpanId::from_bytes(span_id_number.to_be_bytes()),
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    );

    Some(Context::new().with_remote_span_context(span_context))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-cloud-trace-context", HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn parses_valid_cloud_trace_context() {
        let headers = headers_with("105445aa7843bc8bf206b12000100000/1;o=1");
        let context = parse_cloud_trace_context(&headers).expect("should parse");
        let binding = context.span();
        let span_context = binding.span_context();

        assert!(span_context.is_valid());
        assert!(span_context.is_remote());
        assert_eq!(span_context.trace_id().to_string(), "105445aa7843bc8bf206b12000100000");
    }

    #[test]
    fn parses_without_options_suffix() {
        let headers = headers_with("105445aa7843bc8bf206b12000100000/42");
        assert!(parse_cloud_trace_context(&headers).is_some());
    }

    #[test]
    fn rejects_malformed_values() {
        for value in ["not-a-trace", "shorthex/1;o=1", "105445aa7843bc8bf206b12000100000/0"] {
            assert!(parse_cloud_trace_context(&headers_with(value)).is_none(), "{value}");
        }
        assert!(parse_cloud_trace_context(&HeaderMap::new()).is_none());
    }
}
