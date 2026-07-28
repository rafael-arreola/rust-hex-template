# AGENTS.md — Constitution of this template

This file is the **source of truth for invariants**: what must always be true of
the code, and why. It is loaded into every agent session, so it stays dense.

Step-by-step *procedures* do not live here — they live in `.agents/skills/`.
This document tells you the rules; the skills tell you the moves.

## How to work in this repo

1. **Route the task** through the table below. If a skill covers it, follow the
   skill — it already encodes the file list and the registration points people
   forget.
2. **Write code in dependency order**: `domain` → `application` →
   `driven/mongo` → `driving/http_axum` → `main.rs`. This order compiles on the
   first try; any other order does not.
3. **Close with the quality gate** (§7). Work is not done until it is green.
4. **Do not re-derive these rules per task.** If a rule is missing here, that is
   a gap in this file — say so and propose the addition, rather than inventing a
   local convention.

### Task router

| You are asked to…                                       | Follow                                          | Read first                                                   |
| ------------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------ |
| Add an entity / aggregate / CRUD resource               | `.agents/skills/add-entity`                     | §3 templates, §4.7 wiring                                     |
| Add an endpoint / use case to an existing entity        | `.agents/skills/add-endpoint`                   | §3.3 service, §3.6 handler                                    |
| Add pure business logic (no I/O)                        | `.agents/skills/add-domain-service`             | §3.4                                                          |
| Integrate an external HTTP service or Redis cache       | `.agents/skills/add-driven-adapter`             | §4.6 timeouts, §4.1 errors                                    |
| Write tests                                             | `.agents/skills/test-entity`                    | §6                                                            |
| Verify the code respects the invariants                 | `.agents/skills/architecture-audit`             | §2                                                            |
| Verify before committing                                | `.agents/skills/quality-gate`                   | §7                                                            |
| Something not covered above                             | This file, then ask                             | —                                                             |

> **Skill discovery.** Claude Code loads skills from `.claude/skills/`. That path
> is a symlink to `.agents/skills` and is **not tracked by git**, so a fresh
> clone has no auto-discovery until you recreate it:
> `ln -s ../.agents/skills .claude/skills`. Without it the skills still work as
> plain documentation — read the `SKILL.md` directly.

### When to stop and ask

Ask before: changing a `DomainError` variant or its `code()` (it is a public API
contract), changing the response envelope, adding a dependency that duplicates
one already present, or relaxing anything in §2. Everything else: decide, state
the decision in one line, and proceed.

---

# 1. Orientation

## 1.1 Demo code vs. template

The repo ships three example entities — `DemoUser`, `DemoProduct`, `DemoOrder` —
plus `DemoPricingService`. The `Demo` prefix means exactly one thing: **this is
disposable reference code, delete it when starting a real service.** Removal
steps are at the top of `README.md`.

Everything without the prefix is the template proper and stays: `DomainError`,
`DomainId`, `Pagination`, `GenericApiResponse`, `ValidatedBody`, the
middlewares, the tracer, config and the health endpoints.

Naming examples in this document (`User`, `UserRepositoryPort`,
`CreateUserInput`…) describe the convention for a **real** entity and therefore
carry no prefix. Do not prefix your own entities with `Demo`.

Two prefixes, two meanings — do not mix them:

- `Demo*` → disposable reference code. Delete it.
- `Fake*` → in-memory test double over a port (`FakeDemoUserRepository`). A
  testing device that stays.

## 1.2 Stack — authoritative versions

Single Cargo package named `service`, **Rust edition 2024** (let-chains,
`if let … && let …`, are in use — do not "fix" them into nested `if`s).

| Area          | Crates (pinned in `Cargo.toml`)                                                                            |
| ------------- | ---------------------------------------------------------------------------------------------------------- |
| HTTP / async  | `axum 0.8`, `tokio 1` (full), `tower-http 0.7` (cors, compression-gzip, decompression-gzip)                  |
| Databases     | `mongodb 3.8` (bson-3, rustls-tls, dns-resolver, opentelemetry), `bson 3`, `redis 1` (aio, tokio-comp)      |
| Observability | `opentelemetry` / `opentelemetry_sdk` / `opentelemetry-semantic-conventions` **0.32**, `tracing-opentelemetry 0.33`, `opentelemetry-gcloud-trace 0.24`, `tracing 0.1`, `tracing-subscriber 0.3` |
| Outbound HTTP | `reqwest 0.13` (rustls, http2, json), `reqwest-middleware 0.5`, `reqwest-tracing 0.7` (`opentelemetry_0_32`) |
| Serialization | `serde 1`, `serde_json 1`, `rmp-serde 1.3` (MessagePack), `erased-serde 0.4` (deferred encoding)             |
| Validation    | `validator 0.20` (derive)                                                                                    |
| Errors / misc | `thiserror 2`, `anyhow 1` (**tracer bootstrap only**), `uuid 1` (v7), `chrono 0.4`, `dotenvy`, `futures`, `rustls 0.23` |
| Dev only      | `tower 0.5` (util) — drives the router from `#[cfg(test)]` via `ServiceExt::oneshot`                         |

`anyhow` is confined to `shared/tracer.rs`, where there is no domain yet.
Everywhere else the error type is `DomainError` (§4.1).

Release profile is tuned (`lto = "thin"`, `strip = true`, `opt-level = 3`) and
the dev profile compiles dependencies at `opt-level = 3` for a usable debug
build. Do not change these to "speed up CI" without measuring.

> ### ⚠ Known drift — OpenTelemetry is currently split
>
> `mongodb 3.8` depends on **`opentelemetry 0.31`**, while the application and
> every other telemetry crate are on **0.32**. Both are in
> `Cargo.lock`. The two versions are distinct crates with distinct global
> registries, so the tracer provider registered by `shared/tracer.rs` (0.32)
> does **not** reach the driver's 0.31 global — MongoDB command spans do not
> join the request trace today.
>
> Detect it:
>
> ```bash
> cargo tree -i opentelemetry@0.31.0   # expected: nothing
> ```
>
> **Bumping the driver does not fix it** — `3.4` → `3.8` was tried and `3.8`
> still requires 0.31. The two real options are to pin the app's telemetry
> stack down to 0.31 (`opentelemetry`, `opentelemetry_sdk`,
> `opentelemetry-semantic-conventions`, a `tracing-opentelemetry` release
> matching 0.31, and the `reqwest-tracing` feature `opentelemetry_0_31` —
> check `opentelemetry-gcloud-trace` has a compatible release first), or wait
> for a driver on 0.32. Never paper over it with a second provider. See §4.5
> for why alignment is mandatory.

## 1.3 Repository map

Module routers use the modern Rust convention: when a directory `foo/` has
submodules, the parent module is `foo.rs` **next to** the directory, not
`foo/mod.rs`. Every new file is registered with `pub mod` in its parent router;
the top-level routers are declared in `main.rs` (this crate has a `main.rs`, no
`lib.rs`).

The two surviving `mod.rs` files — `src/domain/services/mod.rs` and
`src/application/shared/mod.rs` — are grandfathered. Do not add new ones.

```
AGENTS.md                                        → this file (CLAUDE.md and GEMINI.md are symlinks to it)
README.md                                        → human onboarding + demo-removal steps
.agents/README.md                                → skill index and maintenance rules
.agents/skills/<name>/SKILL.md                   → operating procedures (see task router)
.github/workflows/ci.yml                         → the quality gate, enforced (§7)
build/Dockerfile, build/cloudbuild.yaml          → container image & GCP build
.env.example                                     → the full environment contract (§5)
rustfmt.toml, clippy.toml                        → style, enforced at build time (§7)
Cargo.toml                                       → single package + [lints.clippy] denies

src/main.rs                                      → module routers + Composition Root (`main` orchestrates, `serve` holds the body)

src/domain.rs                                    → domain router
src/domain/entities.rs                           → entity router
src/domain/entities/{entity}.rs                  → entity struct + marker + typed ID
src/domain/port.rs                               → port router
src/domain/port/{entity}.rs                      → trait {Entity}RepositoryPort
src/domain/services/mod.rs                       → domain-service router
src/domain/services/{service}.rs                 → pure business logic (no I/O, no deps)
src/domain/error.rs                              → DomainError + ErrorSeverity + DomainResult<T>
src/domain/values.rs                             → DomainId<T, V> + DomainIdValue
src/domain/pagination.rs                         → Pagination
src/domain/macros.rs                             → macro router
src/domain/macros/json.rs                        → as_json! (exported at crate root — see §4.5)

src/application.rs                               → application router
src/application/{entity}.rs                      → {Entity}Service (use-case orchestration)
src/application/shared/mod.rs                    → reusable sub-flows WITH I/O

src/shared.rs                                    → shared-capabilities router
src/shared/config.rs                             → Env, loaded once into a OnceLock
src/shared/http_client.rs                        → instrumented reqwest client + timeout budgets
src/shared/tracer.rs                             → OpenTelemetry + tracing setup, TracerGuard
src/shared/tracer/format.rs                      → GCP Cloud Logging JSON event formatter

src/infrastructure.rs                            → infrastructure router
src/infrastructure/driven.rs                     → driven-adapter router
src/infrastructure/driven/mongo.rs               → Mongo router
src/infrastructure/driven/mongo/provider.rs      → MongoProvider (connect, ping, otel opt-in)
src/infrastructure/driven/mongo/{entity}.rs      → per-entity router
src/infrastructure/driven/mongo/{entity}/model.rs      → {Entity}Model (BSON/serde)
src/infrastructure/driven/mongo/{entity}/repository.rs → {Entity}Repository + create_indexes
src/infrastructure/driven/redis.rs               → RedisProvider (wired but disabled — §5)

src/infrastructure/driving.rs                    → driving-adapter router
src/infrastructure/driving/http_axum.rs          → HTTP adaptor router (re-exports ServerLauncher, AppState)
src/infrastructure/driving/http_axum/routes.rs                → app_router(): nests every entity router
src/infrastructure/driving/http_axum/routes/{entity}.rs       → router() + Axum handlers + query structs
src/infrastructure/driving/http_axum/routes/{entity}/dtos.rs  → *Input / *Output DTOs
src/infrastructure/driving/http_axum/server.rs                → ServerLauncher, layer stack, msgpack middleware
src/infrastructure/driving/http_axum/server/error.rs          → ApiError + the single error-log choke point
src/infrastructure/driving/http_axum/server/health.rs         → /healthz, /readyz, drain flag
src/infrastructure/driving/http_axum/server/middleware.rs     → trace_context, request_timeout
src/infrastructure/driving/http_axum/server/response.rs       → GenericApiResponse, GenericPagination, NegotiablePayload
src/infrastructure/driving/http_axum/server/state.rs          → AppState + FromRef wiring
src/infrastructure/driving/http_axum/server/validation.rs     → ValidatedBody extractor
```

There is no `tests/` directory: tests live in `#[cfg(test)] mod tests` next to
the code they cover (§6).

---

# 2. Invariants

Non-negotiable. They exist so the codebase stays predictable across entities and
contributors. `.agents/skills/architecture-audit` turns most of them into
executable greps.

## 2.1 Layer dependencies

```
driving/http_axum ──> application ──> domain <── driven/mongo, driven/redis
```

| Module              | May import                                                   | Must never import                               |
| ------------------- | ------------------------------------------------------------ | ----------------------------------------------- |
| `domain`            | Nothing outside itself (plus `serde`/`chrono`/`thiserror`)   | Every other local module                        |
| `application`       | `domain`, `shared`                                           | `infrastructure::*` (driven **and** driving)    |
| `infrastructure::driven` | `domain`, `shared`                                      | `application`, `infrastructure::driving`        |
| `infrastructure::driving` | `domain`, `application`, framework/observability deps | `infrastructure::driven`, `shared::config`      |
| `shared`            | External crates only                                         | `domain`, `application`, `infrastructure`       |

The driving layer imports the **concrete** `{Entity}Service`, not a trait — the
inversion happens at the port, between application and driven.

Config is read once in `main.rs` and passed down as values; adapters never call
`config::get()` themselves.

## 2.2 What may cross a module boundary

✅ Primitives (`String`, `i32`, `bool`, `f64`, …), `chrono::DateTime<Utc>`,
domain entities, domain enums, `DomainId` typed IDs, `Pagination`,
`DomainError`.

❌ DTOs (`*Input`, `*Output`) outside `driving/http_axum`
❌ Models (`*Model`) outside `driven/mongo`
❌ Driver types (`bson::ObjectId`, `mongodb::*`, `redis::*`) outside their adapter
❌ Framework types (`axum::*`, `StatusCode`) outside `driving/http_axum`

## 2.3 Naming

| Scope                  | Rule                                  | Example ✅                          | Avoid ❌                    |
| ---------------------- | ------------------------------------- | ----------------------------------- | --------------------------- |
| Files & folders        | singular                              | `user.rs`, `product/`               | `users.rs`, `products/`     |
| Structs                | PascalCase, singular                  | `User`, `Order`                     | `Users`, `Orders`           |
| Port traits            | `{Entity}RepositoryPort`              | `UserRepositoryPort`                | `UserRepository` (as trait) |
| Infrastructure structs | `{Entity}Repository` — no tech prefix | `UserRepository` in `driven/mongo`  | `MongoUserRepository`       |
| DB collections         | plural, snake_case                    | `users`, `order_items`              | `user`, `orderItems`        |
| BSON document fields   | snake_case, always                    | `total_price`, `created_at`         | `totalPrice`, `createdAt`   |
| API routes             | plural                                | `/api/v1/users`, `/api/v1/orders`   | `/api/v1/user`              |
| DTOs                   | `*Input` / `*Output` suffix           | `CreateUserInput`, `UserOutput`     | `UserDto`, `UserRequest`    |
| Mongo indexes          | explicit `name`, `*_idx` suffix       | `email_unique_idx`                  | driver-generated names      |
| Variables & fields     | full words, no abbreviations          | `user_email`, `page_number`         | `usr`, `idx`, `tmp`         |

## 2.4 Anti-patterns — reject on sight

| Anti-pattern                                              | Why it is banned                                                  | Do instead                                        |
| --------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------- |
| `unwrap()` / `expect()` / `dbg!` in production code       | Denied by `[lints.clippy]` — the build fails                       | `?`, `ok_or_else(DomainError::…)`, `map_err`      |
| Raw driver error crossing an adapter boundary             | Leaks infrastructure into the domain, and connection strings into logs | `.map_err(\|e\| DomainError::database(e.to_string()))` |
| `delete_one` / `$unset` on an entity collection           | Soft-delete is the contract (§4.3)                                 | `$set { deleted_at: now }`                        |
| A query without `"deleted_at": { "$exists": false }`      | Returns tombstoned documents                                       | Add the filter to *every* read                    |
| Read-then-check before a mutating write                   | Two concurrent callers both pass the check (§4.4)                  | One conditional atomic update                     |
| Logging an error *and* returning it                       | Double-logs; the boundary logs once (§4.1)                         | Construct and propagate with `?`                  |
| `tower_http::TimeoutLayer`                                | Answers 408 with an empty body, breaking the envelope              | `middleware::request_timeout` (§4.6)              |
| `TraceLayer`                                              | Creates a second, disconnected root span                           | `middleware::trace_context` (§4.5)                |
| Bare `reqwest::Client`                                    | Does not propagate `traceparent`, has no timeout                   | `shared::http_client::instrumented_client()`      |
| `Span::current().record("x", …)` on an undeclared field   | `tracing` drops it silently                                        | Declare the field in the span macro               |
| `#[tracing::instrument]` with no `fields(...)`            | No correlation key in the log line                                 | Always declare at least one (`%id`, `%email`)     |
| Assembling response JSON by hand                          | Diverges from the envelope                                         | `GenericApiResponse::{success,paginated,error}`   |
| `TryFrom` for entity↔model conversion                     | Inconsistent with every other entity                               | `From` both ways (§3.5)                           |
| A new `foo/mod.rs`                                        | The router convention is `foo.rs`                                  | `foo.rs` next to `foo/`                           |
| Axum path `:id`                                           | Axum 0.8 syntax is `{id}`; `:id` panics at startup                 | `.route("/{id}", …)`                              |
| A `create_indexes()` call in `main.rs`                    | A wiring step the compiler cannot enforce; forget it and queries silently do collection scans | `Repository::new(&db).await` owns it (§3.7) |
| `repo.clone() as Arc<dyn UserRepositoryPort>`             | Rust coerces `Arc<Concrete>` → `Arc<dyn Trait>` on its own         | Pass `repo.clone()` directly (§3.10)              |

---

# 3. Canonical templates

Presented in dependency order — the order you should write them in.

## 3.1 Entity — `src/domain/entities/{entity}.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::values;

pub struct UserMarker;
pub type UserId = values::DomainId<UserMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<UserId>,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

impl User {
    /// Predicates over the entity's own state are welcome here.
    /// Anything that needs I/O is an application service, not this.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
```

- Derives are `Debug, Serialize, Deserialize, Clone` — `Serialize` is what makes
  the entity loggable through `as_json!` (§4.5).
- `id` is `Option` — `None` until the repository assigns it on `create`.
- `created_at` / `updated_at` / `deleted_at` are mandatory on every entity.
- The marker struct needs **no derives**; see §3.2.

## 3.2 Typed IDs — `DomainId<T, V = String>`

Defined in `src/domain/values.rs`. A `UserId` cannot be mistaken for a
`ProductId` at compile time, which is the whole point.

```rust
pub struct UserMarker;
pub type UserId       = DomainId<UserMarker>;        // String-backed (default)
pub type LegacyUserId = DomainId<UserMarker, i64>;   // any V: DomainIdValue
```

| Need                             | Use                                          |
| -------------------------------- | -------------------------------------------- |
| Build from a known value         | `UserId::new("usr_abc")`                     |
| Build from an untrusted string   | `UserId::parse(&s)? ` → `Result<_, String>`  |
| Read the inner value             | `id.inner()` → `&V`                          |
| Consume it (DTO conversion)      | `id.into_inner()` → `V`                      |
| Pass to a repository (String IDs)| `&**id` → `&str` (via `Deref`)               |

`DomainIdValue` is implemented for `String`, `i64`, `u64`, `i32`, `u32`.
Implement it for your own type if you need a different inner value.

`Clone`, `Debug`, `PartialEq`, `Eq` and `Hash` are hand-written on purpose:
`#[derive]` would place the bound on the marker `T`, forcing every marker struct
to derive traits it never uses — and the breakage only surfaces the first time
someone compares two IDs, usually inside a test. Do not "simplify" them into
derives.

## 3.3 Port — `src/domain/port/{entity}.rs`

```rust
use crate::domain::entities::user::{User, UserId};
use crate::domain::error::DomainResult;
use crate::domain::pagination::Pagination;
use async_trait::async_trait;

#[async_trait]
pub trait UserRepositoryPort: Send + Sync {
    async fn create(&self, user: &User) -> DomainResult<UserId>;
    async fn find_by_id(&self, id: &UserId) -> DomainResult<Option<User>>;
    async fn find_by_email(&self, email: &str) -> DomainResult<Option<User>>;
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<User>>;
    async fn update(&self, id: &UserId, user: &User) -> DomainResult<bool>;
    async fn delete(&self, id: &UserId) -> DomainResult<bool>;
    async fn count(&self) -> DomainResult<u64>;
}
```

- Ports exist **only for aggregate roots**. Not every entity needs a repository.
- Signatures use domain types and primitives only — never DTOs, never BSON.
- `#[async_trait]` + `Send + Sync` on every port: Axum and Tokio move request
  work across worker threads, so anything shared through `AppState` must be
  thread-safe.
- `count()` is not optional — every paginated list handler needs it.
- Return `DomainResult<bool>` from `update`/`delete` to mean "did it match?".
  Mapping `false` to `NotFound` is the *service's* decision, not the adapter's.
- A conditional write returns `DomainResult<bool>` too (`try_reserve_stock`) —
  see §4.4.

## 3.4 Domain service — `src/domain/services/{service}.rs`

```rust
use crate::domain::entities::order::Order;

/// Pure business logic — zero I/O, zero constructor dependencies.
pub struct PricingService;

impl PricingService {
    pub fn new() -> Self { Self }

    /// Business rule: orders over 1000 get a 10% discount.
    pub fn apply_discount(&self, order: &Order) -> f64 {
        if order.total_price > 1000.0 { order.total_price * 0.90 } else { order.total_price }
    }
}

impl Default for PricingService {
    fn default() -> Self { Self::new() }
}
```

- Stateless, no constructor parameters, no ports.
- Operates exclusively on domain entities and primitives.
- Called from application services — never from infrastructure.
- Provide `Default` alongside `new()` (clippy asks for it).

## 3.5 Application service — `src/application/{entity}.rs`

```rust
use crate::domain::entities::user::{User, UserId};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::user::UserRepositoryPort;
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
        if self.repo.find_by_email(email).await?.is_some() {
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
}
```

- Constructor injection via `Arc<dyn Port>` (dynamic dispatch). A service that
  spans aggregates takes one `Arc<dyn …Port>` per aggregate.
- `#[derive(Clone)]` so it can be shared through `AppState`.
- Every public method carries `#[tracing::instrument(skip_all, fields(...))]`
  with **at least one field** — that field is the correlation key in the logs.
- Parameters are primitives, typed IDs or domain values. Never DTOs.
- This is where **semantic** validation lives (uniqueness, existence, business
  rules) — see §4.2.
- Timestamps are set here with `chrono::Utc::now()`; IDs stay `None` until the
  repository returns one.

## 3.6 Mongo model — `src/infrastructure/driven/mongo/{entity}/model.rs`

```rust
use crate::domain::entities::user::{User, UserId};
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UserModel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub email: String,
    pub created_at: bson::DateTime,
    pub updated_at: bson::DateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<bson::DateTime>,
}

impl From<User> for UserModel {
    fn from(entity: User) -> Self {
        Self {
            id: entity.id.as_ref().and_then(|id| ObjectId::parse_str(&**id).ok()),
            name: entity.name,
            email: entity.email,
            created_at: bson::DateTime::from_chrono(entity.created_at),
            updated_at: bson::DateTime::from_chrono(entity.updated_at),
            deleted_at: entity.deleted_at.map(bson::DateTime::from_chrono),
        }
    }
}

impl From<UserModel> for User {
    fn from(model: UserModel) -> Self {
        Self {
            id: model.id.map(|oid| UserId::new(oid.to_hex())),
            name: model.name,
            email: model.email,
            created_at: model.created_at.to_chrono(),
            updated_at: model.updated_at.to_chrono(),
            deleted_at: model.deleted_at.map(|dt| dt.to_chrono()),
        }
    }
}
```

- **`From` in both directions, never `TryFrom`.** An unparseable ID becomes
  `None` via `.ok()`; a validation error at this layer would be a lie, since the
  document already exists.
- **MongoDB is snake_case, always.** `#[serde(rename_all = "snake_case")]` makes
  the contract explicit. The only field-level `rename` allowed is `_id`. Every
  `doc! { … }` — queries and index keys alike — uses snake_case names.

## 3.7 Mongo repository — `.../{entity}/repository.rs`

```rust
use crate::domain::entities::user::{User, UserId};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::user::UserRepositoryPort;
use crate::infrastructure::driven::mongo::user::model::UserModel;
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
};

#[derive(Clone)]
pub struct UserRepository {
    collection: Collection<UserModel>,
}

impl UserRepository {
    /// Building the repository ensures its indexes exist.
    ///
    /// `new` is **async and fallible on purpose**: index creation happens here,
    /// not in `main.rs`. That turns "the indexes are there" from a wiring step
    /// someone can forget — one `cargo check` will never catch — into a
    /// property of the type. Holding a `UserRepository` means its indexes were
    /// created, or the service never started.
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<UserModel>("users") };
        repo.create_indexes().await?;
        Ok(repo)
    }

    /// Idempotent — safe to run on every startup.
    /// **Private**: `new` is the only caller, by design. A `pub` version
    /// reopens the door to calling it (or forgetting to) from the outside.
    async fn create_indexes(&self) -> DomainResult<()> {
        let indexes = vec![
            IndexModel::builder()
                .keys(doc! { "email": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .name("email_unique_idx".to_string())
                        .build(),
                )
                .build(),
            // Every read filters on `deleted_at`, so it leads every compound index.
            IndexModel::builder()
                .keys(doc! { "deleted_at": 1, "created_at": -1 })
                .options(
                    IndexOptions::builder()
                        .name("deleted_created_compound_idx".to_string())
                        .build(),
                )
                .build(),
        ];

        self.collection
            .create_indexes(indexes)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        tracing::info!("✓ User indexes created");
        Ok(())
    }
}

#[async_trait]
impl UserRepositoryPort for UserRepository {
    #[tracing::instrument(skip_all)]
    async fn create(&self, user: &User) -> DomainResult<UserId> {
        let model = UserModel::from(user.clone());
        let result = self
            .collection
            .insert_one(model)
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        result
            .inserted_id
            .as_object_id()
            .map(|oid| UserId::new(oid.to_hex()))
            .ok_or_else(|| DomainError::internal("Failed to get inserted ID"))
    }

    #[tracing::instrument(skip_all)]
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<User>> {
        let cursor = self
            .collection
            .find(doc! { "deleted_at": { "$exists": false } })
            .skip(pagination.get_skip())
            .limit(pagination.get_limit())
            .sort(doc! { "created_at": -1 })
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        let models: Vec<UserModel> =
            cursor.try_collect().await.map_err(|e| DomainError::database(e.to_string()))?;

        Ok(models.into_iter().map(User::from).collect())
    }

    #[tracing::instrument(skip_all)]
    async fn update(&self, id: &UserId, user: &User) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "User", &**id))?;

        let model = UserModel::from(user.clone());
        // bson 3 API (`serialize_to_document`, not the old `bson::to_document`).
        // This `$set`s the whole serialized model, so any field the model
        // declares is overwritten — partial updates need a hand-built `doc!`.
        let bson_doc = mongodb::bson::serialize_to_document(&model)
            .map_err(|e| DomainError::internal(e.to_string()))?;

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! { "$set": bson_doc },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, id: &UserId) -> DomainResult<bool> {
        let oid = ObjectId::parse_str(&**id)
            .map_err(|_| DomainError::invalid_param("id", "User", &**id))?;

        let now = mongodb::bson::DateTime::from_chrono(chrono::Utc::now());

        let result = self
            .collection
            .update_one(
                doc! { "_id": oid, "deleted_at": { "$exists": false } },
                doc! { "$set": { "deleted_at": now } },
            )
            .await
            .map_err(|e| DomainError::database(e.to_string()))?;

        Ok(result.matched_count > 0)
    }

    #[tracing::instrument(skip_all)]
    async fn count(&self) -> DomainResult<u64> {
        self.collection
            .count_documents(doc! { "deleted_at": { "$exists": false } })
            .await
            .map_err(|e| DomainError::database(e.to_string()))
    }
}
```

- Every method is `#[tracing::instrument(skip_all)]`.
- Every `.await` on the driver is followed by `.map_err(…)`. No raw driver error
  ever leaves this file.
- Every filter carries `"deleted_at": { "$exists": false }`.
- Invalid `ObjectId` → `DomainError::invalid_param`, not a database error: the
  client sent it.
- Index names are explicit. A query that filters or sorts on a new field needs a
  new index in the same PR.
- Compound indexes lead with `deleted_at`, because every read filters on it
  first.
- For ephemeral collections, add a TTL index instead of a cleanup job — Mongo
  expires the documents for you:

  ```rust
  IndexModel::builder()
      .keys(doc! { "updated_at": 1 })
      .options(
          IndexOptions::builder()
              .expire_after(std::time::Duration::from_secs(7 * 24 * 3600))
              .name("updated_at_ttl_idx".to_string())
              .build(),
      )
      .build()
  ```

## 3.8 DTOs — `.../routes/{entity}/dtos.rs`

```rust
use crate::domain::entities::user::{User, UserId};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateUserInput {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

#[derive(Serialize)]
pub struct UserOutput {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<User> for UserOutput {
    fn from(user: User) -> Self {
        Self {
            id: user.id.map(|id: UserId| id.into_inner()).unwrap_or_default(),
            name: user.name,
            email: user.email,
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        }
    }
}
```

- `*Input`: `#[derive(Deserialize, Validate)]`. Syntactic rules only (§4.2).
- `*Output`: `#[derive(Serialize)]` + `From<Entity>` so handlers use `.into()`.
- Timestamps are serialized as RFC 3339 strings.
- `deleted_at` is never exposed in an `*Output`.

## 3.9 Handlers and routing — `.../routes/{entity}.rs`

```rust
pub mod dtos;

use crate::application::user::UserService;
use crate::domain::entities::user::{User, UserId};
use crate::domain::pagination::Pagination;
use crate::infrastructure::driving::http_axum::server::{
    error::ApiError,
    response::{GenericApiResponse, GenericPagination},
    state::AppState,
    validation::ValidatedBody,
};
use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::Arc;

use self::dtos::{CreateUserInput, UserOutput};

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Axum 0.8 path syntax is `{id}`. `:id` panics at router construction.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_user).get(list_users))
        .route("/{id}", get(get_user).delete(delete_user))
}

#[tracing::instrument(skip_all)]
pub async fn create_user(
    State(service): State<Arc<UserService>>,
    ValidatedBody(req): ValidatedBody<CreateUserInput>,
) -> Result<GenericApiResponse<UserOutput>, ApiError> {
    let user: User = service.create_user(&req.name, &req.email).await?;
    Ok(GenericApiResponse::success(user.into()))
}

#[tracing::instrument(skip_all)]
pub async fn list_users(
    State(service): State<Arc<UserService>>,
    Query(query): Query<UserQuery>,
) -> Result<GenericApiResponse<GenericPagination<UserOutput>>, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let pagination = Pagination { page, limit };

    let users: Vec<User> = service.list_users(pagination).await?;
    let total = service.count_users().await?;
    let data: Vec<UserOutput> = users.into_iter().map(Into::into).collect();

    Ok(GenericApiResponse::paginated(data, total, page, limit))
}
```

A handler does exactly five things, in this order:

1. Deserialize + validate the body via `ValidatedBody<T>`.
2. Convert path/query strings into typed IDs and `Pagination`.
3. Call the service with primitives / domain values.
4. Convert the domain result into an `*Output` DTO with `.into()`.
5. Wrap it in `GenericApiResponse`.

Zero business logic. Zero branching on content type. The `?` operator carries
`DomainError` into `ApiError` through the `From` impl in §4.1.

> **Caveat that is easy to get wrong.** `Query<T>` does **not** run
> `validator` — `#[derive(Validate)]` on a query struct is decorative and its
> rules never execute. Bound pagination in the handler with `clamp`, as above.
> Only `ValidatedBody<T>` validates. The demo entities still use the unbounded
> `unwrap_or` form; new code should clamp.

Register the entity router in `routes.rs`:

```rust
pub fn app_router() -> Router<AppState> {
    Router::new()
        .nest("/users", user::router())
        .nest("/products", product::router())
}
```

## 3.10 Wiring — `AppState` and `main.rs`

`src/.../server/state.rs`:

```rust
#[derive(Clone)]
pub struct AppState {
    pub health_checker: HealthChecker,
    pub user_service: Arc<UserService>,
    pub product_service: Arc<ProductService>,
}

macro_rules! impl_from_ref {
    ($state:ty, $field:ident, $service:ty) => {
        impl FromRef<$state> for Arc<$service> {
            fn from_ref(state: &$state) -> Self { state.$field.clone() }
        }
    };
}

// HealthChecker is not an Arc<Service>, so it gets a hand-written impl.
impl FromRef<AppState> for HealthChecker {
    fn from_ref(state: &AppState) -> Self { state.health_checker.clone() }
}

impl_from_ref!(AppState, user_service, UserService);
impl_from_ref!(AppState, product_service, ProductService);
```

`src/main.rs` is the Composition Root. `main` only orchestrates; the body lives
in `serve()` so **every** exit path — including fail-fast returns — reaches the
tracer flush:

```rust
#[tokio::main]
async fn main() {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        eprintln!("Failed to install rustls crypto provider: {:?}", e);
        return;
    }

    let env = config::get();
    let tracer_guard = match tracer::init_tracing().await { /* … */ };

    serve(env).await;

    if let Some(guard) = tracer_guard {
        guard.shutdown();
    }
}
```

Providers and repositories are **fail-fast** — the service does not start
degraded. Every one of them follows the same shape:

```rust
let mongo = match MongoProvider::new(&env.service_name, &env.mongo_url, &env.mongo_db).await {
    Ok(mongo) => mongo,
    Err(e) => {
        tracing::error!("Failed to connect to MongoDB: {}", e);
        return;
    }
};

// Repositories: `new` is async and fallible because it also ensures the
// indexes. There is no separate `create_indexes()` step here to forget.
let user_repo = match UserRepository::new(&db).await {
    Ok(repo) => Arc::new(repo),
    Err(e) => {
        tracing::error!("Failed to initialize UserRepository: {}", e);
        return;
    }
};
```

**No explicit trait casts.** Rust coerces `Arc<Concrete>` into `Arc<dyn Trait>`
by itself at the call site, so passing the repository to a service needs no
turbofish and no `as`:

```rust
// ✅
let user_service = Arc::new(UserService::new(user_repo.clone()));

// ❌ noise the compiler does not need — and it drags the port trait into
//    main.rs's imports for no reason
let user_service = Arc::new(UserService::new(user_repo.clone() as Arc<dyn UserRepositoryPort>));
```

Then `AppState` is assembled and the server launched:

```rust
ServerLauncher::new(state)
    .with_cors_origins(env.cors_origins.clone())
    .with_http(env.port)
    .with_drain_timeout(env.drain_timeout_secs)
    .with_request_timeout(env.request_timeout_secs)
    .with_msgpack(env.msgpack_enabled)
    .run()
    .await;
```

| Builder method                 | Effect                                                        |
| ------------------------------ | ------------------------------------------------------------- |
| `new(state)`                   | Takes `AppState` with every service injected                  |
| `with_http(port)`              | **Omit it and no server starts** — `run()` returns immediately |
| `with_cors_origins(origins)`   | Comma-separated list, or `"*"` for permissive                 |
| `with_drain_timeout(secs)`     | Hard cap on in-flight connections during shutdown (§4.7)      |
| `with_request_timeout(secs)`   | Per-request budget → 504 with the standard envelope (§4.6)    |
| `with_msgpack(enabled)`        | Response-side `Accept` negotiation, on by default (§4.8)      |
| `run()`                        | Binds and blocks until the shutdown signal                    |

### The 7 registration points for a new entity

Forgetting one of these is the most common failure mode. **`cargo check` catches
every one of them** — that is deliberate. The one step it used to miss (calling
`create_indexes()` in `main.rs`) no longer exists: the constructor owns it.

1. `pub mod {entity};` in `src/domain/entities.rs`
2. `pub mod {entity};` in `src/domain/port.rs`
3. `pub mod {entity};` in `src/application.rs`
4. `pub mod {entity};` in `src/infrastructure/driven/mongo.rs`
5. `pub mod {entity};` in `src/infrastructure/driving/http_axum/routes.rs` **and**
   `.nest("/{entities}", {entity}::router())` in `app_router()`
6. Field in `AppState` + `impl_from_ref!` in `server/state.rs`
7. In `main.rs`: `Repository::new(&db).await` (fail-fast), build the service,
   add it to `AppState`

If you ever find yourself adding a step that the compiler cannot enforce, that
is a signal the design is wrong — push the guarantee into a type, the way
`Repository::new` does with indexes.

---

# 4. Cross-cutting contracts

## 4.1 Errors

Rules:

- Every domain/application/adapter function returns `DomainResult<T>`.
- No `unwrap()` / `expect()` — denied at build time (§7).
- Every external error is mapped with `.map_err(…)` at the adapter boundary.
- Build `DomainError` through its constructors, not by hand, except where a
  variant needs a custom `reason` (`DomainError::Invalid { field, reason }`).
- **Every error is logged exactly once, at the driving boundary.** Services and
  repositories construct and propagate with `?`; they never log-and-return the
  same error. A new driving adapter (pubsub, gRPC) implements its own single
  choke point reusing `severity()` and `public_message()`.

Each error has two views, both declared in `src/domain/error.rs`:

- `Display` / `to_string()` — the **internal** message, full detail (raw driver
  text, connection strings). Logs only. It never crosses a driving boundary.
- `public_message()` — the **client-safe** message. Client-caused variants reuse
  their `Display` text; variants carrying infrastructure detail return a generic
  message pointing at the `trace_id`.
- `severity()` — the `ErrorSeverity` (`Info` | `Warn` | `Error`) the boundary
  must log with.

| Variant            | `code()`                        | HTTP  | `severity()` | Public message              |
| ------------------ | ------------------------------- | ----- | ------------ | --------------------------- |
| `NotFound`         | `NOT_FOUND`                     | 404   | Info         | same as `Display`           |
| `AlreadyExists`    | `ALREADY_EXISTS`                | 409   | Info         | same as `Display`           |
| `Invalid`          | `INVALID_INPUT`                 | 400   | Info         | same as `Display`           |
| `Required`         | `REQUIRED_FIELD`                | 400   | Info         | same as `Display`           |
| `Unauthorized`     | `UNAUTHORIZED`                  | 401   | Warn         | same as `Display`           |
| `Forbidden`        | `FORBIDDEN`                     | 403   | Warn         | same as `Display`           |
| `BusinessRule`     | `BUSINESS_RULE_VIOLATION`       | 422   | Warn         | same as `Display`           |
| `Timeout`          | `TIMEOUT`                       | 504   | Error        | generic, "please retry"     |
| `ExternalService`  | `EXTERNAL_SERVICE_UNAVAILABLE`  | 500   | Error        | generic, names the service  |
| `Database`         | `INTERNAL_ERROR`                | 500   | Error        | generic, points at trace_id |
| `Internal`         | `INTERNAL_ERROR`                | 500   | Error        | generic, points at trace_id |

Constructors: `not_found(entity, id)`, `duplicate(entity, field, value)`,
`invalid_param(param, entity, value)`, `business_rule(msg)`, `timeout(op)`,
`external_service(service, msg)`, `database(msg)`, `internal(msg)`.

`code()` is a **public API contract** — clients branch on `cause` + HTTP status,
never on `message`. Changing a code is a breaking change; adding a variant is
not. Tests assert on `code()`, never on message text.

`ApiError` (`server/error.rs`) is a struct — not an enum — with `code`, `message`
and `status`. Its `From<DomainError>` impl is the single place that decides the
status mapping, emits the single severity-driven log with full internal detail,
and takes `message` from `public_message()` (never `to_string()`).

See `src/domain/error.rs` for the canonical implementation.

## 4.2 Validation boundaries

- **Syntactic — HTTP layer, `*Input` DTOs, `validator`.** Shape and format:
  string length, email format, numeric ranges. Runs inside `ValidatedBody<T>`;
  failures become `400 INVALID_INPUT` without ever reaching the service.
- **Semantic — application services, against ports.** Uniqueness, existence,
  stock availability, transactional limits. Anything requiring a query lives
  here and returns a `DomainError`.

A rule that needs no I/O and belongs to the business (not the transport) goes in
a domain service (§3.4).

## 4.3 Soft delete

Mandatory for every entity. `deleted_at: Option<DateTime<Utc>>` on the entity,
`Option<bson::DateTime>` on the model. `delete` does
`$set { deleted_at: now }`; **no hard deletes.** Every read filters
`"deleted_at": { "$exists": false }`, including `count_documents`. Compound
indexes lead with `deleted_at` because every query starts there.

The fakes used in tests replicate this: a soft-deleted record must be invisible
to `find_by_id`, `find_all` and `count`.

## 4.4 Concurrency and atomic writes

Anything that guards a resource (stock, quota, seats, balance) must be a
**single conditional update**, not a read followed by a check followed by a
write. Two concurrent callers both pass a read-then-check, and both proceed.

The canonical pattern, from `demo_product/repository.rs`:

```rust
// The `$gte` guard is what makes this atomic: MongoDB matches the document and
// applies the `$inc` as one operation, so two concurrent reservations cannot
// both succeed against the same last unit. Dropping it turns this into a race.
let result = self
    .collection
    .update_one(
        doc! { "_id": oid, "deleted_at": { "$exists": false }, "stock": { "$gte": quantity } },
        doc! { "$inc": { "stock": -quantity }, "$set": { "updated_at": now } },
    )
    .await
    .map_err(|e| DomainError::database(e.to_string()))?;

Ok(result.matched_count > 0)   // false = the guard rejected it
```

Rules that follow:

- A conditional write returns `bool` from the port. The **service** turns
  `false` into the right `DomainError` — and may re-read to distinguish "gone"
  from "out of stock", so the client gets the correct code.
- A read-then-check *before* the atomic write is allowed only as a friendly
  fast-fail that shows the client the real number. It is never the guard. Say so
  in a comment, as the demo does.
- **After a successful reservation, every later failure must compensate.** The
  demo releases the stock inline and logs at `error` if the compensation itself
  fails, naming the record that needs manual reconciliation. Once the order
  moves to another service, that inline compensation is the seam where a saga or
  an outbox belongs.

## 4.5 Observability

**The request span.** `middleware::trace_context` owns it. There is no
`TraceLayer` — adding one creates a disconnected root trace. The middleware:

- Extracts the remote context from W3C `traceparent` via the global propagator,
  falling back to GCP's legacy `X-Cloud-Trace-Context`. Present → the span joins
  that trace with `set_parent`; absent → a fresh `trace_id`.
- Attaches the span's OTel context as the task-local current context
  (`.with_context(...)`), so natively instrumented clients parent their spans to
  the request. Removing this silently orphans them.
- Propagates `X-Request-Id`, or generates a UUID v7, echoes it on the response,
  and records it as a **declared** span field. `Span::current().record(...)` with
  an undeclared field is dropped silently by `tracing`.

**Structured object logging.** Use the `as_json!` macro instead of `?` (Debug)
or manual serialization. It is `#[macro_export]`ed, which places it at the
**crate root** — the import is `crate::as_json`, not `crate::domain::as_json`:

```rust
use crate::as_json;

tracing::info!(user = %as_json!(&user), "User created successfully");
```

The `%` prefix marks it as a formatted string. The macro degrades to a JSON
error object rather than panicking, so it is safe on any `Serialize` value.

**Tracer setup** (`shared/tracer.rs`) registers both the global text-map
propagator (`TraceContextPropagator`) and the global tracer provider
(`global::set_tracer_provider`). Instrumented libraries resolve the tracer from
the global provider — without it their spans go nowhere.

**OTel version alignment is mandatory.** `opentelemetry`, `opentelemetry_sdk`,
`opentelemetry-semantic-conventions`, `opentelemetry-gcloud-trace`,
`tracing-opentelemetry` and the `reqwest-tracing` feature flag must all target
the same OpenTelemetry minor — the one the `mongodb` driver requires. Mixing
minors compiles fine but splits spans into disconnected traces, because each
minor is a separate crate with its own global registry. **This invariant is
currently violated — see the drift note in §1.2.**

MongoDB: the driver's `opentelemetry` feature is on, plus `bson`'s
`serde_json-1` feature (required by the driver's otel code with `bson-3`).
`MongoProvider` activates it via `OpentelemetryOptions::builder().enabled(true)`.

**GCP logs** come from the custom `CloudLoggingFormat` in
`shared/tracer/format.rs`. Do not reintroduce `tracing-stackdriver`: it pins
`tracing-opentelemetry 0.23` internally, cannot read the OTel context of modern
spans, and silently drops the `logging.googleapis.com/trace` correlation field.

**Local fallback.** When the GCP exporter is unavailable, `init_tracing` falls
back to plain `fmt` logs plus an exporterless in-process tracer, so every
request still carries a valid `trace_id`. Startup does not fail because
telemetry is unavailable.

**Flush.** `init_tracing` returns a `TracerGuard`; `main.rs` holds it and calls
`guard.shutdown()` after the server exits to flush batched spans. Never add an
exit path that bypasses it — that is why the body lives in `serve()`.

## 4.6 Timeouts

Two independent budgets. Both exist because a dependency that accepts a
connection and then goes silent would otherwise pin a task forever.

**Inbound** — `middleware::request_timeout`, configured by
`REQUEST_TIMEOUT_SECS` (default 30). On expiry it returns
`DomainError::timeout(...)` → **504** with the standard envelope and
`cause: "TIMEOUT"`. It is deliberately **not** `tower_http::TimeoutLayer`, which
answers a bare 408 with an empty body and breaks the envelope. It is layered
*inside* `trace_context`, so a timed-out request is still recorded on its span
and still echoes its `X-Request-Id`. The internal detail (method, path, budget)
goes to the log; the client gets the generic message.

**Outbound** — `shared::http_client`. `reqwest` applies **no timeout by
default**, so every client this template hands out sets one:

| Constant / fn                      | Value / purpose                                       |
| ---------------------------------- | ------------------------------------------------------ |
| `DEFAULT_TIMEOUT`                  | 10 s — whole request (connect + send + response)        |
| `DEFAULT_CONNECT_TIMEOUT`          | 3 s — TCP/TLS handshake alone                           |
| `instrumented_client()`            | The default client. Use this.                           |
| `client_with_timeout(duration)`    | Explicit budget for a legitimately slower/stricter dep  |

Both return a `ClientWithMiddleware` carrying `TracingMiddleware`, so outbound
calls propagate `traceparent`. Build them in `main.rs` and inject them into
driven adapters — never construct a bare `reqwest::Client`.

Per-dependency timeouts should normally fire before the inbound budget; the
request timeout is the last-resort guard.

## 4.7 Health checks and graceful shutdown

Two endpoints, outside `/api/v1`:

- `GET /healthz` — liveness. Returns 200 whenever the process is alive,
  **including while draining**. A failing liveness probe tells the orchestrator
  to *kill* the process, which is the opposite of a graceful drain.
- `GET /readyz` — readiness. 200 when dependencies respond to ping, 503
  otherwise. The readiness checker is a `HealthChecker` closure injected from
  `main.rs`.

`SIGTERM`/`Ctrl+C` triggers, in order:

1. `health::start_draining()` flips a process-wide `AtomicBool`, so `/readyz`
   returns 503 **before** the listener closes and the load balancer takes the
   pod out of rotation instead of racing the shutdown.
2. The graceful-shutdown future resolves **immediately** — never sleep before
   resolving it; that delays the drain and leaves in-flight requests unbounded.
3. `drain_timeout` (a `oneshot` + `tokio::select!`) acts as a hard cap on
   in-flight connections; exceeding it logs a warning and aborts them.
4. `serve()` returns, `main` flushes the tracer.

## 4.8 HTTP response envelope

Every response — success or error — is built by `GenericApiResponse`
(`server/response.rs`). Handlers never assemble response JSON by hand.

```json
// Success
{ "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736", "data": { "id": "u1", "name": "Ada" } }

// Error — same envelope; `data` carries the detail, `cause` the stable code
{ "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736", "data": { "message": "User not found: u9" }, "cause": "NOT_FOUND" }

// Paginated
{
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "data": { "data": [{ "id": "u1", "name": "Ada" }], "total": 42, "page": 1, "limit": 20 }
}
```

- `trace_id` is always present — taken from the active OTel span, zeros when
  tracing is unavailable.
- `data` carries the payload. On errors it is an `ErrorDetail` **object**
  (`{ "message": … }`), not a bare string, so error payloads can gain fields
  without breaking clients.
- `cause` appears **only** on errors and is always a `DomainError::code()`.
- There is no top-level `error` field — that legacy shape is retired, and a test
  asserts it stays gone.

Constructors: `success(data)`, `paginated(data, total, page, limit)`,
`error(code, message, status)` (used by `ApiError::into_response`).

## 4.9 Content negotiation (MessagePack)

Split by direction:

- **Input — always on.** `ValidatedBody<T>` deserializes the body straight into
  the DTO: MessagePack when `Content-Type: application/vnd.msgpack`, JSON
  otherwise. Then it runs the `validator` rules. No intermediate `Value` tree.
- **Output — on by default, disable with `ENABLE_MSGPACK=false`.**
  `GenericApiResponse::into_response` stores a type-erased `Arc` of itself in
  the response extensions (`NegotiablePayload`). When the client sends
  `Accept: application/vnd.msgpack`, the `msgpack_negotiation` middleware
  encodes that original value **once** with `rmp_serde::to_vec_named` and swaps
  the body. Without the header — or with the flag off — the JSON body passes
  through at zero cost.

Rules: handlers never branch on format; responses must always be valid JSON by
default (the swap is an optimization, never a requirement); use `to_vec_named`,
never `to_vec` — positional arrays break clients that mirror the JSON contract.
Responses without the extension (health checks) pass through untouched.

## 4.10 Middleware stack

Declared in `ServerLauncher::run()`. Axum wraps each `.layer()` around the
previous one, so **the last registered is the outermost**. A request traverses:

```
CORS
  → DefaultBodyLimit (32 MiB)
    → RequestDecompressionLayer
      → CompressionLayer
        → trace_context            (span, traceparent, X-Request-Id)
          → request_timeout        (504 with the envelope)
            → msgpack_negotiation  (only if enabled)
              → handler
```

The two orderings that matter and must not be swapped: `request_timeout` inside
`trace_context` (so timeouts are traced and get their request id), and
`msgpack_negotiation` innermost (so it sees the response extension before any
other layer can rewrite the body).

---

# 5. Configuration

`shared/config.rs` loads `.env` once into a `OnceLock<Env>`. `config::get()`
returns `&'static Env`. **A missing required variable calls `process::exit(1)`
with a message on stderr** — the service refuses to start half-configured.
`.env.example` is the contract; keep it in sync when you add a variable.

| Variable               | Required | Default              | Purpose                                        |
| ---------------------- | -------- | -------------------- | ---------------------------------------------- |
| `SERVICE_NAME`         | **yes**  | —                    | Mongo app name, OTel `service.name`             |
| `MONGO_URL`            | **yes**  | —                    | Connection string                               |
| `MONGO_DB`             | **yes**  | —                    | Database name                                   |
| `PORT`                 | no       | `3000`               | HTTP port (must parse as `u16`, else exit)      |
| `APP_ENV` (or `ENV`)   | no       | `DEV`                | `LCL` / `SBX` / `PRD`; OTel `deployment.environment` |
| `PROJECT_ID`           | no       | empty                | GCP project for trace/log correlation           |
| `DEBUG_LEVEL`          | no       | `info`               | Base level of the `EnvFilter`                   |
| `CORS_ORIGINS`         | no       | `*`                  | Comma-separated list, or `*`                    |
| `DRAIN_TIMEOUT_SECS`   | no       | `10`                 | Hard cap on in-flight connections at shutdown   |
| `REQUEST_TIMEOUT_SECS` | no       | `30`                 | Per-request budget → 504                        |
| `ENABLE_MSGPACK`       | no       | `true`               | Response-side `Accept` negotiation              |

Booleans accept `1/true/yes` and `0/false/no` (any case); anything else falls
back to the default rather than failing.

Noisy dependencies are pinned to `warn` in the `EnvFilter` regardless of
`DEBUG_LEVEL`: `h2`, `hyper`, `tokio_util`, `tower_http`, `axum`.

**Redis is wired but disabled.** `RedisProvider` is complete (connect,
multiplexed connection, `PING` on startup, prefixed key helper), but its
construction in `main.rs` and its `redis_url` / `redis_prefix` fields in `Env`
are commented out, and there is no `REDIS_URL` in `.env.example`. Enabling it
means uncommenting all three and adding the variable — do not describe Redis as
active until then.

---

# 6. Testing

No `tests/` directory. Tests live in `#[cfg(test)] mod tests` next to the code
they cover, so they move and get deleted with it. Procedure:
`.agents/skills/test-entity`.

**Application services — fake the port.** The `Arc<dyn Port>` seam is what makes
this possible: an in-memory fake exercises the whole service with no Mongo, no
mocking crate, and no extra dependency.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeUserRepository {
        users: Mutex<Vec<User>>,
    }

    #[async_trait::async_trait]
    impl UserRepositoryPort for FakeUserRepository {
        async fn create(&self, user: &User) -> DomainResult<UserId> { /* assigns an ID */ }
        async fn find_by_id(&self, id: &UserId) -> DomainResult<Option<User>> { /* honours soft-delete */ }
        // …every method, with the real adapter's semantics
    }

    fn service() -> UserService {
        UserService::new(Arc::new(FakeUserRepository::default()))
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_email() {
        let service = service();
        service.create_user("Ada", "ada@example.com").await.unwrap();

        let error = service.create_user("Ada II", "ada@example.com").await.unwrap_err();
        assert_eq!(error.code(), "ALREADY_EXISTS");
    }
}
```

Rules:

- **The fake replicates the real adapter's semantics**, or the test proves
  nothing: `create` assigns an ID, every read honours soft-delete, and
  `update`/`delete` return `false` when nothing matched.
- **Assert on `error.code()`, never on the message.** The code is the stable
  contract; the message is free to change.
- Name tests after the behaviour, not the method
  (`deleted_user_is_invisible`, not `test_delete`).
- `unwrap()` / `expect()` are allowed in tests — `clippy.toml` re-allows them
  there, and only there.

**HTTP layer — drive the router.** `tower::ServiceExt::oneshot` (in
`[dev-dependencies]`) sends a real request through the real layer stack. Use it
for anything that lives in the middleware or the extractors — content
negotiation, envelope shape, timeouts:

```rust
let request = HttpRequest::post("/echo")
    .header(header::CONTENT_TYPE, "application/vnd.msgpack")
    .body(Body::from(payload))?;

let response = test_app().oneshot(request).await?;
assert_eq!(response.status(), StatusCode::OK);
```

**What is already covered** — do not duplicate it, extend it:
`domain/error.rs` (no infrastructure detail leaks into `public_message()`,
severity mapping), `http_axum/server.rs` (msgpack in/out, envelope on rejection,
request timeout), `server/middleware.rs` (`X-Cloud-Trace-Context` parsing),
`shared/tracer/format.rs` (Cloud Logging JSON shape),
`application/demo_user.rs` and `application/demo_order.rs` (the canonical
fake-port suites).

**What has no coverage yet** — good places to add: the Mongo repositories (need
a live Mongo or a container), `application/demo_product.rs`, `shared/config.rs`,
`shared/http_client.rs`.

---

# 7. Quality gate

Run before every commit, in this order — cheapest first, so a formatting slip
does not cost a compile. A red step stops the sequence: fix it, restart from
step 1. Procedure and per-step fixes: `.agents/skills/quality-gate`.

```bash
cargo fmt --all -- --check          # 1. rustfmt.toml rules (max_width=100, edition 2024)
cargo clippy --all-targets -- -D warnings   # 2. includes the unwrap/expect/dbg denies
cargo sort --grouped --check        # 3. Cargo.toml dependency order
cargo test --all-targets            # 4. tests
```

`.github/workflows/ci.yml` runs exactly this on every push to `main` and every
PR, plus a separate `cargo audit` job for RustSec advisories. The audit is a
separate job on purpose: a new advisory is a different kind of signal and must
not block the quality gate.

`RUSTFLAGS: -D warnings` is deliberately **not** set globally — it would also
apply to dependency compilation, and a third-party warning would redden CI. The
`-D warnings` that matters is passed explicitly to clippy.

**Style enforcement is structural, not conventional:**

- `rustfmt.toml` — `max_width = 100`, `tab_spaces = 4`, `edition = "2024"`,
  `use_small_heuristics = "Max"`.
- `clippy.toml` — re-allows `unwrap`/`expect`/`dbg!` **in tests only**.
- `[lints.clippy]` in `Cargo.toml` — **denies** `unwrap_used`, `expect_used`,
  `dbg_macro`. Do not remove this section: without it, `clippy.toml`'s test
  allowances configure lints that are never enabled, and the "no unwrap" rule
  silently becomes a suggestion.

Never edit a config file to make failing code pass. Never `#[allow]` in
production code without a one-line comment explaining the constraint.

**Cargo.toml hygiene.** The crate declares only dependencies it actually
imports. The definitive test is `cargo check` — if it compiles without the
dependency, the dependency does not belong. Test-only dependencies go in
`[dev-dependencies]`. Run `cargo sort -g` after any dependency change.

## Definition of done

- [ ] The quality gate is green, all four steps.
- [ ] Layer boundaries hold (§2.1) — `.agents/skills/architecture-audit` passes.
- [ ] Every new repository query filters on `deleted_at`, and every new
      filtered/sorted field has an index in the repository's private
      `create_indexes()` — which `new` calls, so `main.rs` stays clean.
- [ ] Every new public service method has `#[tracing::instrument]` with at least
      one field.
- [ ] Every new external error is mapped to a `DomainError`.
- [ ] A new entity registered at all 7 points (§3.10).
- [ ] New behaviour has a test asserting on `code()`, not on message text.
- [ ] A new environment variable is in `.env.example` **and** in the §5 table.
- [ ] An invariant that changed is updated **here first**, then propagated to the
      affected skills in the same PR — never the other way around.

---

# 8. How to respond

1. One line stating the architectural decision.
2. Code in dependency order: `domain` → `application` → `driven` → `driving` →
   `main.rs`.
3. Trade-offs only when the complexity warrants them.
4. Name what you did not verify. "Compiles" and "tested" are different claims;
   do not merge them.

Flag missing context, anticipate edge cases and propose shortcuts — but respect
the existing architecture and do not rewrite what was not asked for.
