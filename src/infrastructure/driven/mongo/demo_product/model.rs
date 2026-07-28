use crate::domain::entities::demo_product::{
    DemoProduct, DemoProductId, DemoProductMetadata, DemoProductStatus,
};
use mongodb::bson::{self, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DemoProductModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub price: f64,
    pub stock: i32,
    pub status: DemoProductStatus,
    pub metadata: DemoProductMetadata,
    pub created_at: bson::DateTime,
    pub updated_at: bson::DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<bson::DateTime>,
}

impl From<DemoProduct> for DemoProductModel {
    fn from(entity: DemoProduct) -> Self {
        Self {
            id: entity.id.and_then(|id| ObjectId::parse_str(id.into_inner()).ok()),
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

impl From<DemoProductModel> for DemoProduct {
    fn from(model: DemoProductModel) -> Self {
        Self {
            id: model.id.map(|oid| DemoProductId::new(oid.to_hex())),
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
