use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_product::{DemoProduct, DemoProductId, DemoProductMetadata};
use crate::domain::port::repositories::demo_product::DemoProductRepositoryPort;
use crate::infrastructure::persistence::mongo::{
    delete_by_id, demo_product::model::DemoProductModel, error, find_by_id, insert_one, paginate,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    Collection, Database, IndexModel,
    bson::{self, doc},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct DemoProductRepository {
    collection: Collection<DemoProductModel>,
}

impl DemoProductRepository {
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<DemoProductModel>("products") };
        repo.create_indexes().await?;
        Ok(repo)
    }

    async fn create_indexes(&self) -> DomainResult<()> {
        let indexes = vec![
            IndexModel::builder()
                .keys(doc! { "created_at": -1, "_id": -1 })
                .options(
                    IndexOptions::builder().name("created_id_compound_idx".to_string()).build(),
                )
                .build(),
            IndexModel::builder()
                .keys(doc! { "price": 1 })
                .options(IndexOptions::builder().name("price_idx".to_string()).build())
                .build(),
        ];

        self.collection
            .create_indexes(indexes)
            .await
            .map_err(|error| error::map(error, "DemoProduct"))?;

        tracing::info!("✓ Products indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoProductRepositoryPort for DemoProductRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, product: &DemoProduct) -> DomainResult<DemoProductId> {
        insert_one(&self.collection, product, "DemoProduct").await
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoProductId) -> DomainResult<Option<DemoProduct>> {
        find_by_id(&self.collection, id, "DemoProduct").await
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoProduct>> {
        paginate(&self.collection, doc! {}, pagination, "DemoProduct").await
    }

    #[tracing::instrument(skip_all)]
    async fn update_metadata(
        &self,
        id: &DemoProductId,
        metadata: &DemoProductMetadata,
        now: DateTime<Utc>,
    ) -> DomainResult<bool> {
        let oid = error::parse_oid(id, "id", "DemoProduct")?;
        let bson_metadata = bson::serialize_to_bson(metadata).map_err(|_| {
            DomainError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "MongoDB metadata serialization failed",
            )
            .with_trace_code("MONGO_METADATA_SERIALIZATION_FAILED")
        })?;
        let now = bson::DateTime::from_chrono(now);

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid },
                doc! { "$set": { "metadata": bson_metadata, "updated_at": now } },
            )
            .await
            .map_err(|error| error::map(error, "DemoProduct"))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn try_reserve_stock(
        &self,
        id: &DemoProductId,
        quantity: i32,
        now: DateTime<Utc>,
    ) -> DomainResult<bool> {
        if quantity <= 0 {
            return Err(DomainError::new(
                StatusCode::BAD_REQUEST,
                format!("Reserved quantity {quantity}"),
            )
            .with_public("Invalid quantity: must be positive")
            .with_trace_code("INVALID_STOCK_QUANTITY"));
        }

        let oid = error::parse_oid(id, "id", "DemoProduct")?;
        let now = bson::DateTime::from_chrono(now);

        // The `$gte` guard makes the availability check and decrement a single atomic operation.
        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "stock": { "$gte": quantity } },
                doc! { "$inc": { "stock": -quantity }, "$set": { "updated_at": now } },
            )
            .await
            .map_err(|error| error::map(error, "DemoProduct"))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn release_stock(
        &self,
        id: &DemoProductId,
        quantity: i32,
        now: DateTime<Utc>,
    ) -> DomainResult<bool> {
        if quantity <= 0 {
            return Err(DomainError::new(
                StatusCode::BAD_REQUEST,
                format!("Released quantity {quantity}"),
            )
            .with_public("Invalid quantity: must be positive")
            .with_trace_code("INVALID_STOCK_QUANTITY"));
        }

        let oid = error::parse_oid(id, "id", "DemoProduct")?;
        let now = bson::DateTime::from_chrono(now);

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid },
                doc! { "$inc": { "stock": quantity }, "$set": { "updated_at": now } },
            )
            .await
            .map_err(|error| error::map(error, "DemoProduct"))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoProductId) -> DomainResult<bool> {
        delete_by_id(&self.collection, id, "DemoProduct").await
    }
}
