use crate::entities::user::{User, UserId};
use crate::error::DomainResult;
use crate::pagination::Pagination;
use async_trait::async_trait;

/// Repository Interface for User Management.
/// Strictly decoupled from persistence implementation.
#[async_trait]
pub trait UserRepositoryPort: Send + Sync {
    async fn create(&self, user: &User) -> DomainResult<UserId>;

    async fn find_by_id(&self, id: &UserId) -> DomainResult<Option<User>>;

    async fn find_by_email(&self, email: &str) -> DomainResult<Option<User>>;

    /// List users with pagination.
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<User>>;

    async fn update(&self, id: &UserId, user: &User) -> DomainResult<bool>;

    async fn delete(&self, id: &UserId) -> DomainResult<bool>;

    async fn count(&self) -> DomainResult<u64>;
}
