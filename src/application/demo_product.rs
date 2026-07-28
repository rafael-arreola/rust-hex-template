use crate::domain::entities::demo_product::{
    DemoProduct, DemoProductId, DemoProductMetadata, DemoProductStatus,
};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::demo_product::DemoProductRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct DemoProductService {
    repo: Arc<dyn DemoProductRepositoryPort>,
}

impl DemoProductService {
    pub fn new(repo: Arc<dyn DemoProductRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(%name))]
    pub async fn create_product(
        &self,
        name: &str,
        price: f64,
        stock: i32,
        metadata: DemoProductMetadata,
    ) -> DomainResult<DemoProduct> {
        let now = chrono::Utc::now();
        let mut product = DemoProduct {
            id: None,
            name: name.to_string(),
            price,
            stock,
            status: DemoProductStatus::Draft,
            metadata,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = self.repo.create(&product).await?;
        product.id = Some(id);

        tracing::info!(product_id = %product.id.as_deref().unwrap_or("unknown"), "DemoProduct created");
        Ok(product)
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn get_product(&self, id: &DemoProductId) -> DomainResult<DemoProduct> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("DemoProduct", id.to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_products(&self, pagination: Pagination) -> DomainResult<Vec<DemoProduct>> {
        self.repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn update_metadata(
        &self,
        id: &DemoProductId,
        metadata: DemoProductMetadata,
    ) -> DomainResult<DemoProduct> {
        let updated = self.repo.update_metadata(id, &metadata).await?;
        if !updated {
            return Err(DomainError::not_found("DemoProduct", id.to_string()));
        }

        tracing::info!("DemoProduct metadata updated");
        self.get_product(id).await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn delete_product(&self, id: &DemoProductId) -> DomainResult<()> {
        let deleted = self.repo.delete(id).await?;
        if !deleted {
            return Err(DomainError::not_found("DemoProduct", id.to_string()));
        }
        tracing::info!("DemoProduct soft-deleted");
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn count_products(&self) -> DomainResult<u64> {
        self.repo.count().await
    }

    /// Reserves `quantity` units atomically.
    ///
    /// The reservation itself decides whether there is enough stock — there is
    /// deliberately no read-then-check before it, because that pattern lets two
    /// concurrent callers both pass the check and oversell.
    #[tracing::instrument(skip_all, fields(%id, %quantity))]
    pub async fn reserve_stock(&self, id: &DemoProductId, quantity: i32) -> DomainResult<()> {
        if quantity <= 0 {
            return Err(DomainError::Invalid {
                field: "quantity",
                reason: format!("Reserved quantity must be positive, got {}", quantity),
            });
        }

        if self.repo.try_reserve_stock(id, quantity).await? {
            tracing::info!("Stock reserved");
            return Ok(());
        }

        // The reservation can fail because the product is gone or because it
        // ran out of stock; distinguish them so the client gets the right code.
        let product = self.get_product(id).await?;
        Err(DomainError::business_rule(format!(
            "Insufficient stock for product {}: requested {}, available {}",
            id, quantity, product.stock
        )))
    }

    /// Returns previously reserved units. Compensating action for a failed flow.
    #[tracing::instrument(skip_all, fields(%id, %quantity))]
    pub async fn release_stock(&self, id: &DemoProductId, quantity: i32) -> DomainResult<()> {
        let released = self.repo.release_stock(id, quantity).await?;
        if !released {
            return Err(DomainError::not_found("DemoProduct", id.to_string()));
        }

        tracing::info!("Stock released");
        Ok(())
    }
}
