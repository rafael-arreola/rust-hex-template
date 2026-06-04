use crate::user::model::UserModel;
use async_trait::async_trait;
use domain::entities::user::{User, UserId};
use domain::error::{DomainError, DomainResult};
use domain::pagination::Pagination;
use domain::port::user::UserRepositoryPort;
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct UserRepository {
    collection: Collection<UserModel>,
}

impl UserRepository {
    pub fn new(db: &Database) -> Self {
        Self { collection: db.collection::<UserModel>("users") }
    }

    /// Create database indexes (idempotent — safe to call on every startup)
    pub async fn create_indexes(&self) -> DomainResult<()> {
        let indexes = vec![
            IndexModel::builder()
                .keys(doc! { "email": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .name("email_unique_idx".to_string())
                        .build(),
                )
                .build(),
            IndexModel::builder()
                .keys(doc! { "deleted_at": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("deleted_created_compound_idx".to_string())
                        .build(),
                )
                .build(),
            IndexModel::builder()
                .keys(doc! { "deleted_at": 1, "email": 1 })
                .options(
                    IndexOptions::builder().name("deleted_email_compound_idx".to_string()).build(),
                )
                .build(),
        ];

        self.collection
            .create_indexes(indexes)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        tracing::info!("✓ User indexes created");
        Ok(())
    }
}

#[async_trait]
impl UserRepositoryPort for UserRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, user: &User) -> DomainResult<UserId> {
        let model = UserModel::from(user.clone());
        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| UserId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &UserId) -> DomainResult<Option<User>> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "User", &**id))?;

        let model = self
            .collection
            .find_one(doc! { "_id": oid, "deleted_at": { "$exists": false } })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(model.map(User::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_email(&self, email: &str) -> DomainResult<Option<User>> {
        let model = self
            .collection
            .find_one(doc! {
                "email": email,
                "deleted_at": { "$exists": false }
            })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(model.map(User::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<User>> {
        let cursor = self
            .collection
            .find(doc! { "deleted_at": { "$exists": false } })
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1 })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        let models: Vec<UserModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(User::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn update(&self, id: &UserId, user: &User) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "User", &**id))?;

        let model = UserModel::from(user.clone());
        let bson_doc = mongodb::bson::serialize_to_document(&model)
            .map_err(|e| DomainError::internal(e.to_string()))?;

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! { "$set": bson_doc },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &UserId) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "User", &**id))?;

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
