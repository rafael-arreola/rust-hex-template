use std::collections::BTreeMap;
use std::fmt;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use bytes::Bytes;
use google_cloud_gax::options::RequestOptionsBuilder as _;
use google_cloud_gax::retry_policy::{Aip194Strict, RetryPolicyExt as _};
use google_cloud_tasks_v2::client::CloudTasks;
use google_cloud_tasks_v2::model::{HttpMethod, HttpRequest, OidcToken, Task};
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use tokio::time;

use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::port::task_queue::{Enqueued, TaskMethod, TaskQueue, TaskRequest};

const SERVICE: &str = "Cloud Tasks";
const MAX_TASK_ID_BYTES: usize = 500;
/// Cloud Tasks accepts HTTP dispatch deadlines between 15 seconds and 30 minutes.
const DISPATCH_DEADLINE: (Duration, Duration) =
    (Duration::from_secs(15), Duration::from_secs(1800));
/// Set by Cloud Tasks or by this provider; a caller value would be ignored or conflict with OIDC.
const RESERVED_HEADERS: [&str; 5] =
    ["authorization", "content-length", "content-type", "host", "user-agent"];
const RESERVED_HEADER_PREFIXES: [&str; 3] = ["x-cloudtasks-", "x-appengine-", "x-google-"];

/// Queue and OIDC target of a `CloudTasksProvider`, supplied by the composition root.
pub struct CloudTasksSettings {
    pub project_id: String,
    pub location: String,
    pub queue: String,
    /// HTTPS base URL of the worker; task paths resolve under it.
    pub target_url: String,
    /// Service account whose OIDC token authenticates each task.
    pub service_account: String,
    /// OIDC audience; defaults to the origin of `target_url`.
    pub audience: Option<String>,
}

/// One queue whose HTTP tasks authenticate to the target with Google OIDC.
///
/// No shared secret travels in tasks: the target (e.g. Cloud Run with `roles/run.invoker` for the
/// service account) validates the token's audience and issuer. Named tasks are idempotent, so the
/// SDK retries them on transient failures; unnamed tasks are sent once to avoid duplicates.
#[derive(Clone)]
pub struct CloudTasksProvider {
    client: CloudTasks,
    queue: String,
    target_url: Url,
    oidc: OidcToken,
}

impl fmt::Debug for CloudTasksProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CloudTasksProvider")
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

impl CloudTasksProvider {
    /// Builds the client from Application Default Credentials; it performs no queue I/O.
    #[tracing::instrument(skip_all, fields(dependency = "cloud_tasks"))]
    pub async fn new(settings: CloudTasksSettings) -> DomainResult<Self> {
        let mut target_url = Url::parse(&settings.target_url)
            .ok()
            .filter(|url| {
                url.scheme() == "https"
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
            })
            .ok_or_else(|| {
                DomainError::internal("Cloud Tasks target must be an HTTPS URL without credentials")
            })?;
        // A trailing slash makes `join` append task paths instead of replacing the last segment.
        if !target_url.path().ends_with('/') {
            target_url.set_path(&format!("{}/", target_url.path()));
        }
        let audience = match &settings.audience {
            Some(audience) => audience.clone(),
            None => target_url.origin().ascii_serialization(),
        };

        // Enqueueing usually happens inside a request: bound retries well below its budget.
        let retry = Aip194Strict.with_attempt_limit(3).with_time_limit(Duration::from_secs(10));
        let client = time::timeout(
            Duration::from_secs(10),
            CloudTasks::builder().with_retry_policy(retry).build(),
        )
        .await
        .map_err(|_| DomainError::timeout("Cloud Tasks client initialization"))?
        .map_err(|_| {
            DomainError::service_unavailable(SERVICE, "Failed to initialize the client")
        })?;

        tracing::info!("Cloud Tasks initialized");
        Ok(Self {
            client,
            queue: format!(
                "projects/{}/locations/{}/queues/{}",
                settings.project_id, settings.location, settings.queue
            ),
            target_url,
            oidc: OidcToken::new()
                .set_service_account_email(settings.service_account.clone())
                .set_audience(audience),
        })
    }

    /// Escape hatch for operations not covered here (list, delete, queue administration).
    pub fn client(&self) -> &CloudTasks {
        &self.client
    }

    /// Parent for `create_task`: `projects/<project>/locations/<location>/queues/<queue>`.
    pub fn queue(&self) -> &str {
        &self.queue
    }

    fn build(&self, task: TaskRequest) -> DomainResult<Task> {
        let url = self
            .target_url
            .join(task.path.trim_start_matches('/'))
            .ok()
            .filter(|url| url.origin() == self.target_url.origin())
            .ok_or_else(|| invalid("path", "Task path must stay on the configured target"))?;

        let mut headers = BTreeMap::new();
        for (name, value) in task.headers {
            let name = HeaderName::from_bytes(name.trim().as_bytes())
                .map_err(|_| invalid("headers", "Task header name is malformed"))?;
            let reserved = RESERVED_HEADERS.contains(&name.as_str())
                || RESERVED_HEADER_PREFIXES.iter().any(|prefix| name.as_str().starts_with(prefix));
            if reserved {
                return Err(invalid("headers", "Task header is reserved by Cloud Tasks or OIDC"));
            }
            if HeaderValue::from_str(&value).is_err() {
                return Err(invalid("headers", "Task header value is malformed"));
            }
            if headers.insert(name.as_str().to_owned(), value).is_some() {
                return Err(invalid("headers", "Task header is repeated"));
            }
        }

        let mut request = HttpRequest::new()
            .set_url(url.to_string())
            .set_http_method(http_method(task.method))
            .set_oidc_token(self.oidc.clone());
        if let Some((body, content_type)) = task.body {
            if !task.method.accepts_body() {
                return Err(invalid("body", "Only POST, PUT and PATCH tasks may carry a body"));
            }
            headers.insert("content-type".to_owned(), content_type);
            request = request.set_body(Bytes::from(body));
        }
        let mut built = Task::new().set_http_request(request.set_headers(headers));

        if let Some(id) = task.id {
            let valid = !id.is_empty()
                && id.len() <= MAX_TASK_ID_BYTES
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
            if !valid {
                return Err(invalid("task_id", "Task IDs use 1-500 letters, digits, - or _"));
            }
            built = built.set_name(format!("{}/tasks/{id}", self.queue));
        }
        if let Some(at) = task.schedule_at {
            let at = google_cloud_wkt::Timestamp::try_from(SystemTime::from(at))
                .map_err(|_| invalid("schedule_at", "Task schedule time is out of range"))?;
            built = built.set_schedule_time(at);
        }
        if let Some(deadline) = task.dispatch_deadline {
            let (min, max) = DISPATCH_DEADLINE;
            let deadline = Some(deadline)
                .filter(|deadline| (min..=max).contains(deadline))
                .and_then(|deadline| google_cloud_wkt::Duration::try_from(deadline).ok())
                .ok_or_else(|| invalid("dispatch_deadline", "Deadline must be 15 s to 30 min"))?;
            built = built.set_dispatch_deadline(deadline);
        }
        Ok(built)
    }
}

#[async_trait]
impl TaskQueue for CloudTasksProvider {
    /// Enqueues any HTTP task under the configured target.
    #[tracing::instrument(skip_all, fields(dependency = "cloud_tasks", named = task.id.is_some()))]
    async fn enqueue(&self, task: TaskRequest) -> DomainResult<Enqueued> {
        let named = task.id.is_some();
        let request = self.build(task)?;
        let result = self
            .client
            .create_task()
            .set_parent(&self.queue)
            .set_task(request)
            .with_idempotency(named)
            .send()
            .await;

        match result {
            Ok(created) => Ok(Enqueued::Created { name: created.name }),
            Err(error) if named && error.http_status_code() == Some(409) => {
                Ok(Enqueued::AlreadyEnqueued)
            }
            Err(error) => {
                let status = error.http_status_code();
                tracing::warn!(status, "Cloud Tasks enqueue failed");
                Err(match status {
                    // Malformed or forbidden requests repeat identically: a configuration defect.
                    Some(status) if (400..500).contains(&status) && status != 429 => {
                        DomainError::internal("Cloud Tasks rejected the task")
                    }
                    _ => DomainError::service_unavailable(SERVICE, "Failed to enqueue the task"),
                })
            }
        }
    }
}

fn http_method(method: TaskMethod) -> HttpMethod {
    match method {
        TaskMethod::Get => HttpMethod::Get,
        TaskMethod::Head => HttpMethod::Head,
        TaskMethod::Post => HttpMethod::Post,
        TaskMethod::Put => HttpMethod::Put,
        TaskMethod::Patch => HttpMethod::Patch,
        TaskMethod::Delete => HttpMethod::Delete,
        TaskMethod::Options => HttpMethod::Options,
    }
}

/// A malformed task is built by the service itself, so it is an internal fault, not client input.
fn invalid(field: &'static str, reason: &str) -> DomainError {
    DomainError::internal(format!("Invalid task {field}: {reason}"))
}
