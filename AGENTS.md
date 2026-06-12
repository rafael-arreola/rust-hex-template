# Target Outcome

Produce a Rust monolithic codebase following a hexagonal/clean architecture with Axum HTTP, MongoDB, and Redis. Every entity follows identical structural patterns so the codebase is predictable across all contributors. The agent may flag missing context, anticipate edge cases, and propose shortcuts—but respects the existing architecture and avoids unwarranted rewrites.

# Success Criteria

- All entities follow the same directory structure, naming conventions, and template patterns below.
- Layer dependencies are strictly enforced: `driving/http_axum → application → domain ← driven/mongo/redis`.
- All external errors are mapped to `DomainError` variants with stable, machine-readable codes.
- Health check endpoints (`/healthz`, `/readyz`) and `X-Request-Id` middleware are present in every service.
- Cargo dependencies are sorted alphabetically; unused dependencies are removed.
- `rustfmt.toml` and `clippy.toml` enforce consistent formatting and linting across the codebase.

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

## Cargo Monolithic Directory Structure (mandatory)

```
Cargo.toml                                       → Root package configuration
src/main.rs                                      → Composition Root & DI wiring (Main binary)
src/domain.rs                                    → Domain module router (former domain/src/lib.rs)
src/domain/entities.rs                           → Entity module router
src/domain/entities/{entity}.rs                  → Entity struct + typed ID + marker
src/domain/port.rs                               → Port module router
src/domain/port/{entity}.rs                      → trait {Entity}RepositoryPort
src/domain/services/mod.rs                       → Domain services module router
src/domain/services/{service}.rs                 → Pure business logic (no I/O, no deps)
src/domain/error.rs                              → DomainError enum + DomainResult<T>
src/domain/values.rs                             → DomainId<T>
src/domain/pagination.rs                         → Pagination struct
src/domain/macros.rs                             → Macros module router
src/domain/macros/json.rs                        → JSON serialization macros (as_json!)

src/application.rs                               → Application module router (former application/src/lib.rs)
src/application/{entity}.rs                      → {Entity}Service (use case orchestration)
src/application/shared/mod.rs                    → Reusable sub-flows WITH I/O

src/shared.rs                                    → Shared capabilities module router (former shared/src/lib.rs)
src/shared/config.rs                             → Environment configuration (loaded once)
src/shared/http_client.rs                        → Instrumented reqwest client (traceparent propagation)
src/shared/tracer.rs                             → OpenTelemetry & tracing setup
src/shared/tracer/format.rs                      → GCP Cloud Logging JSON event formatter

src/infrastructure.rs                            → General infrastructure module router
src/infrastructure/driven.rs                     → Driven adapters module router
src/infrastructure/driven/mongo.rs               → MongoDB adaptor module router (former mongo/src/lib.rs)
src/infrastructure/driven/mongo/provider.rs      → MongoDB connection provider
src/infrastructure/driven/mongo/{entity}.rs      → {Entity} module router
src/infrastructure/driven/mongo/{entity}/model.rs   → {Entity}Model (BSON/serde)
src/infrastructure/driven/mongo/{entity}/repository.rs → {Entity}Repository
src/infrastructure/driven/redis.rs               → Redis adaptor (former redis/src/lib.rs)

src/infrastructure/driving.rs                    → Driving adapters module router
src/infrastructure/driving/http_axum.rs          → Axum HTTP adaptor module router (former http-axum/src/lib.rs)
src/infrastructure/driving/http_axum/routes.rs              → Router registration
src/infrastructure/driving/http_axum/routes/{entity}.rs     → Axum handlers
src/infrastructure/driving/http_axum/server.rs              → Server module router
src/infrastructure/driving/http_axum/server/error.rs        → ApiError definition
src/infrastructure/driving/http_axum/server/health.rs       → Health check endpoints (/healthz, /readyz)
src/infrastructure/driving/http_axum/server/middleware.rs   → Cross-cutting HTTP middleware (e.g. X-Request-Id)
src/infrastructure/driving/http_axum/server/response.rs     → GenericApiResponse
src/infrastructure/driving/http_axum/server/state.rs        → AppState (Services container)
src/infrastructure/driving/http_axum/server/validation.rs   → Validation utilities

rustfmt.toml                                     → Rustfmt configuration (workspace-wide, mandatory)
clippy.toml                                      → Clippy configuration (workspace-wide, mandatory)
```

Module routers use the modern Rust convention: when a directory `foo/` contains submodules, the parent module is `foo.rs` at the same level as the directory — never `foo/mod.rs`. Every new file is exported with `pub mod` in its parent router or `lib.rs`.

## Layer Dependencies (Enforced by Module Boundaries)

```
driving/http_axum ──> application ──> domain <── driven/mongo, driven/redis
```

| Module / Layer      | May import                                                   | Forbidden to import                             |
| ------------------- | ------------------------------------------------------------ | ----------------------------------------------- |
| `domain`            | Nothing outside itself (zero local modules)                  | Everything else                                 |
| `application`       | Only `domain`, `shared`                                      | `infra_mongo`, `infra_redis`, `infra_http_axum` |
| `infra_mongo/redis` | Only `domain`, `shared`                                      | `application`, `infra_http_axum`                |
| `infra_http_axum`   | `domain`, `application`, framework deps, observability types | `infra_mongo`, `infra_redis`, config deps       |
| `shared`            | Only external crates                                         | `domain`, `application`, `infra_*`              |

## What may cross module boundaries

✅ Primitives (`String`, `i32`, `bool`, `f64`, etc.), `DateTime<Utc>`, domain entities, domain enums, domain typed IDs.

❌ DTOs (`*Input`, `*Output`) outside `infra_http_axum` &nbsp;|&nbsp; ❌ Models (`*Model`) outside `infra_mongo` &nbsp;|&nbsp; ❌ DB driver types (`bson::ObjectId`) outside `infra_mongo`.

## Three essential templates

### Port (`src/domain/port/{entity}.rs`)

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

### Service (`src/application/{entity}.rs`)

```rust
use crate::domain::port::user::UserRepositoryPort;
use crate::domain::entities::user::User;
use crate::domain::error::DomainResult;
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

### Domain Service (`src/domain/services/{service}.rs`)

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

### Repository (`src/infrastructure/driven/mongo/{entity}/repository.rs`)

```rust
use crate::infrastructure::driven::mongo::user::model::UserModel;
use async_trait::async_trait;
use crate::domain::entities::user::{User, UserId};
use crate::domain::error::DomainResult;
use crate::domain::port::user::UserRepositoryPort;

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

1. Validate input via `ValidatedBody` (backed by `validator` crate; deserializes JSON or MessagePack according to `Content-Type`).
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
2. Code in dependency order: `domain` → `application` → `infra_mongo`/`infra_redis` → `infra_http_axum` → `main.rs`.
3. Trade-offs only if complexity demands it.

## Logging & Structured Telemetry

To log complete domain objects, entities, or DTOs in telemetry or tracing events, use the `as_json!` macro exported by the `domain` module instead of `?` (Debug) format or manual serialization.

Inject the field using the `%` prefix to indicate a formatted string.

```rust
use crate::domain::as_json;

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

## Dependency Sorting

- All dependency blocks (`[dependencies]`, etc.) are sorted alphabetically and grouped by nature.
- Run `cargo sort -g` to check and apply changes across the crate before committing dependency changes.

## Cargo.toml Hygiene

The root crate declares only dependencies it actually imports in its source code. The definitive test is `cargo check` — if it compiles without a dependency, that dependency does not belong.

## Provider Fail-Fast

All infrastructure providers (MongoDB, Redis, etc.) instantiated in `src/main.rs` follow the same fail-fast pattern:

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

All `{Entity}Model` structs in `src/infrastructure/driven/mongo/{entity}/model.rs` implement `From<Entity> for Model` and `From<Model> for Entity`. Do not use `TryFrom` — it introduces an inconsistent pattern across entities. Invalid IDs are handled silently via `.unwrap_or_default()` for `ObjectId`.

## Health Check

Every service exposes two endpoints outside the `/api/v1` namespace:

- `GET /healthz` — liveness probe, returns 200 if the process is alive.
- `GET /readyz` — readiness probe, returns 200 if external dependencies (e.g., MongoDB) respond to ping, 503 otherwise.

Handlers live in `src/infrastructure/driving/http_axum/server/health.rs`. The readiness checker is injected from `main.rs` as a `HealthChecker` closure.

## MessagePack Negotiation (bidirectional)

Content negotiation is split by direction:

- **Input (always on)**: `ValidatedBody<T>` deserializes the request body straight into the DTO — MessagePack when `Content-Type: application/vnd.msgpack`, JSON otherwise — then runs `validator` rules. No intermediate `Value` tree.
- **Output (on by default, disable via `ENABLE_MSGPACK=false`)**: `GenericApiResponse::into_response` stores a type-erased `Arc` of itself in the response extensions (`NegotiablePayload`). When the client sends `Accept: application/vnd.msgpack`, the `msgpack_negotiation` middleware encodes that original value **once** with `rmp_serde::to_vec_named` (named maps, mirroring the JSON shape) and swaps the body. Without the header — or when the flag is off — the JSON body passes through untouched at zero cost.

Rules: handlers never branch on format; responses must keep producing a valid JSON body by default (the middleware swap is an optimization, never a requirement); use `to_vec_named`, not `to_vec` — positional arrays break clients that mirror the JSON contract.

## Trace Context & X-Request-Id Middleware

The `trace_context` middleware in `src/infrastructure/driving/http_axum/server/middleware.rs` owns the per-request span. There is no `TraceLayer` — adding one creates disconnected root traces. The middleware:

- Extracts the remote trace context from the W3C `traceparent` header (via the global OpenTelemetry propagator), falling back to GCP's legacy `X-Cloud-Trace-Context`. If present, the request span joins that trace via `span.set_parent(...)`; otherwise a new `trace_id` is generated.
- Attaches the span's OTel context as the task-local current context (`.with_context(...)`) so natively instrumented clients (e.g. the MongoDB driver) parent their spans to the request's trace. Removing this silently orphans driver spans.
- Propagates the incoming `X-Request-Id` header if present, generates a UUID v7 if absent, echoes it on the response, and records it as a **declared field** on the request span. Never use `Span::current().record(...)` with undeclared fields — tracing drops them silently.

## Distributed Tracing & Telemetry Propagation

- `shared/tracer.rs` registers both the global text-map propagator (`TraceContextPropagator`) and the global tracer provider (`global::set_tracer_provider`). Instrumented libraries resolve the tracer from the global provider — without it their spans go nowhere.
- **OTel version alignment is mandatory**: `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-semantic-conventions`, `opentelemetry-gcloud-trace`, `tracing-opentelemetry`, and the `reqwest-tracing` feature flag must all target the same OpenTelemetry minor — pinned by whatever the `mongodb` driver requires (currently 0.31). Mixing minors compiles fine but splits spans into disconnected traces.
- MongoDB: the driver's `opentelemetry` feature is enabled, plus `bson`'s `serde_json-1` feature (the driver's otel code requires it with `bson-3`). The provider activates it via `OpentelemetryOptions::builder().enabled(true)`.
- Outbound HTTP: always use `shared::http_client::instrumented_client()` (reqwest + `reqwest-tracing`), injected from `main.rs` into driven adapters. Never construct a bare `reqwest::Client` — it does not propagate `traceparent`.
- GCP structured logs are emitted by the custom `CloudLoggingFormat` in `shared/tracer/format.rs`. Do not reintroduce `tracing-stackdriver`: it pins `tracing-opentelemetry 0.23` internally, so it cannot read the OTel context of modern spans and silently drops the `logging.googleapis.com/trace` correlation field.
- When the GCP exporter is unavailable (local dev), `init_tracing` falls back to plain `fmt` logs plus an exporterless in-process tracer, so every request still carries a valid `trace_id`.
- `init_tracing` returns a `TracerGuard`; `main.rs` keeps it and calls `guard.shutdown()` after the server exits to flush batched spans. The service body lives in `serve()` so every exit path — including provider fail-fast returns — goes through the flush. Never add an exit path that bypasses it.

## Graceful Shutdown

`SIGTERM`/`Ctrl+C` starts draining **immediately** — the shutdown future resolves on the signal, and `drain_timeout` acts as a hard cap on in-flight connections (enforced with a `oneshot` + `tokio::select!`). Never sleep before resolving the shutdown future: that delays the drain and leaves in-flight requests unbounded.

## Code Style Files

The project includes `rustfmt.toml` and `clippy.toml` at the repository root, plus a `[lints.clippy]` section in `Cargo.toml` that **denies** `unwrap_used`, `expect_used`, and `dbg_macro`. These enforce:

- Consistent formatting across all contributors.
- The "no `unwrap()`/`expect()`" error rule at build time — not as convention. `clippy.toml` re-allows them exclusively within test code.
- Do not remove the `[lints.clippy]` section: without it, `clippy.toml`'s test allowances configure lints that are never enabled.

# Context

- **Language**: Rust (stable, LTS preference)
- **Framework**: Axum (HTTP), Tokio (async runtime)
- **Databases**: MongoDB (primary), Redis (caching/helpers)
- **Structure**: Monolithic Cargo project with modules: `domain`, `application`, `infra_mongo`, `infra_redis`, `infra_http_axum`, `shared` and `main.rs`
- **Observability**: OpenTelemetry + `tracing` with structured JSON logging via `as_json!` macro
- **Validation**: `validator` crate for DTO syntactic validation
- **Serialization**: `serde`, `bson`
- **Code quality**: `rustfmt`, `clippy`, `cargo-sort` (for single crate)
