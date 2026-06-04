# Target Outcome

Produce a Rust workspace following a hexagonal/clean architecture with Axum HTTP, MongoDB, and Redis. Every entity follows identical structural patterns so the codebase is predictable across all contributors. The agent may flag missing context, anticipate edge cases, and propose shortcuts—but respects the existing architecture and avoids unwarranted rewrites.

# Success Criteria

- All entities follow the same directory structure, naming conventions, and template patterns below.
- Layer dependencies are strictly enforced: `driving/http-axum → application → domain ← driven/mongo/redis`.
- All external errors are mapped to `DomainError` variants with stable, machine-readable codes.
- Health check endpoints (`/healthz`, `/readyz`) and `X-Request-Id` middleware are present in every service.
- Cargo dependencies are sorted alphabetically; unused dependencies are removed.
- `rustfmt.toml` and `clippy.toml` enforce consistent formatting and linting across the workspace.

# Invariants

These rules are non-negotiable. They exist to make the codebase predictable across entities and contributors.

## Naming Conventions

| Scope                  | Rule                                  | Example ✅                          | Avoid ❌                    |
| ---------------------- | ------------------------------------- | ----------------------------------- | --------------------------- |
| Files & folders        | singular                              | `user.rs`, `product/`               | `users.rs`, `products/`     |
| Structs                | PascalCase, singular                  | `User`, `Order`                     | `Users`, `Orders`           |
| Port traits            | `{Entity}RepositoryPort`              | `UserRepositoryPort`                | `UserRepository` (as trait) |
| Infrastructure structs | `{Entity}Repository` — no tech prefix | `UserRepository` (in `infra_mongo`) | `MongoUserRepository`       |
| DB collections/tables  | plural, snake_case                    | `users`, `order_items`              | `user`, `orderItems`        |
| API routes             | plural                                | `/api/v1/users`, `/api/v1/orders`   | `/api/v1/user`              |
| DTOs                   | `*Input` / `*Output` suffix           | `CreateUserInput`, `UserOutput`     | `UserDto`, `UserRequest`    |
| Variables & fields     | full words, no abbreviations          | `user_email`, `page_number`         | `usr`, `idx`, `tmp`         |

## Cargo Workspace Directory Structure (mandatory)

```
domain/src/entities.rs                          → Entity module router
domain/src/entities/{entity}.rs                 → Entity struct + typed ID + marker
domain/src/port.rs                              → Port module router
domain/src/port/{entity}.rs                     → trait {Entity}RepositoryPort
domain/src/services.rs                          → Domain services module router
domain/src/services/{service}.rs                → Pure business logic (no I/O, no deps)
domain/src/error.rs                             → DomainError enum + DomainResult<T>
domain/src/values.rs                            → DomainId<T>
domain/src/pagination.rs                        → Pagination struct
domain/src/macros.rs                            → Macros module router
domain/src/macros/json.rs                       → JSON serialization macros (as_json!)
domain/src/lib.rs                               → Domain crate root (pub mod)

application/src/{entity}.rs                     → {Entity}Service (use case orchestration)
application/src/shared/mod.rs                   → Reusable sub-flows WITH I/O
application/src/lib.rs                          → Application crate root (pub mod)

infrastructure/driven/mongo/src/{entity}.rs     → {Entity} module router
infrastructure/driven/mongo/src/{entity}/model.rs   → {Entity}Model (BSON/serde)
infrastructure/driven/mongo/src/{entity}/repository.rs → {Entity}Repository
infrastructure/driven/mongo/src/provider.rs     → MongoDB connection provider
infrastructure/driven/mongo/src/lib.rs          → Mongo crate root (pub mod)

infrastructure/driven/redis/src/lib.rs          → Redis connections & helpers

infrastructure/driving/http-axum/src/routes.rs              → Router registration
infrastructure/driving/http-axum/src/routes/{entity}.rs     → Axum handlers
infrastructure/driving/http-axum/src/server.rs              → Server module router
infrastructure/driving/http-axum/src/server/error.rs        → ApiError definition
infrastructure/driving/http-axum/src/server/health.rs       → Health check endpoints (/healthz, /readyz)
infrastructure/driving/http-axum/src/server/middleware.rs   → Cross-cutting HTTP middleware (e.g. X-Request-Id)
infrastructure/driving/http-axum/src/server/response.rs     → GenericApiResponse
infrastructure/driving/http-axum/src/server/state.rs        → AppState (Services container)
infrastructure/driving/http-axum/src/server/validation.rs   → Validation utilities
infrastructure/driving/http-axum/src/lib.rs                 → HTTP-Axum crate root (pub mod)

shared/src/config.rs                             → Environment configuration (loaded once)
shared/src/tracer.rs                             → OpenTelemetry & tracing setup
shared/src/lib.rs                                → Shared crate root (pub mod)

cmd/service/src/main.rs                          → Composition Root & DI wiring (Main binary)

rustfmt.toml                                     → Rustfmt configuration (workspace-wide, mandatory)
clippy.toml                                      → Clippy configuration (workspace-wide, mandatory)
```

Module routers use the modern Rust convention: when a directory `foo/` contains submodules, the parent module is `foo.rs` at the same level as the directory — never `foo/mod.rs`. Every new file is exported with `pub mod` in its parent router or `lib.rs`.

## Layer Dependencies (Enforced by Cargo Workspace)

```
driving/http-axum ──> application ──> domain <── driven/mongo, driven/redis
```

| Crate / Layer       | May import                                                   | Forbidden to import                             |
| ------------------- | ------------------------------------------------------------ | ----------------------------------------------- |
| `domain`            | Nothing outside itself (zero local crates)                   | Everything else                                 |
| `application`       | Only `domain`, `shared`                                      | `infra-mongo`, `infra-redis`, `infra-http-axum` |
| `infra-mongo/redis` | Only `domain`, `shared`                                      | `application`, `infra-http-axum`                |
| `infra-http-axum`   | `domain`, `application`, framework deps, observability types | `infra-mongo`, `infra-redis`, SDK config deps   |
| `shared`            | Only external crates (zero project crates)                   | `domain`, `application`, `infra-*`              |

## What may cross crate boundaries

✅ Primitives (`String`, `i32`, `bool`, `f64`, etc.), `DateTime<Utc>`, domain entities, domain enums, domain typed IDs.

❌ DTOs (`*Input`, `*Output`) outside `infra-http-axum` &nbsp;|&nbsp; ❌ Models (`*Model`) outside `infra-mongo` &nbsp;|&nbsp; ❌ DB driver types (`bson::ObjectId`) outside `infra-mongo`.

## Three essential templates

### Port (`domain/src/port/{entity}.rs`)

```rust
use crate::entities::user::{User, UserId};
use crate::error::DomainResult;
use crate::pagination::Pagination;
use async_trait::async_trait;

#[async_trait]
pub trait UserRepositoryPort: Send + Sync {
    async fn create(&self, user: &User) -> DomainResult<UserId>;
    async fn find_by_id(&self, id: &UserId) -> DomainResult<Option<User>>;
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<User>>;
    async fn update(&self, id: &UserId, user: &User) -> DomainResult<bool>;
    async fn delete(&self, id: &UserId) -> DomainResult<bool>;
}
```

- Ports only for Aggregate Roots. Not every entity needs a repository.
- Methods receive and return only domain types and primitives.
- Every port trait uses `#[async_trait]` and is bounded by `Send + Sync`.

### Service (`application/src/{entity}.rs`)

```rust
use domain::port::user::UserRepositoryPort;
use domain::entities::user::User;
use domain::error::DomainResult;
use std::sync::Arc;

pub struct UserService {
    repo: Arc<dyn UserRepositoryPort>,
}

impl UserService {
    pub fn new(repo: Arc<dyn UserRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all)]
    pub async fn create_user(&self, email: &str) -> DomainResult<User> {
        // ...
    }
}
```

- Constructor injection via `Arc<dyn Port>` (dynamic dispatch).
- Every public method instrumented with `#[tracing::instrument(skip_all)]`.
- Parameters are primitives, typed IDs, or domain values. Never DTOs.

### Domain Service (`domain/src/services/{service}.rs`)

```rust
use crate::entities::order::Order;

/// Pure business logic — zero I/O, zero constructor dependencies.
pub struct PricingService;

impl PricingService {
    pub fn new() -> Self { Self }

    pub fn apply_discount(&self, order: &Order) -> f64 {
        if order.total_price > 1000.0 { order.total_price * 0.90 } else { order.total_price }
    }
}
```

- Stateless. No constructor parameters.
- Operates exclusively on domain entities and primitives.
- Called from application services — never from infrastructure.

### Repository (`infrastructure/driven/mongo/src/{entity}/repository.rs`)

```rust
use crate::user::model::UserModel;
use async_trait::async_trait;
use domain::entities::user::{User, UserId};
use domain::error::DomainResult;
use domain::port::user::UserRepositoryPort;

#[derive(Clone)]
pub struct UserRepository { /* collection / pool */ }

#[async_trait]
impl UserRepositoryPort for UserRepository {
    async fn create(&self, user: &User) -> DomainResult<UserId> {
        let model = UserModel::from(user.clone());
        // Map every driver error with .map_err(|e| DomainError::database(...))
    }
}
```

- Implements `From<Entity> for Model` and `From<Model> for Entity` in `model.rs`.
- Map all external errors with `.map_err(...)`. Driver errors never propagate raw.

## Handler rules

Handlers do zero business logic. Their job:

1. Validate input via `ValidatedJson` (backed by `validator` crate).
2. Convert string path/query params to typed IDs.
3. Call the service with primitives/domain values.
4. Convert the domain result into an output DTO.
5. Wrap it in `GenericApiResponse`.

## Error rules

- All domain/usecase functions return `DomainResult<T>`.
- Do not use `unwrap()` or `expect()`.
- Map every external error with `.map_err(...)`.
- The `DomainError` enum carries all logic errors; build it via constructor methods.
- Every variant exposes a stable, machine-readable code via `DomainError::code()`:

```rust
#[derive(Error, Debug)]
pub enum DomainError {
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },
    #[error("{entity} already exists: {details}")]
    AlreadyExists { entity: &'static str, details: String },
    #[error("Invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("Internal: {0}")]
    Internal(String),
}

impl DomainError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "NOT_FOUND",
            Self::AlreadyExists { .. } => "ALREADY_EXISTS",
            Self::Invalid { .. } => "INVALID_INPUT",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }
}
pub type DomainResult<T> = std::result::Result<T, DomainError>;
```

- `ApiError` in the HTTP layer is a struct — not an enum — with `code`, `message`, and `status`. The `From<DomainError>` mapping centralizes the code-to-status relationship in one place.
- Error responses follow the shape `{ "trace_id": "...", "error": { "code": "NOT_FOUND", "message": "..." } }`.

## Response format

1. One-line architectural decision.
2. Code in dependency order: `domain` → `application` → `infra-mongo`/`infra-redis` → `infra-http-axum` → `main.rs`.
3. Trade-offs only if complexity demands it.

## Logging & Structured Telemetry

To log complete domain objects, entities, or DTOs in telemetry or tracing events, use the `as_json!` macro exported by the `domain` crate instead of `?` (Debug) format or manual serialization.

Inject the field using the `%` prefix to indicate a formatted string.

```rust
use domain::as_json;

tracing::info!(user = %as_json!(&user), "User created successfully");
```

## Architectural Boundaries & Concurrency

### Thread Safety (`Send + Sync`)

Axum and Tokio distribute request execution concurrently across multiple worker threads. Any struct, service, or port that crosses application layers must be safe to share across threads:

- All port traits in the `domain` layer are explicitly bounded by `Send + Sync`.
- Async traits are decorated with `#[async_trait]`.

```rust
#[async_trait]
pub trait UserRepositoryPort: Send + Sync { ... }
```

### Data Validation Boundaries (DTOs vs. Domain)

- **Syntactic Validation (HTTP Layer - DTOs):** Basic data structure and format (e.g., string length, email format, positive numbers) in `*Input` DTOs using the `validator` library.
- **Semantic Validation (Use Cases Layer - Domain):** Complex business rules and state consistency (e.g., email uniqueness, stock availability, transactional limits) by querying domain ports.

### Infrastructure Error Encapsulation

No database driver error (`mongodb::error::Error`, `redis::RedisError`) or external dependency error propagates to upper layers (`application` or `domain`):

- Infrastructure adapters intercept all technology-specific errors with `.map_err(...)`.
- Map these errors using the corresponding constructors of `DomainError` (e.g., `DomainError::database`, `DomainError::internal`).

```rust
self.collection
    .insert_one(model)
    .await
    .map_err(|e| DomainError::database(e.to_string()))?
```

## Dependency Sorting (`cargo-sort`)

- All dependency blocks (`[dependencies]`, `[workspace.dependencies]`, etc.) are sorted alphabetically and grouped by nature.
- Run `cargo sort -w -g` to check and apply changes across all workspace crates before committing dependency changes.

## Cargo.toml Hygiene

Every crate declares only dependencies it actually imports in its source code. The definitive test is `cargo check -p <crate>` — if it compiles without a dependency, that dependency does not belong.

## Provider Fail-Fast

All infrastructure providers (MongoDB, Redis, etc.) instantiated in `cmd/service/src/main.rs` follow the same fail-fast pattern:

```rust
let provider = match Provider::new(&url).await {
    Ok(p) => p,
    Err(e) => {
        tracing::error!("Failed to connect to Provider: {}", e);
        return;
    }
};
```

Index creation follows the same contract — if indexes cannot be ensured at startup, the service does not start:

```rust
if let Err(e) = repository.create_indexes().await {
    tracing::error!("Failed to create indexes: {}", e);
    return;
}
```

## Model Conversion Consistency

All `{Entity}Model` structs in `infrastructure/driven/mongo/src/{entity}/model.rs` implement `From<Entity> for Model` and `From<Model> for Entity`. Do not use `TryFrom` — it introduces an inconsistent pattern across entities. Invalid IDs are handled silently via `.unwrap_or_default()` for `ObjectId`.

## Health Check

Every service exposes two endpoints outside the `/api/v1` namespace:

- `GET /healthz` — liveness probe, returns 200 if the process is alive.
- `GET /readyz` — readiness probe, returns 200 if external dependencies (e.g., MongoDB) respond to ping, 503 otherwise.

Handlers live in `infrastructure/driving/http-axum/src/server/health.rs`. The readiness checker is injected from `main.rs` as a `HealthChecker` closure.

## X-Request-Id Middleware

All HTTP responses include an `X-Request-Id` header. The middleware in `infrastructure/driving/http-axum/src/server/middleware.rs`:

- Propagates the incoming `X-Request-Id` header if present.
- Generates a UUID v7 if absent.
- Records the value in the tracing span for log correlation.

## Code Style Files

Every workspace includes `rustfmt.toml` and `clippy.toml` at the repository root. These enforce:

- Consistent formatting across all contributors.
- Linting rules that allow `unwrap`, `expect`, and `dbg!` exclusively within test code.

# Context

- **Language**: Rust (stable, LTS preference)
- **Framework**: Axum (HTTP), Tokio (async runtime)
- **Databases**: MongoDB (primary), Redis (caching/helpers)
- **Workspace**: Cargo workspace with crates: `domain`, `application`, `infra-mongo`, `infra-redis`, `infra-http-axum`, `shared`, `service`
- **Observability**: OpenTelemetry + `tracing` with structured JSON logging via `as_json!` macro
- **Validation**: `validator` crate for DTO syntactic validation
- **Serialization**: `serde`, `bson`
- **Code quality**: `rustfmt`, `clippy`, `cargo-sort`
