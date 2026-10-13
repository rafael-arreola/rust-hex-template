use crate::domain::core::error::DomainResult;
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_product::{DemoProduct, DemoProductId, DemoProductMetadata};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Repository Interface for DemoProduct Management.
#[async_trait]
pub trait DemoProductRepositoryPort: Send + Sync {
    async fn create(&self, product: &DemoProduct) -> DomainResult<DemoProductId>;

    async fn find_by_id(&self, id: &DemoProductId) -> DomainResult<Option<DemoProduct>>;

    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoProduct>>;

    async fn update_metadata(
        &self,
        id: &DemoProductId,
        metadata: &DemoProductMetadata,
        now: DateTime<Utc>,
    ) -> DomainResult<bool>;

    /// Atomically reserves `quantity` units.
    ///
    /// Returns `false` when the product does not exist or does not have enough
    /// stock. The availability check and the decrement happen in a **single**
    /// storage operation, so concurrent callers can never oversell.
    ///
    /// Never expose a raw "add this delta" method here: it invites the
    /// read-check-then-write pattern, which is a race condition under any
    /// real concurrency. The signature is the guard rail.
    async fn try_reserve_stock(
        &self,
        id: &DemoProductId,
        quantity: i32,
        now: DateTime<Utc>,
    ) -> DomainResult<bool>;

    /// Returns `quantity` previously reserved units to the product.
    ///
    /// Compensating action for use cases that reserve stock and then fail on a
    /// later step. Returns `false` when the product no longer exists.
    async fn release_stock(
        &self,
        id: &DemoProductId,
        quantity: i32,
        now: DateTime<Utc>,
    ) -> DomainResult<bool>;

    async fn delete(&self, id: &DemoProductId) -> DomainResult<bool>;
}
