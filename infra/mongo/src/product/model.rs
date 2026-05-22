use domain::entities::product::{Product, ProductId, ProductMetadata, ProductStatus};
use mongodb::bson::{self, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub price: f64,
    pub stock: i32,
    pub status: ProductStatus,
    pub metadata: ProductMetadata,
    pub created_at: bson::DateTime,
    pub updated_at: bson::DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<bson::DateTime>,
}

impl From<Product> for ProductModel {
    fn from(entity: Product) -> Self {
        Self {
            id: entity
                .id
                .and_then(|id| ObjectId::parse_str(id.into_inner()).ok()),
            name: entity.name,
            price: entity.price,
            stock: entity.stock,
            status: entity.status,
            metadata: entity.metadata,
            created_at: bson::DateTime::from_chrono(entity.created_at),
            updated_at: bson::DateTime::from_chrono(entity.updated_at),
            deleted_at: entity.deleted_at.map(bson::DateTime::from_chrono),
        }
    }
}

impl From<ProductModel> for Product {
    fn from(model: ProductModel) -> Self {
        Self {
            id: model.id.map(|oid| ProductId::new(oid.to_hex())),
            name: model.name,
            price: model.price,
            stock: model.stock,
            status: model.status,
            metadata: model.metadata,
            created_at: model.created_at.to_chrono(),
            updated_at: model.updated_at.to_chrono(),
            deleted_at: model.deleted_at.map(|dt| dt.to_chrono()),
        }
    }
}
