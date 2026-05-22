use domain::entities::user::{User, UserId};
use domain::error::{DomainError, DomainResult};
use domain::pagination::Pagination;
use domain::port::user::UserRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct UserService {
    repo: Arc<dyn UserRepositoryPort>,
}

impl UserService {
    pub fn new(repo: Arc<dyn UserRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(%email))]
    pub async fn create_user(&self, name: &str, email: &str) -> DomainResult<User> {
        let existing = self.repo.find_by_email(email).await?;
        if existing.is_some() {
            return Err(DomainError::duplicate("User", "email", email));
        }

        let now = chrono::Utc::now();
        let mut user = User {
            id: None,
            name: name.to_string(),
            email: email.to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = self.repo.create(&user).await?;
        user.id = Some(id);

        tracing::info!(user_id = %user.id.as_deref().unwrap_or("unknown"), "User created");
        Ok(user)
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn get_user(&self, id: &UserId) -> DomainResult<User> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("User", id.to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_users(&self, pagination: Pagination) -> DomainResult<Vec<User>> {
        self.repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(%id, %email))]
    pub async fn update_user(&self, id: &UserId, name: &str, email: &str) -> DomainResult<User> {
        let mut user = self.get_user(id).await?;

        if email != user.email {
            let existing = self.repo.find_by_email(email).await?;
            if existing.is_some() {
                return Err(DomainError::duplicate("User", "email", email));
            }
        }

        user.name = name.to_string();
        user.email = email.to_string();
        user.updated_at = chrono::Utc::now();

        self.repo.update(id, &user).await?;

        tracing::info!("User updated");
        Ok(user)
    }

    #[tracing::instrument(skip_all)]
    pub async fn count_users(&self) -> DomainResult<u64> {
        self.repo.count().await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn delete_user(&self, id: &UserId) -> DomainResult<()> {
        let deleted = self.repo.delete(id).await?;
        if !deleted {
            return Err(DomainError::not_found("User", id.to_string()));
        }
        tracing::info!("User soft-deleted");
        Ok(())
    }
}
