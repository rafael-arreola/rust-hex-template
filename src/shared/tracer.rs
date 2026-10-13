pub mod format;

use crate::shared::envs;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_gcloud_trace::GcpCloudTraceExporterBuilder;
use opentelemetry_sdk::{Resource, error, propagation, trace};
use std::time::Duration;
use tokio::time;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

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
        .event_format(format::CloudLoggingFormat::new(env.project_id.clone()))
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
