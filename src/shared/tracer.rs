pub mod format;

use crate::shared::config;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_gcloud_trace::GcpCloudTraceExporterBuilder;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

pub async fn init_tracing() -> anyhow::Result<()> {
    let config = config::get();
    let service_name = &config.service_name;
    let base_level = &config.debug_level;
    let project_id = &config.project_id;
    let app_env = &config.app_env;
    let version = env!("CARGO_PKG_VERSION");

    // W3C trace context propagation (traceparent/tracestate). Required so the
    // HTTP middleware can join traces propagated by upstream services.
    global::set_text_map_propagator(TraceContextPropagator::new());

    let env_filter = EnvFilter::new(format!(
        "h2=warn,hyper=warn,tokio_util=warn,tower_http=warn,rig=warn,axum=warn,{}",
        base_level
    ));

    match build_gcp_tracer(service_name, project_id, app_env, version).await {
        Ok(tracer) => {
            let cloud_logging_layer = tracing_subscriber::fmt::layer()
                .event_format(format::CloudLoggingFormat::new(project_id.clone()))
                .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new());

            let subscriber = tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .with(cloud_logging_layer);

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Setting default subscriber failed: {}", e))?;
        }
        Err(e) => {
            // Local fallback: plain fmt logs plus an in-process tracer with no
            // exporter, so every request span still carries a valid trace_id.
            let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
            let tracer = provider.tracer(service_name.clone());
            global::set_tracer_provider(provider);

            let subscriber = tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer())
                .with(tracing_opentelemetry::layer().with_tracer(tracer));

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Setting default subscriber failed: {}", e))?;

            tracing::warn!(
                "GCP trace exporter unavailable ({}); falling back to local fmt logging",
                e
            );
        }
    }

    Ok(())
}

async fn build_gcp_tracer(
    service_name: &str,
    project_id: &str,
    app_env: &str,
    version: &str,
) -> anyhow::Result<opentelemetry_sdk::trace::Tracer> {
    let builder = GcpCloudTraceExporterBuilder::for_default_project_id()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize GCP exporter builder: {}", e))?
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_attributes(vec![
                    opentelemetry::KeyValue::new("service.name", service_name.to_string()),
                    opentelemetry::KeyValue::new("service.version", version.to_string()),
                    opentelemetry::KeyValue::new("deployment.environment", app_env.to_string()),
                    opentelemetry::KeyValue::new("project.id", project_id.to_string()),
                ])
                .build(),
        );

    let provider = builder
        .create_provider()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create tracer provider: {}", e))?;

    let tracer = builder
        .install(&provider)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create tracer: {}", e))?;

    // Register as the global provider so instrumented libraries (e.g. the
    // MongoDB driver) attach their spans to the same traces.
    global::set_tracer_provider(provider);

    Ok(tracer)
}
