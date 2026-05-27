use domain::entities::product::{Product, ProductId, ProductMetadata, ProductStatus};
use domain::error::{DomainError, DomainResult};
use domain::pagination::Pagination;
use domain::port::product::ProductRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProductService {
    repo: Arc<dyn ProductRepositoryPort>,
}

impl ProductService {
    pub fn new(repo: Arc<dyn ProductRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(%name))]
    pub async fn create_product(
        &self,
        name: &str,
        price: f64,
        stock: i32,
        metadata: ProductMetadata,
    ) -> DomainResult<Product> {
        let now = chrono::Utc::now();
        let mut product = Product {
            id: None,
            name: name.to_string(),
            price,
            stock,
            status: ProductStatus::Draft,
            metadata,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = self.repo.create(&product).await?;
        product.id = Some(id);

        tracing::info!(product_id = %product.id.as_deref().unwrap_or("unknown"), "Product created");
        Ok(product)
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn get_product(&self, id: &ProductId) -> DomainResult<Product> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Product", id.to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_products(&self, pagination: Pagination) -> DomainResult<Vec<Product>> {
        self.repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn update_metadata(
        &self,
        id: &ProductId,
        metadata: ProductMetadata,
    ) -> DomainResult<Product> {
        let updated = self.repo.update_metadata(id, &metadata).await?;
        if !updated {
            return Err(DomainError::not_found("Product", id.to_string()));
        }

        tracing::info!("Product metadata updated");
        self.get_product(id).await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn delete_product(&self, id: &ProductId) -> DomainResult<()> {
        let deleted = self.repo.delete(id).await?;
        if !deleted {
            return Err(DomainError::not_found("Product", id.to_string()));
        }
        tracing::info!("Product soft-deleted");
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn count_products(&self) -> DomainResult<u64> {
        self.repo.count().await
    }

    #[tracing::instrument(skip_all, fields(%id, %quantity))]
    pub async fn decrement_stock(&self, id: &ProductId, quantity: i32) -> DomainResult<()> {
        let product = self.get_product(id).await?;

        if product.stock < quantity {
            return Err(DomainError::business_rule(format!(
                "Insufficient stock for product {}: requested {}, available {}",
                id, quantity, product.stock
            )));
        }

        let updated = self.repo.update_stock(id, -quantity).await?;
        if !updated {
            return Err(DomainError::not_found("Product", id.to_string()));
        }

        tracing::info!(remaining = product.stock - quantity, "Stock decremented");
        Ok(())
    }
}
