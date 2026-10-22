use crate::domain::core::error::DomainResult;
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use crate::domain::port::repositories::demo_user::DemoUserRepositoryPort;
use crate::infrastructure::persistence::mongo::{
    delete_by_id, demo_user::model::DemoUserModel, error, find_by_id, insert_one, paginate,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::{
    Collection, Database, IndexModel,
    bson::{self, doc},
    options::IndexOptions,
};

const ENTITY: &str = "DemoUser";

#[derive(Clone)]
pub struct DemoUserRepository {
    collection: Collection<DemoUserModel>,
}

impl DemoUserRepository {
    /// Building the repository ensures its indexes exist.
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<DemoUserModel>("users") };
        repo.create_indexes().await?;
        Ok(repo)
    }

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
                .keys(doc! { "created_at": -1, "_id": -1 })
                .options(
                    IndexOptions::builder().name("created_id_compound_idx".to_string()).build(),
                )
                .build(),
        ];

        self.collection.create_indexes(indexes).await.map_err(|error| error::map(error, ENTITY))?;

        tracing::info!("✓ DemoUser indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoUserRepositoryPort for DemoUserRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, user: &DemoUser) -> DomainResult<DemoUserId> {
        insert_one(&self.collection, user, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoUserId) -> DomainResult<Option<DemoUser>> {
        find_by_id(&self.collection, id, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoUser>> {
        paginate(&self.collection, doc! {}, pagination, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn update(
        &self,
        id: &DemoUserId,
        user: &DemoUser,
        now: DateTime<Utc>,
    ) -> DomainResult<bool> {
        let oid = error::parse_oid(id, "id", ENTITY)?;
        let now = bson::DateTime::from_chrono(now);
        let result = self
            .collection
            .update_one(
                doc! { "_id": oid },
                doc! {
                    "$set": {
                        "name": &user.name,
                        "email": &user.email,
                        "updated_at": now,
                    }
                },
            )
            .await
            .map_err(|error| error::map(error, ENTITY))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoUserId) -> DomainResult<bool> {
        delete_by_id(&self.collection, id, ENTITY).await
    }
}
