use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::domain::core::error::{DomainError, DomainResult, StatusCode};

/// HTTP verb of a deferred task. Only POST, PUT and PATCH may carry a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl TaskMethod {
    pub fn accepts_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }
}

/// A typed task: the payload type owns its route, verb and optional deduplication key.
///
/// ```ignore
/// #[derive(Serialize)]
/// struct ReindexProduct { product_id: String }
///
/// impl TaskPayload for ReindexProduct {
///     const PATH: &'static str = "products/reindex";
///     fn task_id(&self) -> Option<String> { Some(format!("reindex-{}", self.product_id)) }
/// }
///
/// tasks.dispatch(&ReindexProduct { product_id }).await?;
/// ```
pub trait TaskPayload: Serialize {
    /// Path relative to the queue's configured target.
    const PATH: &'static str;
    /// Must accept a body (POST, PUT or PATCH), because the payload is sent as JSON.
    const METHOD: TaskMethod = TaskMethod::Post;

    /// Deterministic ID for deduplication; `None` lets the queue assign one.
    fn task_id(&self) -> Option<String> {
        None
    }
}

/// Any HTTP task under the queue's target. Adapters validate it against their own limits.
#[derive(Clone, Debug)]
pub struct TaskRequest {
    pub method: TaskMethod,
    pub path: String,
    pub headers: Vec<(String, String)>,
    /// Body and its media type.
    pub body: Option<(Vec<u8>, String)>,
    /// Names the task so a repeated enqueue is recognised instead of duplicated.
    pub id: Option<String>,
    /// Earliest dispatch time; the caller reads the clock once and passes the instant.
    pub schedule_at: Option<DateTime<Utc>>,
    /// How long the queue waits for the target's response.
    pub dispatch_deadline: Option<Duration>,
}

impl TaskRequest {
    pub fn new(method: TaskMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            headers: Vec::new(),
            body: None,
            id: None,
            schedule_at: None,
            dispatch_deadline: None,
        }
    }

    pub fn post(path: impl Into<String>) -> Self {
        Self::new(TaskMethod::Post, path)
    }

    pub fn put(path: impl Into<String>) -> Self {
        Self::new(TaskMethod::Put, path)
    }

    pub fn patch(path: impl Into<String>) -> Self {
        Self::new(TaskMethod::Patch, path)
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(TaskMethod::Get, path)
    }

    pub fn delete(path: impl Into<String>) -> Self {
        Self::new(TaskMethod::Delete, path)
    }

    /// Builds a JSON task from a typed payload, including its deduplication key.
    pub fn from_payload<T: TaskPayload>(payload: &T) -> DomainResult<Self> {
        let task = Self::new(T::METHOD, T::PATH).json(payload)?;
        Ok(match payload.task_id() {
            Some(id) => task.id(id),
            None => task,
        })
    }

    /// Serializes `body` as `application/json`.
    pub fn json<T: Serialize + ?Sized>(self, body: &T) -> DomainResult<Self> {
        let bytes = serde_json::to_vec(body).map_err(|_| {
            DomainError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Task body cannot be serialized as JSON",
            )
            .with_trace_code("TASK_JSON_SERIALIZATION_FAILED")
        })?;
        Ok(self.body(bytes, "application/json"))
    }

    /// Raw body with its media type, e.g. `application/x-protobuf` or `text/plain`.
    pub fn body(mut self, body: Vec<u8>, content_type: impl Into<String>) -> Self {
        self.body = Some((body, content_type.into()));
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn schedule_at(mut self, at: DateTime<Utc>) -> Self {
        self.schedule_at = Some(at);
        self
    }

    pub fn dispatch_deadline(mut self, deadline: Duration) -> Self {
        self.dispatch_deadline = Some(deadline);
        self
    }
}

/// Result of an enqueue. A named task that already exists is a success, not an error: a previous
/// attempt enqueued it and only its response was lost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enqueued {
    Created { name: String },
    AlreadyEnqueued,
}
