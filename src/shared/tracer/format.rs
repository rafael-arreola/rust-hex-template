use opentelemetry::trace::TraceContextExt as _;
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

        if let Some(scope) = context.event_scope() {
            for span in scope.from_root() {
                if let Some(formatted) = span.extensions().get::<FormattedFields<N>>() {
                    let fields: Map<String, Value> =
                        serde_json::from_str(formatted).map_err(|_| fmt::Error)?;
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

        let otel_context = opentelemetry::Context::current();
        let otel_span = otel_context.span();
        let span_context = otel_span.span_context();

        if span_context.is_valid() {
            entry
                .insert("trace_id".to_string(), Value::String(span_context.trace_id().to_string()));
            entry.insert("span_id".to_string(), Value::String(span_context.span_id().to_string()));
            entry.insert("trace_sampled".to_string(), span_context.is_sampled().into());

            if !self.project_id.is_empty() {
                entry.insert(
                    "logging.googleapis.com/trace".to_string(),
                    Value::String(format!(
                        "projects/{}/traces/{}",
                        self.project_id,
                        span_context.trace_id()
                    )),
                );
                entry.insert(
                    "logging.googleapis.com/trace_sampled".to_string(),
                    span_context.is_sampled().into(),
                );
                entry.insert(
                    "logging.googleapis.com/spanId".to_string(),
                    Value::String(span_context.span_id().to_string()),
                );
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
