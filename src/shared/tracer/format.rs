//! Structured JSON event formatter for GCP Cloud Logging.
//!
//! Replaces `tracing-stackdriver`, which pins `tracing-opentelemetry 0.23`
//! internally and therefore can no longer read the OTel context of spans
//! created by the 0.32 layer (the `logging.googleapis.com/trace` field was
//! silently dropped). This formatter reads `OtelData` from the same
//! `tracing-opentelemetry` version as the rest of the stack, so log↔trace
//! correlation stays consistent with the spans exported to Cloud Trace.

use serde_json::{Map, Value, json};
use std::fmt;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

pub struct CloudLoggingFormat {
    project_id: String,
}

impl CloudLoggingFormat {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self { project_id: project_id.into() }
    }
}

impl<S, N> FormatEvent<S, N> for CloudLoggingFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        let mut entry = Map::new();

        // Span fields first (root → leaf, so inner spans win), then event
        // fields, then the reserved Cloud Logging keys override everything.
        if let Some(scope) = context.event_scope() {
            for span in scope.from_root() {
                if let Some(formatted) = span.extensions().get::<FormattedFields<N>>()
                    && let Ok(Value::Object(fields)) = serde_json::from_str(formatted)
                {
                    entry.extend(fields);
                }
            }
        }

        event.record(&mut JsonVisitor(&mut entry));

        entry.insert(
            "time".to_string(),
            Value::String(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)),
        );
        entry.insert("severity".to_string(), Value::String(severity(metadata.level()).into()));
        entry.insert("target".to_string(), Value::String(metadata.target().to_string()));

        if let (Some(file), Some(line)) = (metadata.file(), metadata.line()) {
            entry.insert(
                "logging.googleapis.com/sourceLocation".to_string(),
                json!({ "file": file, "line": line.to_string() }),
            );
        }

        // Trace correlation: nearest span in scope with a built OTel context.
        if let Some(scope) = context.event_scope() {
            for span in scope {
                let extensions = span.extensions();
                let Some(otel_data) = extensions.get::<tracing_opentelemetry::OtelData>() else {
                    continue;
                };
                if let Some(trace_id) = otel_data.trace_id() {
                    entry.insert(
                        "logging.googleapis.com/trace".to_string(),
                        Value::String(format!("projects/{}/traces/{}", self.project_id, trace_id)),
                    );
                    entry.insert("logging.googleapis.com/trace_sampled".to_string(), true.into());
                    if let Some(span_id) = otel_data.span_id() {
                        entry.insert(
                            "logging.googleapis.com/spanId".to_string(),
                            Value::String(span_id.to_string()),
                        );
                    }
                    break;
                }
            }
        }

        let line = serde_json::to_string(&Value::Object(entry)).map_err(|_| fmt::Error)?;
        writeln!(writer, "{}", line)
    }
}

fn severity(level: &Level) -> &'static str {
    match *level {
        Level::TRACE | Level::DEBUG => "DEBUG",
        Level::INFO => "INFO",
        Level::WARN => "WARNING",
        Level::ERROR => "ERROR",
    }
}

struct JsonVisitor<'a>(&'a mut Map<String, Value>);

impl tracing::field::Visit for JsonVisitor<'_> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), value.into());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), value.into());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), value.into());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), value.into());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.into());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(field.name().to_string(), value.to_string().into());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().to_string(), format!("{:?}", value).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::TracerProvider as _;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    #[derive(Clone, Default)]
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBuffer {
        type Writer = SharedBuffer;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn emits_cloud_logging_fields_with_trace_correlation() {
        let buffer = SharedBuffer::default();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let tracer = provider.tracer("test");

        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(CloudLoggingFormat::new("test-project"))
                    .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new())
                    .with_writer(buffer.clone()),
            );

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("http.request", request_id = "req-123");
            let _guard = span.enter();
            tracing::info!(user_count = 5, "hello world");
        });

        let output = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        let entry: Value = serde_json::from_str(output.lines().next().unwrap()).unwrap();

        assert_eq!(entry["severity"], "INFO");
        assert_eq!(entry["message"], "hello world");
        assert_eq!(entry["user_count"], 5);
        assert_eq!(entry["request_id"], "req-123");
        assert_eq!(entry["logging.googleapis.com/trace_sampled"], true);
        assert!(entry["logging.googleapis.com/sourceLocation"]["file"].is_string());

        let trace = entry["logging.googleapis.com/trace"].as_str().unwrap();
        assert!(trace.starts_with("projects/test-project/traces/"));
        assert!(!trace.ends_with("00000000000000000000000000000000"));
        assert!(entry["logging.googleapis.com/spanId"].is_string());
    }
}
