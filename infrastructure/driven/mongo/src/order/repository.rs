use crate::order::model::OrderModel;
use async_trait::async_trait;
use domain::entities::order::{Order, OrderId};
use domain::entities::user::UserId;
use domain::error::{DomainError, DomainResult};
use domain::pagination::Pagination;
use domain::port::order::OrderRepositoryPort;
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct OrderRepository {
    collection: Collection<OrderModel>,
}

impl OrderRepository {
    pub fn new(db: &Database) -> Self {
        Self { collection: db.collection::<OrderModel>("orders") }
    }

    /// Create database indexes (idempotent — safe to call on every startup)
    pub async fn create_indexes(&self) -> DomainResult<()> {
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
impl OrderRepositoryPort for OrderRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, order: &Order) -> DomainResult<OrderId> {
        let model = OrderModel::from(order.clone());

        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| OrderId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &OrderId) -> DomainResult<Option<Order>> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "Order", &**id))?;

        let model = self
            .collection
            .find_one(doc! { "_id": oid, "deleted_at": { "$exists": false } })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(model.map(Order::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<Order>> {
        let cursor = self
            .collection
            .find(doc! { "deleted_at": { "$exists": false } })
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1 })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        let models: Vec<OrderModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(Order::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
        pagination: Pagination,
    ) -> DomainResult<Vec<Order>> {
        let oid = ObjectId::parse_str(&**user_id)
            .map_err(|_| DomainError::invalid_param("user_id", "Order", &**user_id))?;

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

        let models: Vec<OrderModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(Order::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &OrderId) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "Order", &**id))?;

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
