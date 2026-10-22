pub mod demo_order;
pub mod demo_product;
pub mod demo_user;
mod error;

use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database,
    bson::{Document, doc},
    options::{CollectionOptions, ReadPreference, SelectionCriteria},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::core::values::DomainId;

/// Returns a collection handle whose reads always route to the replica-set primary.
fn primary_collection<T>(db: &Database, name: &str) -> Collection<T>
where
    T: Send + Sync,
{
    db.collection_with_options::<T>(
        name,
        CollectionOptions::builder()
            .selection_criteria(SelectionCriteria::ReadPreference(ReadPreference::Primary))
            .build(),
    )
}

/// Converts `entity_val` into its BSON model `M`, inserts it and returns the assigned `DomainId`.
async fn insert_one<M, E, T>(
    collection: &Collection<M>,
    entity_val: &E,
    entity: &'static str,
) -> DomainResult<DomainId<T>>
where
    M: TryFrom<E, Error = DomainError> + Serialize + Send + Sync,
    E: Clone,
{
    let model = M::try_from(entity_val.clone())?;
    let result = collection.insert_one(model).await.map_err(|err| error::map(err, entity))?;
    result.inserted_id.as_object_id().map(|oid| DomainId::new(oid.to_hex())).ok_or_else(|| {
        DomainError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get inserted ID")
            .with_trace_code("MONGO_INSERT_ID_MISSING")
    })
}

/// Loads one document by its `_id` and converts the BSON model `M` into its domain entity `E`.
async fn find_by_id<M, E, T>(
    collection: &Collection<M>,
    id: &DomainId<T>,
    entity: &'static str,
) -> DomainResult<Option<E>>
where
    M: DeserializeOwned + Unpin + Send + Sync,
    E: From<M>,
{
    let oid = error::parse_oid(id, "id", entity)?;
    let model =
        collection.find_one(doc! { "_id": oid }).await.map_err(|err| error::map(err, entity))?;
    Ok(model.map(E::from))
}

/// Deletes one document by its `_id` and reports whether a document was removed.
async fn delete_by_id<M, T>(
    collection: &Collection<M>,
    id: &DomainId<T>,
    entity: &'static str,
) -> DomainResult<bool>
where
    M: Send + Sync,
{
    let oid = error::parse_oid(id, "id", entity)?;
    let result =
        collection.delete_one(doc! { "_id": oid }).await.map_err(|err| error::map(err, entity))?;
    Ok(result.deleted_count > 0)
}

/// Executes the paginated cursor query and document count concurrently over the same filter,
/// converting each BSON model `M` into its domain entity `E`.
async fn paginate<M, E>(
    collection: &Collection<M>,
    filter: Document,
    pagination: Pagination,
    entity: &'static str,
) -> DomainResult<PageResult<E>>
where
    M: DeserializeOwned + Unpin + Send + Sync,
    E: From<M>,
{
    let find_fut = async {
        let cursor = collection
            .find(filter.clone())
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1, "_id": -1 })
            .await
            .map_err(|err| error::map(err, entity))?;

        cursor.try_collect::<Vec<M>>().await.map_err(|err| error::map(err, entity))
    };
    let count_fut = async {
        collection.count_documents(filter.clone()).await.map_err(|err| error::map(err, entity))
    };

    let (models, total) = tokio::try_join!(find_fut, count_fut)?;
    Ok(PageResult { items: models.into_iter().map(E::from).collect(), total })
}
