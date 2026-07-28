use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use crate::domain::error::DomainResult;
use crate::domain::pagination::Pagination;
use async_trait::async_trait;

/// Repository Interface for DemoUser Management.
/// Strictly decoupled from persistence implementation.
#[async_trait]
pub trait DemoUserRepositoryPort: Send + Sync {
    async fn create(&self, user: &DemoUser) -> DomainResult<DemoUserId>;

    async fn find_by_id(&self, id: &DemoUserId) -> DomainResult<Option<DemoUser>>;

    async fn find_by_email(&self, email: &str) -> DomainResult<Option<DemoUser>>;

    /// List users with pagination.
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<DemoUser>>;

    async fn update(&self, id: &DemoUserId, user: &DemoUser) -> DomainResult<bool>;

    async fn delete(&self, id: &DemoUserId) -> DomainResult<bool>;

    async fn count(&self) -> DomainResult<u64>;
}
