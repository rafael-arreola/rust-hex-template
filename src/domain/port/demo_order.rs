use crate::domain::core::error::DomainResult;
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_user::DemoUserId;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Repository Interface for DemoOrder Management.
/// strictly decoupled from persistence implementation.
#[async_trait]
pub trait DemoOrderRepositoryPort: Send + Sync {
    async fn create(&self, order: &DemoOrder) -> DomainResult<DemoOrderId>;

    async fn find_by_id(&self, id: &DemoOrderId) -> DomainResult<Option<DemoOrder>>;

    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoOrder>>;

    async fn find_by_user_id(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<PageResult<DemoOrder>>;

    async fn delete(&self, id: &DemoOrderId, now: DateTime<Utc>) -> DomainResult<bool>;
}
