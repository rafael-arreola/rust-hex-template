use async_trait::async_trait;

use crate::domain::contracts::system::task_queue::{Enqueued, TaskPayload, TaskRequest};
use crate::domain::core::error::DomainResult;

/// Deferred HTTP work delivered to the configured target with the queue's own authentication.
#[async_trait]
pub trait TaskQueue: Send + Sync {
    async fn enqueue(&self, task: TaskRequest) -> DomainResult<Enqueued>;
}

impl dyn TaskQueue {
    /// Enqueues a typed payload as JSON on its own route and verb.
    pub async fn dispatch<T: TaskPayload>(&self, payload: &T) -> DomainResult<Enqueued> {
        self.enqueue(TaskRequest::from_payload(payload)?).await
    }
}
