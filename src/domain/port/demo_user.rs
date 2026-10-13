use crate::domain::core::error::DomainResult;
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Repository Interface for DemoUser Management.
/// Strictly decoupled from persistence implementation.
#[async_trait]
pub trait DemoUserRepositoryPort: Send + Sync {
    async fn create(&self, user: &DemoUser) -> DomainResult<DemoUserId>;

    async fn find_by_id(&self, id: &DemoUserId) -> DomainResult<Option<DemoUser>>;

    async fn find_by_email(&self, email: &str) -> DomainResult<Option<DemoUser>>;

    /// List users with pagination.
    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoUser>>;

    async fn update(
        &self,
        id: &DemoUserId,
        user: &DemoUser,
        now: DateTime<Utc>,
    ) -> DomainResult<bool>;

    async fn delete(&self, id: &DemoUserId, now: DateTime<Utc>) -> DomainResult<bool>;
}
