use std::fmt;
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::trace::{TraceContextExt as _, TracerProvider as _};
use opentelemetry_gcloud_trace::GcpCloudTraceExporterBuilder;
use opentelemetry_sdk::{Resource, error, propagation, trace};
use serde_json::{Map, Value, json};
use tokio::time;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

use crate::shared::envs;

pub struct TracerGuard {
    provider: trace::SdkTracerProvider,
}

impl TracerGuard {
    pub fn shutdown(self) -> error::OTelSdkResult {
        self.provider.shutdown()
    }
}

pub async fn init_tracing() -> Result<TracerGuard, &'static str> {
    let env = envs::get();
    let env_filter = EnvFilter::try_new(format!(
        "h2=warn,hyper=warn,tokio_util=warn,tower_http=warn,rig=warn,axum=warn,{}",
        env.debug_level
    ))
    .map_err(|_| "Invalid environment variable DEBUG_LEVEL")?;

    let mut resource_builder = Resource::builder_empty().with_attributes([
        opentelemetry::KeyValue::new("service.name", env.service_name.clone()),
        opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        opentelemetry::KeyValue::new("deployment.environment", env.service_env.clone()),
    ]);
    if !env.project_id.is_empty() {
        resource_builder = resource_builder
            .with_attribute(opentelemetry::KeyValue::new("project.id", env.project_id.clone()));
    }
    let resource = resource_builder.build();

    let (provider, exporter_failed) = if env.project_id.is_empty() {
        (provider_builder(resource).build(), false)
    } else {
        let exporter_builder = GcpCloudTraceExporterBuilder::new(env.project_id.clone())
            .with_resource(resource.clone());
        let exporter =
            exporter_builder.create_provider_from_builder(provider_builder(resource.clone()));
        match time::timeout(Duration::from_secs(10), exporter).await {
            Ok(Ok(provider)) => (provider, false),
            Ok(Err(_)) | Err(_) => (provider_builder(resource).build(), true),
        }
    };
    let tracer = provider.tracer(env.service_name.clone());
    let cloud_logging_layer = tracing_subscriber::fmt::layer()
        .event_format(CloudLoggingFormat { project_id: env.project_id.clone() })
        .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new());

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(cloud_logging_layer);

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        provider
            .shutdown()
            .map_err(|_| "Failed to initialize tracing subscriber and shut down tracer provider")?;
        return Err("Failed to initialize tracing subscriber");
    }

    global::set_text_map_propagator(propagation::TraceContextPropagator::new());
    global::set_tracer_provider(provider.clone());

    if exporter_failed {
        tracing::warn!("Cloud Trace exporter initialization failed; using local tracing");
    }

    Ok(TracerGuard { provider })
}

fn provider_builder(resource: Resource) -> trace::TracerProviderBuilder {
    trace::SdkTracerProvider::builder()
        .with_resource(resource)
        .with_sampler(trace::Sampler::ParentBased(Box::new(trace::Sampler::AlwaysOn)))
}

struct CloudLoggingFormat {
    project_id: String,
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
        writeln!(writer, "{line}")
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
        let formatted = format!("{:?}", value);
        let json_value = if field.name() == "error.breadcrumb" {
            serde_json::from_str::<Value>(&formatted).unwrap_or(Value::String(formatted))
        } else {
            Value::String(formatted)
        };
        self.0.insert(field.name().to_string(), json_value);
    }
}

/// Serializes a sanitized projection safely for structured tracing logs.
///
/// If serialization fails, returns a JSON object with the error detail instead of panicking.
#[macro_export]
macro_rules! as_json {
    ($val:expr) => {
        &serde_json::to_string($val)
            .unwrap_or_else(|e| format!("{{\"error\":\"Serialization failed: {}\"}}", e))
    };
}
