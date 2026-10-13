use crate::domain::core::error::{DomainError, DomainResult};
use crate::domain::entities::demo_user::DemoUser;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateDemoUserInput {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

#[derive(Serialize)]
pub struct DemoUserOutput {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<DemoUser> for DemoUserOutput {
    type Error = DomainError;

    fn try_from(user: DemoUser) -> DomainResult<Self> {
        let id =
            user.id.ok_or_else(|| DomainError::internal("Persisted user has no identifier"))?;
        Ok(Self {
            id: id.into_inner(),
            name: user.name,
            email: user.email,
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        })
    }
}
