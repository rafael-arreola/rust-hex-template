use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use crate::infrastructure::persistence::mongo::error;
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
}

impl TryFrom<DemoUser> for DemoUserModel {
    type Error = DomainError;

    fn try_from(entity: DemoUser) -> DomainResult<Self> {
        let id = entity.id.map(|id| error::parse_oid(id.as_ref(), "id", "DemoUser")).transpose()?;

        Ok(Self {
            id,
            name: entity.name,
            email: entity.email,
            created_at: bson::DateTime::from_chrono(entity.created_at),
            updated_at: bson::DateTime::from_chrono(entity.updated_at),
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
        }
    }
}
