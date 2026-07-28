use crate::domain::entities::demo_user::{DemoUser, DemoUserId};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::demo_user::DemoUserRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct DemoUserService {
    repo: Arc<dyn DemoUserRepositoryPort>,
}

impl DemoUserService {
    pub fn new(repo: Arc<dyn DemoUserRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(%email))]
    pub async fn create_user(&self, name: &str, email: &str) -> DomainResult<DemoUser> {
        let existing = self.repo.find_by_email(email).await?;
        if existing.is_some() {
            return Err(DomainError::duplicate("DemoUser", "email", email));
        }

        let now = chrono::Utc::now();
        let mut user = DemoUser {
            id: None,
            name: name.to_string(),
            email: email.to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = self.repo.create(&user).await?;
        user.id = Some(id);

        tracing::info!(user_id = %user.id.as_deref().unwrap_or("unknown"), "DemoUser created");
        Ok(user)
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn get_user(&self, id: &DemoUserId) -> DomainResult<DemoUser> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::not_found("DemoUser", id.to_string()))
    }

    #[tracing::instrument(skip_all)]
    pub async fn list_users(&self, pagination: Pagination) -> DomainResult<Vec<DemoUser>> {
        self.repo.find_all(pagination).await
    }

    #[tracing::instrument(skip_all, fields(%id, %email))]
    pub async fn update_user(
        &self,
        id: &DemoUserId,
        name: &str,
        email: &str,
    ) -> DomainResult<DemoUser> {
        let mut user = self.get_user(id).await?;

        if email != user.email {
            let existing = self.repo.find_by_email(email).await?;
            if existing.is_some() {
                return Err(DomainError::duplicate("DemoUser", "email", email));
            }
        }

        user.name = name.to_string();
        user.email = email.to_string();
        user.updated_at = chrono::Utc::now();

        self.repo.update(id, &user).await?;

        tracing::info!("DemoUser updated");
        Ok(user)
    }

    #[tracing::instrument(skip_all)]
    pub async fn count_users(&self) -> DomainResult<u64> {
        self.repo.count().await
    }

    #[tracing::instrument(skip_all, fields(%id))]
    pub async fn delete_user(&self, id: &DemoUserId) -> DomainResult<()> {
        let deleted = self.repo.delete(id).await?;
        if !deleted {
            return Err(DomainError::not_found("DemoUser", id.to_string()));
        }
        tracing::info!("DemoUser soft-deleted");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Canonical example of the testing seam described in `.agents/skills/test-entity`.
//
// The port (`Arc<dyn DemoUserRepositoryPort>`) is what makes this possible: an
// in-memory fake exercises the whole application service with no Mongo, no
// mocking crate, and no extra dependency in `Cargo.toml`.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The fake replicates the semantics of the real adapter that the business
    /// depends on: `create` assigns an ID, every read honours soft-delete, and
    /// `update`/`delete` report `false` when nothing matched.
    #[derive(Default)]
    struct FakeDemoUserRepository {
        users: Mutex<Vec<DemoUser>>,
    }

    #[async_trait::async_trait]
    impl DemoUserRepositoryPort for FakeDemoUserRepository {
        async fn create(&self, user: &DemoUser) -> DomainResult<DemoUserId> {
            let mut users = self.users.lock().unwrap();
            let id = DemoUserId::new(format!("{:024x}", users.len() + 1));
            let mut stored = user.clone();
            stored.id = Some(id.clone());
            users.push(stored);
            Ok(id)
        }

        async fn find_by_id(&self, id: &DemoUserId) -> DomainResult<Option<DemoUser>> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .iter()
                .find(|u| u.id.as_ref() == Some(id) && !u.is_deleted())
                .cloned())
        }

        async fn find_by_email(&self, email: &str) -> DomainResult<Option<DemoUser>> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .iter()
                .find(|u| u.email == email && !u.is_deleted())
                .cloned())
        }

        async fn find_all(&self, _pagination: Pagination) -> DomainResult<Vec<DemoUser>> {
            Ok(self.users.lock().unwrap().iter().filter(|u| !u.is_deleted()).cloned().collect())
        }

        async fn update(&self, id: &DemoUserId, user: &DemoUser) -> DomainResult<bool> {
            let mut users = self.users.lock().unwrap();
            match users.iter_mut().find(|u| u.id.as_ref() == Some(id) && !u.is_deleted()) {
                Some(existing) => {
                    *existing = user.clone();
                    existing.id = Some(id.clone());
                    Ok(true)
                }
                None => Ok(false),
            }
        }

        async fn delete(&self, id: &DemoUserId) -> DomainResult<bool> {
            let mut users = self.users.lock().unwrap();
            match users.iter_mut().find(|u| u.id.as_ref() == Some(id) && !u.is_deleted()) {
                Some(user) => {
                    user.deleted_at = Some(chrono::Utc::now());
                    Ok(true)
                }
                None => Ok(false),
            }
        }

        async fn count(&self) -> DomainResult<u64> {
            Ok(self.users.lock().unwrap().iter().filter(|u| !u.is_deleted()).count() as u64)
        }
    }

    fn service() -> DemoUserService {
        DemoUserService::new(Arc::new(FakeDemoUserRepository::default()))
    }

    const MISSING_ID: &str = "ffffffffffffffffffffffff";

    #[tokio::test]
    async fn create_user_assigns_id() {
        let user = service().create_user("Ada", "ada@example.com").await.unwrap();
        assert!(user.id.is_some());
    }

    // Errors are asserted by `code()`, never by message: the code is the
    // stable contract, the message is free to change.
    #[tokio::test]
    async fn create_user_rejects_duplicate_email() {
        let service = service();
        service.create_user("Ada", "ada@example.com").await.unwrap();

        let error = service.create_user("Ada II", "ada@example.com").await.unwrap_err();
        assert_eq!(error.code(), "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn get_user_maps_missing_to_not_found() {
        let error = service().get_user(&DemoUserId::new(MISSING_ID)).await.unwrap_err();
        assert_eq!(error.code(), "NOT_FOUND");
    }

    #[tokio::test]
    async fn update_user_rejects_email_taken_by_another_user() {
        let service = service();
        service.create_user("Ada", "ada@example.com").await.unwrap();
        let grace = service.create_user("Grace", "grace@example.com").await.unwrap();

        let error =
            service.update_user(&grace.id.unwrap(), "Grace", "ada@example.com").await.unwrap_err();
        assert_eq!(error.code(), "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn update_user_allows_keeping_its_own_email() {
        let service = service();
        let ada = service.create_user("Ada", "ada@example.com").await.unwrap();

        let updated =
            service.update_user(&ada.id.unwrap(), "Ada Lovelace", "ada@example.com").await.unwrap();
        assert_eq!(updated.name, "Ada Lovelace");
    }

    #[tokio::test]
    async fn deleted_user_is_invisible() {
        let service = service();
        let user = service.create_user("Ada", "ada@example.com").await.unwrap();
        let id = user.id.unwrap();

        service.delete_user(&id).await.unwrap();

        assert_eq!(service.get_user(&id).await.unwrap_err().code(), "NOT_FOUND");
        assert_eq!(service.count_users().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn delete_user_maps_missing_to_not_found() {
        let error = service().delete_user(&DemoUserId::new(MISSING_ID)).await.unwrap_err();
        assert_eq!(error.code(), "NOT_FOUND");
    }
}
