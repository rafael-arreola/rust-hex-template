use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use google_cloud_auth::{credentials, signer::Signer};
use google_cloud_storage::builder::storage::SignedUrlBuilder;
use google_cloud_storage::client::Storage;
use tokio::time;

use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::port::system::object_storage::ObjectStorage;

const MAX_OBJECT_NAME_BYTES: usize = 1024;
/// V4 signed URLs cannot outlive seven days.
const MAX_SIGNED_URL_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Technical handle for one bucket: object upload/download and V4 signed URLs.
///
/// Signing requires a service account identity (a key, impersonated ADC, or the metadata server
/// with `iam.serviceAccountTokenCreator` on itself); with plain user ADC the client still starts
/// and only `get_public_url` fails if called.
#[derive(Clone)]
pub struct GcsProvider {
    client: Storage,
    signer: Option<Signer>,
    bucket: String,
}

impl fmt::Debug for GcsProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("GcsProvider").field("bucket", &self.bucket).finish_non_exhaustive()
    }
}

impl GcsProvider {
    /// Builds the client from Application Default Credentials; it performs no object I/O.
    #[tracing::instrument(skip_all, fields(dependency = "gcs"))]
    pub async fn new(bucket: &str) -> DomainResult<Self> {
        let client = time::timeout(Duration::from_secs(10), Storage::builder().build())
            .await
            .map_err(|_| {
                DomainError::new(StatusCode::GATEWAY_TIMEOUT, "Cloud Storage client initialization")
                    .with_trace_code("GCS_CLIENT_INIT_TIMEOUT")
            })?
            .map_err(|_| {
                DomainError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Cloud Storage: Failed to initialize the client",
                )
                .with_trace_code("GCS_CLIENT_INIT_FAILED")
            })?;
        let signer = match credentials::Builder::default().build_signer() {
            Ok(signer) => Some(signer),
            Err(_) => {
                tracing::warn!(
                    "Cloud Storage URL signer unavailable with current ADC; signed URLs will fail"
                );
                None
            }
        };

        tracing::info!("Cloud Storage initialized");
        Ok(Self { client, signer, bucket: format!("projects/_/buckets/{bucket}") })
    }
}

#[async_trait]
impl ObjectStorage for GcsProvider {
    /// Uploads `content` as `path`, replacing any existing object with that name.
    #[tracing::instrument(skip_all, fields(dependency = "gcs", operation = "upload"))]
    async fn upload_file(
        &self,
        path: &str,
        content: Vec<u8>,
        content_type: &str,
    ) -> DomainResult<()> {
        let object = object_name(path)?;
        let mut request = self.client.write_object(&self.bucket, object, Bytes::from(content));
        let content_type = content_type.trim();
        if !content_type.is_empty() {
            request = request.set_content_type(content_type);
        }
        request.send_buffered().await.map_err(|error| failure("upload", &error))?;
        Ok(())
    }

    /// Downloads the whole object into memory; intended for small files, not streaming.
    /// A missing object is `NotFound`, distinct from provider failures.
    #[tracing::instrument(skip_all, fields(dependency = "gcs", operation = "download"))]
    async fn download_file(&self, path: &str) -> DomainResult<Vec<u8>> {
        let object = object_name(path)?;
        let mut response = self.client.read_object(&self.bucket, object).send().await.map_err(
            |error| match error.http_status_code() {
                Some(404) => DomainError::new(StatusCode::NOT_FOUND, "Storage object not found")
                    .with_public("Storage object not found")
                    .with_trace_code("GCS_OBJECT_NOT_FOUND"),
                _ => failure("download", &error),
            },
        )?;

        let mut content = Vec::new();
        while let Some(chunk) = response.next().await {
            content.extend_from_slice(&chunk.map_err(|error| failure("download", &error))?);
        }
        Ok(content)
    }

    /// Returns a V4 signed GET URL valid for `expires_in` (at most seven days). The URL grants
    /// access to anyone holding it until it expires; keep the lifetime as short as possible.
    /// `disposition` sets `response-content-disposition`, e.g. `attachment; filename="a.pdf"`.
    #[tracing::instrument(skip_all, fields(dependency = "gcs", operation = "sign_url"))]
    async fn get_public_url(
        &self,
        path: &str,
        expires_in: Duration,
        disposition: Option<&str>,
    ) -> DomainResult<String> {
        let object = object_name(path)?;
        if expires_in.is_zero() || expires_in > MAX_SIGNED_URL_TTL {
            return Err(DomainError::new(
                StatusCode::BAD_REQUEST,
                "Invalid expires_in: Signed URL lifetime must be between 1 second and 7 days",
            )
            .with_public(
                "Invalid expires_in: Signed URL lifetime must be between 1 second and 7 days",
            )
            .with_trace_code("GCS_INVALID_EXPIRES_IN"));
        }

        let signer = self.signer.as_ref().ok_or_else(|| {
            DomainError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Cloud Storage: Credentials cannot sign URLs",
            )
            .with_trace_code("GCS_SIGNER_UNAVAILABLE")
        })?;
        let mut builder =
            SignedUrlBuilder::for_object(&self.bucket, object).with_expiration(expires_in);
        if let Some(disposition) = disposition.map(str::trim).filter(|value| !value.is_empty()) {
            builder = builder.with_query_param("response-content-disposition", disposition);
        }
        builder.sign_with(signer).await.map_err(|_| {
            tracing::warn!(operation = "sign_url", "Cloud Storage request failed");
            DomainError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Cloud Storage: Failed to sign the object URL",
            )
            .with_trace_code("GCS_SIGN_URL_FAILED")
        })
    }
}

/// Object names are opaque to GCS, but empty, oversized or control-character names are rejected,
/// as are leading slashes that would create an empty first segment.
fn object_name(path: &str) -> DomainResult<&str> {
    let valid = !path.is_empty()
        && path.len() <= MAX_OBJECT_NAME_BYTES
        && !path.starts_with('/')
        && !path.chars().any(char::is_control)
        && !path.split('/').any(|segment| segment == "." || segment == "..");
    if valid {
        Ok(path)
    } else {
        Err(DomainError::new(
            StatusCode::BAD_REQUEST,
            "Invalid path: Storage object path is empty, too long or malformed",
        )
        .with_public("Invalid path: Storage object path is empty, too long or malformed")
        .with_trace_code("GCS_INVALID_OBJECT_PATH"))
    }
}

fn failure(operation: &'static str, error: &google_cloud_storage::Error) -> DomainError {
    let status = error.http_status_code();
    tracing::warn!(operation, status, "Cloud Storage request failed");
    DomainError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        format!("Cloud Storage: Object {operation} failed"),
    )
    .with_trace_code("GCS_OPERATION_FAILED")
}
