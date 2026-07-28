use crate::domain::entities::demo_product::{DemoProduct, DemoProductId, DemoProductMetadata};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::demo_product::DemoProductRepositoryPort;
use crate::infrastructure::driven::mongo::demo_product::model::DemoProductModel;
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{self, doc, oid::ObjectId},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct DemoProductRepository {
    collection: Collection<DemoProductModel>,
}

impl DemoProductRepository {
    /// Building the repository ensures its indexes exist. See
    /// `DemoUserRepository::new` for the rationale.
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<DemoProductModel>("products") };
        repo.create_indexes().await?;
        Ok(repo)
    }

    /// Create database indexes (idempotent — safe to call on every startup).
    /// Private: `new` is the only caller, by design.
    async fn create_indexes(&self) -> DomainResult<()> {
        let indexes = vec![
            IndexModel::builder()
                .keys(doc! { "deleted_at": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("deleted_created_compound_idx".to_string())
                        .build(),
                )
                .build(),
            IndexModel::builder()
                .keys(doc! { "deleted_at": 1, "price": 1 })
                .options(
                    IndexOptions::builder().name("deleted_price_compound_idx".to_string()).build(),
                )
                .build(),
        ];

        self.collection
            .create_indexes(indexes)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        tracing::info!("✓ Products indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoProductRepositoryPort for DemoProductRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, product: &DemoProduct) -> DomainResult<DemoProductId> {
        let model = DemoProductModel::from(product.clone());
        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| DemoProductId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoProductId) -> DomainResult<Option<DemoProduct>> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoProduct", &**id))?;

        let model = self
            .collection
            .find_one(doc! { "_id": oid, "deleted_at": { "$exists": false } })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(model.map(DemoProduct::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<DemoProduct>> {
        let cursor = self
            .collection
            .find(doc! { "deleted_at": { "$exists": false } })
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1 })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        let models: Vec<DemoProductModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(DemoProduct::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn update_metadata(
        &self,
        id: &DemoProductId,
        metadata: &DemoProductMetadata,
    ) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoProduct", &**id))?;

        let bson_metadata = bson::serialize_to_bson(metadata)
            .map_err(|e| DomainError::internal(format!("Serialization error: {}", e)))?;

        let now = bson::DateTime::from_chrono(chrono::Utc::now());

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! {
                    "$set": {
                        "metadata": bson_metadata,
                        "updated_at": now
                    }
                },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn try_reserve_stock(&self, id: &DemoProductId, quantity: i32) -> DomainResult<bool> {
        if quantity <= 0 {
            return Err(DomainError::Invalid {
                field: "quantity",
                reason: format!("Reserved quantity must be positive, got {}", quantity),
            });
        }

        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoProduct", &**id))?;

        let now = bson::DateTime::from_chrono(chrono::Utc::now());

        // The `$gte` guard is what makes this atomic: MongoDB matches the
        // document and applies the `$inc` as one operation, so two concurrent
        // reservations cannot both succeed against the same last unit.
        // Dropping it turns this into a read-check-write race.
        let result = self
            .collection
            .update_one(
                doc! {
                    "_id": oid,
                    "deleted_at": { "$exists": false },
                    "stock": { "$gte": quantity },
                },
                doc! {
                    "$inc": { "stock": -quantity },
                    "$set": { "updated_at": now },
                },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn release_stock(&self, id: &DemoProductId, quantity: i32) -> DomainResult<bool> {
        if quantity <= 0 {
            return Err(DomainError::Invalid {
                field: "quantity",
                reason: format!("Released quantity must be positive, got {}", quantity),
            });
        }

        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoProduct", &**id))?;

        let now = bson::DateTime::from_chrono(chrono::Utc::now());

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! {
                    "$inc": { "stock": quantity },
                    "$set": { "updated_at": now },
                },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoProductId) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoProduct", &**id))?;

        let now = bson::DateTime::from_chrono(chrono::Utc::now());

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! { "$set": { "deleted_at": now } },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn count(&self) -> DomainResult<u64> {
        self.collection
            .count_documents(doc! { "deleted_at": { "$exists": false } })
            .await
            .map_err(|e| DomainError::database(e.to_string()))
    }
}
