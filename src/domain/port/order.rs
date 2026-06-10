use crate::domain::entities::order::{Order, OrderId};
use crate::domain::entities::user::UserId;
use crate::domain::error::DomainResult;
use crate::domain::pagination::Pagination;
use async_trait::async_trait;

/// Repository Interface for Order Management.
/// strictly decoupled from persistence implementation.
#[async_trait]
pub trait OrderRepositoryPort: Send + Sync {
    async fn create(&self, order: &Order) -> DomainResult<OrderId>;

    async fn find_by_id(&self, id: &OrderId) -> DomainResult<Option<Order>>;

    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<Order>>;

    async fn find_by_user_id(
        &self,
        user_id: &UserId,
        pagination: Pagination,
    ) -> DomainResult<Vec<Order>>;

    async fn delete(&self, id: &OrderId) -> DomainResult<bool>;

    async fn count(&self) -> DomainResult<u64>;
}
