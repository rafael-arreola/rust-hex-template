use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::error::DomainResult;
use crate::domain::pagination::Pagination;
use async_trait::async_trait;

/// Repository Interface for DemoOrder Management.
/// strictly decoupled from persistence implementation.
#[async_trait]
pub trait DemoOrderRepositoryPort: Send + Sync {
    async fn create(&self, order: &DemoOrder) -> DomainResult<DemoOrderId>;

    async fn find_by_id(&self, id: &DemoOrderId) -> DomainResult<Option<DemoOrder>>;

    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<DemoOrder>>;

    async fn find_by_user_id(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<Vec<DemoOrder>>;

    async fn delete(&self, id: &DemoOrderId) -> DomainResult<bool>;

    async fn count(&self) -> DomainResult<u64>;
}
