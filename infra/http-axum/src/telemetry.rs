use opentelemetry_gcloud_trace::GcpCloudTraceExporterBuilder;
use tracing_stackdriver::CloudTraceConfiguration;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

use crate::config;

pub async fn init_tracing() -> anyhow::Result<()> {
    let config = config::get();
    let service_name = &config.service_name;
    let base_level = &config.debug_level;
    let project_id = &config.project_id;
    let app_env = &config.app_env;
    let version = env!("CARGO_PKG_VERSION");

    let env_filter = EnvFilter::new(format!(
        "h2=warn,hyper=warn,tokio_util=warn,tower_http=warn,rig=warn,axum=warn,{}",
        base_level
    ));

    let stackdriver_layer = Some(tracing_stackdriver::layer().with_cloud_trace(
        CloudTraceConfiguration {
            project_id: project_id.clone(),
        },
    ));

    let builder = GcpCloudTraceExporterBuilder::for_default_project_id()
        .await
        .map_err(|e| {
            tracing::warn!("Failed to initialize GCP exporter builder: {}", e);
            e
        })?
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

    let provider = builder.create_provider().await.map_err(|e| {
        let err = format!("Failed to create tracer provider: {}", e);
        tracing::warn!("{}", err);
        anyhow::anyhow!(err)
    })?;

    let tracer_provider: opentelemetry_sdk::trace::Tracer =
        builder.install(&provider).await.map_err(|e| {
            let err = format!("Failed to install tracer: {}", e);
            tracing::warn!("{}", err);
            anyhow::anyhow!(err)
        })?;

    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer_provider);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stackdriver_layer)
        .with(telemetry_layer);

    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        let err = format!("Setting default subscriber failed: {}", e);
        tracing::warn!("{}", err);
        anyhow::anyhow!(err)
    })?;

    Ok(())
}
