use crate::domain::entities::product::{Product, ProductId, ProductMetadata};
use crate::domain::error::DomainResult;
use crate::domain::pagination::Pagination;
use async_trait::async_trait;

/// Repository Interface for Product Management.
#[async_trait]
pub trait ProductRepositoryPort: Send + Sync {
    async fn create(&self, product: &Product) -> DomainResult<ProductId>;

    async fn find_by_id(&self, id: &ProductId) -> DomainResult<Option<Product>>;

    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<Product>>;

    async fn update_metadata(
        &self,
        id: &ProductId,
        metadata: &ProductMetadata,
    ) -> DomainResult<bool>;

    /// Update stock by delta (positive or negative).
    async fn update_stock(&self, id: &ProductId, delta: i32) -> DomainResult<bool>;

    async fn delete(&self, id: &ProductId) -> DomainResult<bool>;

    async fn count(&self) -> DomainResult<u64>;
}
