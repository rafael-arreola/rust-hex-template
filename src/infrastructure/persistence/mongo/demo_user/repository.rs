use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use crate::domain::port::demo_user::DemoUserRepositoryPort;
use crate::infrastructure::persistence::mongo::{demo_user::model::DemoUserModel, error};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{self, doc, oid::ObjectId},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct DemoUserRepository {
    collection: Collection<DemoUserModel>,
}

impl DemoUserRepository {
    /// Building the repository ensures its indexes exist.
    ///
    /// Index creation lives here — not in `main.rs` — so "the indexes are
    /// there" is a property of the type instead of a wiring step someone can
    /// forget. Holding a `DemoUserRepository` means its indexes were created.
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<DemoUserModel>("users") };
        repo.create_indexes().await?;
        Ok(repo)
    }

    /// Create database indexes (idempotent — safe to call on every startup).
    /// Private: `new` is the only caller, by design.
    async fn create_indexes(&self) -> DomainResult<()> {
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
                .keys(doc! { "deleted_at": 1, "created_at": -1, "_id": -1 })
                .options(
                    IndexOptions::builder()
                        .name("deleted_created_id_compound_idx".to_string())
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
            .map_err(|error| error::map(error, "DemoUser"))?;

        tracing::info!("✓ DemoUser indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoUserRepositoryPort for DemoUserRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, user: &DemoUser) -> DomainResult<DemoUserId> {
        let model = DemoUserModel::try_from(user.clone())?;
        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| DemoUserId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoUserId) -> DomainResult<Option<DemoUser>> {
        let oid =
            ObjectId::parse_str(&**id).map_err(|_| DomainError::invalid_param("id", "DemoUser"))?;

        let model = self
            .collection
            .find_one(doc! { "_id": oid, "deleted_at": { "$exists": false } })
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        Ok(model.map(DemoUser::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_email(&self, email: &str) -> DomainResult<Option<DemoUser>> {
        let model = self
            .collection
            .find_one(doc! {
                "email": email,
                "deleted_at": { "$exists": false }
            })
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        Ok(model.map(DemoUser::from))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoUser>> {
        let filter = doc! { "deleted_at": { "$exists": false } };
        let cursor = self
            .collection
            .find(filter.clone())
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1, "_id": -1 })
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        let models: Vec<DemoUserModel> =
            cursor.try_collect().await.map_err(|error| error::map(error, "DemoUser"))?;
        let total = self
            .collection
            .count_documents(filter)
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        Ok(PageResult { items: models.into_iter().map(DemoUser::from).collect(), total })
    }

    #[tracing::instrument(skip_all)]
    async fn update(
        &self,
        id: &DemoUserId,
        user: &DemoUser,
        now: DateTime<Utc>,
    ) -> DomainResult<bool> {
        let oid =
            ObjectId::parse_str(&**id).map_err(|_| DomainError::invalid_param("id", "DemoUser"))?;

        let now = bson::DateTime::from_chrono(now);
        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! {
                    "$set": {
                        "name": &user.name,
                        "email": &user.email,
                        "updated_at": now,
                    }
                },
            )
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoUserId, now: DateTime<Utc>) -> DomainResult<bool> {
        let oid =
            ObjectId::parse_str(&**id).map_err(|_| DomainError::invalid_param("id", "DemoUser"))?;

        let now = bson::DateTime::from_chrono(now);

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! { "$set": { "deleted_at": now } },
            )
            .await
            .map_err(|error| error::map(error, "DemoUser"))?;

        Ok(result.matched_count > 0)
    }
}
