use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use mongodb::bson::{self, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DemoUserModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub email: String,
    pub created_at: bson::DateTime,
    pub updated_at: bson::DateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<bson::DateTime>,
}

impl TryFrom<DemoUser> for DemoUserModel {
    type Error = DomainError;

    fn try_from(entity: DemoUser) -> DomainResult<Self> {
        let id = entity
            .id
            .map(|id| {
                ObjectId::parse_str(id.as_ref())
                    .map_err(|_| DomainError::invalid_param("id", "DemoUser"))
            })
            .transpose()?;

        Ok(Self {
            id,
            name: entity.name,
            email: entity.email,
            created_at: bson::DateTime::from_chrono(entity.created_at),
            updated_at: bson::DateTime::from_chrono(entity.updated_at),
            deleted_at: entity.deleted_at.map(bson::DateTime::from_chrono),
        })
    }
}

impl From<DemoUserModel> for DemoUser {
    fn from(model: DemoUserModel) -> Self {
        Self {
            id: model.id.map(|oid| DemoUserId::new(oid.to_hex())),
            name: model.name,
            email: model.email,
            created_at: model.created_at.to_chrono(),
            updated_at: model.updated_at.to_chrono(),
            deleted_at: model.deleted_at.map(|dt| dt.to_chrono()),
        }
    }
}
