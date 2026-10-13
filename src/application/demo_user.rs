use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::domain::core::pagination::{PageResult, Pagination};
use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use crate::domain::port::repositories::demo_user::DemoUserRepositoryPort;
use chrono::Utc;
use std::sync::Arc;

#[derive(Clone)]
pub struct DemoUserService {
    repo: Arc<dyn DemoUserRepositoryPort>,
}

impl DemoUserService {
    pub fn new(repo: Arc<dyn DemoUserRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(operation = "create_user"))]
    pub async fn create_user(&self, name: &str, email: &str) -> DomainResult<DemoUser> {
        let now = Utc::now();
        let mut user = DemoUser {
            id: None,
            name: name.to_string(),
            email: email.to_string(),
            created_at: now,
            updated_at: now,
        };

        let id = self.repo.create(&user).await?;
        tracing::info!(user_id = %id, "DemoUser created");
        user.id = Some(id);
        Ok(user)
    }

    #[tracing::instrument(skip_all, fields(operation = "get_user"))]
    pub async fn get_user(&self, id: &DemoUserId) -> DomainResult<DemoUser> {
        self.repo.find_by_id(id).await?.ok_or_else(|| not_found(id))
    }

    #[tracing::instrument(skip_all, fields(page = pagination.page(), limit = pagination.limit()))]
    pub async fn list_users(&self, pagination: Pagination) -> DomainResult<PageResult<DemoUser>> {
        self.repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(operation = "update_user"))]
    pub async fn update_user(
        &self,
        id: &DemoUserId,
        name: &str,
        email: &str,
    ) -> DomainResult<DemoUser> {
        let mut user = self.get_user(id).await?;

        let now = Utc::now();
        user.name = name.to_string();
        user.email = email.to_string();
        user.updated_at = now;

        if !self.repo.update(id, &user, now).await? {
            return Err(not_found(id));
        }

        tracing::info!("DemoUser updated");
        Ok(user)
    }

    #[tracing::instrument(skip_all, fields(operation = "delete_user"))]
    pub async fn delete_user(&self, id: &DemoUserId) -> DomainResult<()> {
        if !self.repo.delete(id).await? {
            return Err(not_found(id));
        }
        tracing::info!("DemoUser deleted");
        Ok(())
    }
}

fn not_found(id: &DemoUserId) -> DomainError {
    DomainError::new(StatusCode::NOT_FOUND, format!("DemoUser id {id} not found"))
        .with_public("DemoUser not found")
        .with_trace_code("DEMO_USER_NOT_FOUND")
}
