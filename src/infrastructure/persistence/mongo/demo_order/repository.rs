use crate::domain::core::error::DomainResult;
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_order::{DemoOrder, DemoOrderId};
use crate::domain::entities::demo_user::DemoUserId;
use crate::domain::port::repositories::demo_order::DemoOrderRepositoryPort;
use crate::infrastructure::persistence::mongo::{
    delete_by_id, demo_order::model::DemoOrderModel, error, find_by_id, insert_one, paginate,
    primary_collection,
};
use async_trait::async_trait;
use mongodb::{Collection, Database, IndexModel, bson::doc, options::IndexOptions};

const ENTITY: &str = "DemoOrder";

#[derive(Clone)]
pub struct DemoOrderRepository {
    collection: Collection<DemoOrderModel>,
}

impl DemoOrderRepository {
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: primary_collection(db, "orders") };
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
                .keys(doc! { "user_id": 1, "created_at": -1, "_id": -1 })
                .options(
                    IndexOptions::builder()
                        .name("user_created_id_compound_idx".to_string())
                        .build(),
                )
                .build(),
        ];

        self.collection.create_indexes(indexes).await.map_err(|error| error::map(error, ENTITY))?;

        tracing::info!("✓ Orders indexes created");
        Ok(())
    }
}

#[async_trait]
impl DemoOrderRepositoryPort for DemoOrderRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, order: &DemoOrder) -> DomainResult<DemoOrderId> {
        insert_one(&self.collection, order, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_id(&self, id: &DemoOrderId) -> DomainResult<Option<DemoOrder>> {
        find_by_id(&self.collection, id, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<PageResult<DemoOrder>> {
        paginate(&self.collection, doc! {}, pagination, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn find_by_user_id(
        &self,
        user_id: &DemoUserId,
        pagination: Pagination,
    ) -> DomainResult<PageResult<DemoOrder>> {
        let oid = error::parse_oid(user_id, "user_id", ENTITY)?;
        paginate(&self.collection, doc! { "user_id": oid }, pagination, ENTITY).await
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &DemoOrderId) -> DomainResult<bool> {
        delete_by_id(&self.collection, id, ENTITY).await
    }
}
