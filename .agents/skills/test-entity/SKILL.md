---
name: test-entity
description: Escribir tests para un application service usando puertos falsos (fakes) y tests HTTP con tower::ServiceExt. Úsala cuando pidan tests de un servicio, caso de uso o handler, o al cerrar el scaffold de una entidad nueva.
---

# Testear una entidad

Los puertos (`Arc<dyn Port>`) son la costura de testing del template: un fake en memoria que implementa el trait permite testear todo el application service sin Mongo, sin mocks de terceros y sin tocar `Cargo.toml`.

Convenciones:

- Tests unitarios inline en `#[cfg(test)] mod tests` dentro del archivo que testean (patrón existente en `server.rs` y `middleware.rs`).
- `unwrap`/`expect` **sí** están permitidos en tests (`clippy.toml` los re-permite ahí).
- No agregues crates de mocking (mockall, etc.) — el fake manual es suficiente y respeta la higiene de dependencias. Si algún día se decide lo contrario, se decide en `AGENTS.md` primero.

## 1. Fake del puerto (en memoria)

Al final de `src/application/demo_user.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::demo_user::DemoUser;
    use std::sync::Mutex;

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
```

El fake replica las semánticas del adapter real que importan al negocio: asigna ID en `create`, **respeta soft-delete** en todas las lecturas, y `update`/`delete` devuelven `false` cuando no hay match.

## 2. Tests del service — qué cubrir siempre

Caso feliz, cada regla semántica, y cada `DomainError` esperado **por su `code()`** (es el contrato estable):

```rust
    #[tokio::test]
    async fn create_user_assigns_id() {
        let user = service().create_user("Ada", "ada@example.com").await.unwrap();
        assert!(user.id.is_some());
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_email() {
        let service = service();
        service.create_user("Ada", "ada@example.com").await.unwrap();

        let err = service.create_user("Ada II", "ada@example.com").await.unwrap_err();
        assert_eq!(err.code(), "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn get_user_maps_missing_to_not_found() {
        let err = service().get_user(&DemoUserId::new("ffffffffffffffffffffffff")).await.unwrap_err();
        assert_eq!(err.code(), "NOT_FOUND");
    }

    #[tokio::test]
    async fn deleted_user_is_invisible() {
        let service = service();
        let user = service.create_user("Ada", "ada@example.com").await.unwrap();
        let id = user.id.unwrap();

        service.delete_user(&id).await.unwrap();
        assert_eq!(service.get_user(&id).await.unwrap_err().code(), "NOT_FOUND");
    }
}
```

Para services con varios puertos (`DemoOrderService`), crea un fake por puerto y testea las reglas cruzadas (stock insuficiente → `BUSINESS_RULE_VIOLATION`, usuario inexistente → `NOT_FOUND`).

Los tests canónicos del contrato de errores viven en `src/domain/error.rs` (`mod tests`): separación vista pública/interna (`public_message()` vs `Display`) y severidad por variante. Si agregas una variante a `DomainError`, extiende esos tests — en particular `infrastructure_detail_never_leaks_into_public_message` si la variante carga detalle de infraestructura.

## 3. Tests HTTP (extractores, envelope, negociación)

Para comportamiento de la capa HTTP usa el patrón ya establecido en los tests de `src/infrastructure/driving/http_axum/server.rs`: un `Router` mínimo + `tower::ServiceExt::oneshot`, verificando status, headers y el envelope (`trace_id`, `data`, `cause`). Ahí se testean extractores, middleware y DTOs — no lógica de negocio (esa ya quedó cubierta en el service).

## 4. Ejecutar

```bash
cargo test              # todo
cargo test --lib user   # módulo específico
```

Los tests de domain services son aún más simples: `#[test]` síncrono puro (ver skill `add-domain-service`).
