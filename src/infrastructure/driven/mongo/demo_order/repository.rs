use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::demo_order::DemoOrderRepositoryPort;
use crate::infrastructure::driven::mongo::demo_order::model::DemoOrderModel;
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct DemoOrderRepository {
    collection: Collection<DemoOrderModel>,
}

impl DemoOrderRepository {
    /// Building the repository ensures its indexes exist. See
    /// `DemoUserRepository::new` for the rationale.
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<DemoOrderModel>("orders") };
        repo.create_indexes().await?;
        Ok(repo)
    }

    /// Create database indexes (idempotent — safe to call on every startup).
    /// Private: `new` is the only caller, by design.
    async fn create_indexes(&self) -> DomainResult<()> {
        let indexes = vec![
            IndexModel::builder()
                .keys(doc! { "user_id": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder().name("user_created_compound_idx".to_string()).build(),
                )
                .build(),
            IndexModel::builder()
                .keys(doc! { "product_id": 1 })
                .options(IndexOptions::builder().name("product_idx".to_string()).build())
                .build(),
        ];

        self.collection
            .create_indexes(indexes)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        tracing::info!("✓ Orders indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoOrderRepositoryPort for DemoOrderRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, order: &DemoOrder) -> DomainResult<DemoOrderId> {
        let model = DemoOrderModel::from(order.clone());

        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| DemoOrderId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoOrderId) -> DomainResult<Option<DemoOrder>> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoOrder", &**id))?;

        let model = self
            .collection
            .find_one(doc! { "_id": oid, "deleted_at": { "$exists": false } })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(model.map(DemoOrder::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<DemoOrder>> {
        let cursor = self
            .collection
            .find(doc! { "deleted_at": { "$exists": false } })
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1 })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        let models: Vec<DemoOrderModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(DemoOrder::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_user_id(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<Vec<DemoOrder>> {
        let oid = ObjectId::parse_str(&**user_id)
            .map_err(|_| DomainError::invalid_param("user_id", "DemoOrder", &**user_id))?;

        let cursor = self
            .collection
            .find(doc! {
                "user_id": oid,
                "deleted_at": { "$exists": false }
            })
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1 })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        let models: Vec<DemoOrderModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(DemoOrder::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoOrderId) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoOrder", &**id))?;

        let now = mongodb::bson::DateTime::from_chrono(chrono::Utc::now());

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
