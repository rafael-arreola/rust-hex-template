use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::entities::demo_product::{
    DemoProduct, DemoProductId, DemoProductMetadata, DemoProductStatus,
};
use crate::infrastructure::persistence::mongo::error;
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
}

impl TryFrom<DemoProduct> for DemoProductModel {
    type Error = DomainError;

    fn try_from(entity: DemoProduct) -> DomainResult<Self> {
        let id =
            entity.id.map(|id| error::parse_oid(id.as_ref(), "id", "DemoProduct")).transpose()?;

        Ok(Self {
            id,
            name: entity.name,
            price: entity.price,
            stock: entity.stock,
            status: entity.status,
            metadata: entity.metadata,
            created_at: bson::DateTime::from_chrono(entity.created_at),
            updated_at: bson::DateTime::from_chrono(entity.updated_at),
        })
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
        }
    }
}
