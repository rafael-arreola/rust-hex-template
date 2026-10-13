use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::port::demo_order::DemoOrderRepositoryPort;
use crate::infrastructure::persistence::mongo::{demo_order::model::DemoOrderModel, error};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{self, doc, oid::ObjectId},
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
                .keys(doc! { "deleted_at": 1, "created_at": -1, "_id": -1 })
                .options(
                    IndexOptions::builder()
                        .name("deleted_created_id_compound_idx".to_string())
                        .build(),
                )
                .build(),
            IndexModel::builder()
                .keys(doc! { "deleted_at": 1, "user_id": 1, "created_at": -1, "_id": -1 })
                .options(
                    IndexOptions::builder()
                        .name("deleted_user_created_id_compound_idx".to_string())
                        .build(),
                )
                .build(),
        ];

        self.collection
            .create_indexes(indexes)
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        tracing::info!("✓ Orders indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoOrderRepositoryPort for DemoOrderRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, order: &DemoOrder) -> DomainResult<DemoOrderId> {
        let model = DemoOrderModel::try_from(order.clone())?;

        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| DemoOrderId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoOrderId) -> DomainResult<Option<DemoOrder>> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoOrder"))?;

        let model = self
            .collection
            .find_one(doc! { "_id": oid, "deleted_at": { "$exists": false } })
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        Ok(model.map(DemoOrder::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoOrder>> {
        let filter = doc! { "deleted_at": { "$exists": false } };
        let cursor = self
            .collection
            .find(filter.clone())
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1, "_id": -1 })
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        let models: Vec<DemoOrderModel> =
            cursor.try_collect().await.map_err(|error| error::map(error, "DemoOrder"))?;
        let total = self
            .collection
            .count_documents(filter)
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        Ok(PageResult { items: models.into_iter().map(DemoOrder::from).collect(), total })
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_user_id(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<PageResult<DemoOrder>> {
        let oid = ObjectId::parse_str(&**user_id)
            .map_err(|_| DomainError::invalid_param("user_id", "DemoOrder"))?;
        let filter = doc! {
            "user_id": oid,
            "deleted_at": { "$exists": false }
        };

        let cursor = self
            .collection
            .find(filter.clone())
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1, "_id": -1 })
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        let models: Vec<DemoOrderModel> =
            cursor.try_collect().await.map_err(|error| error::map(error, "DemoOrder"))?;
        let total = self
            .collection
            .count_documents(filter)
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        Ok(PageResult { items: models.into_iter().map(DemoOrder::from).collect(), total })
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoOrderId, now: DateTime<Utc>) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "DemoOrder"))?;

        let now = bson::DateTime::from_chrono(now);

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! { "$set": { "deleted_at": now } },
            )
            .await
            .map_err(|error| error::map(error, "DemoOrder"))?;

        Ok(result.matched_count > 0)
    }
}
