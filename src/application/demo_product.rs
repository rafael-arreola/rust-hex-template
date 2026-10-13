use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_product::{
    DemoProduct, DemoProductId, DemoProductMetadata, DemoProductStatus,
};
use crate::domain::port::repositories::demo_product::DemoProductRepositoryPort;
use chrono::Utc;
use std::sync::Arc;

#[derive(Clone)]
pub struct DemoProductService {
    repo: Arc<dyn DemoProductRepositoryPort>,
}

impl DemoProductService {
    pub fn new(repo: Arc<dyn DemoProductRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(operation = "create_product"))]
    pub async fn create_product(
        &self,
        name: &str,
        price: f64,
        stock: i32,
        metadata: DemoProductMetadata,
    ) -> DomainResult<DemoProduct> {
        let now = Utc::now();
        let mut product = DemoProduct {
            id: None,
            name: name.to_string(),
            price,
            stock,
            status: DemoProductStatus::Draft,
            metadata,
            created_at: now,
            updated_at: now,
        };

        let id = self.repo.create(&product).await?;
        tracing::info!(product_id = %id, "DemoProduct created");
        product.id = Some(id);
        Ok(product)
    }

    #[tracing::instrument(skip_all, fields(operation = "get_product"))]
    pub async fn get_product(&self, id: &DemoProductId) -> DomainResult<DemoProduct> {
        self.repo.find_by_id(id).await?.ok_or_else(|| not_found(id))
    }

    #[tracing::instrument(skip_all, fields(page = pagination.page(), limit = pagination.limit()))]
    pub async fn list_products(
        &self,
        pagination: Pagination,
    ) -> DomainResult<PageResult<DemoProduct>> {
        self.repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(operation = "update_metadata"))]
    pub async fn update_metadata(
        &self,
        id: &DemoProductId,
        metadata: DemoProductMetadata,
    ) -> DomainResult<DemoProduct> {
        let now = Utc::now();
        if !self.repo.update_metadata(id, &metadata, now).await? {
            return Err(not_found(id));
        }

        tracing::info!("DemoProduct metadata updated");
        self.get_product(id).await
    }

    #[tracing::instrument(skip_all, fields(operation = "delete_product"))]
    pub async fn delete_product(&self, id: &DemoProductId) -> DomainResult<()> {
        if !self.repo.delete(id).await? {
            return Err(not_found(id));
        }
        tracing::info!("DemoProduct deleted");
        Ok(())
    }
}

fn not_found(id: &DemoProductId) -> DomainError {
    DomainError::new(StatusCode::NOT_FOUND, format!("DemoProduct id {id} not found"))
        .with_public("DemoProduct not found")
        .with_trace_code("DEMO_PRODUCT_NOT_FOUND")
}
