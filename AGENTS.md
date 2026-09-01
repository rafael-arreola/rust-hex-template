# Memoria de Arquitectura — Rust Hexagonal Template

```
[ driving: HTTP (Axum) ] ──> [ application: use-cases ] ──> [ domain: core ] <── [ driven: MongoDB / Redis ]
                                   │
                                   └──> [ shared: config / tracer / http_client ]
```

---

## 1. Ontología y Taxonomía del Sistema

### 1.0 Fuentes de Verdad y Separación de Contextos

El sistema separa estrictamente la memoria técnica de la memoria de negocio:

- **`AGENTS.md`**: Fuente de verdad única para la arquitectura técnica, tipos universales, contratos de compilación, transporte, observabilidad, scaffolding y calidad.
- **`PROJECT.md`**: Fuente de verdad única para las reglas de negocio, modelos conceptuales del dominio, políticas comerciales, estados y restricciones funcionales.
- **Protocolo de Consulta de Negocio**: Antes de implementar cualquier caso de uso o lógica de dominio, se debe consultar `PROJECT.md`. Ante reglas no especificadas o ambiguas, se consulta al usuario y se actualiza `PROJECT.md`.

### 1.1 Stack Tecnológico, Descubrimiento Dinámico y Evolución

- **Rust**: Edición actual configurada en `Cargo.toml`.
- **Paquete Cargo**: Crate binario llamado `service` (`src/main.rs`).
- **Descubrimiento Dinámico de Dependencias (Cero Sesgo de Versión)**: La fuente de verdad única para las librerías activas, sus versiones y sus features es `Cargo.toml` y `Cargo.lock`. El agente debe inspeccionar los manifiestos y el código real antes de asumir firmas o capacidades de cualquier dependencia.
- **Evolución y Sugerencias Proactivas**: El agente no está atado a versiones o librerías fijas; tiene la capacidad y libertad de sugerir mejoras, actualizaciones de crates o adopción de nuevas librerías cuando aporten beneficios técnicos claros (rendimiento, seguridad, ergonomía, compatibilidad), evaluando siempre los trade-offs y verificando la compatibilidad en el workspace.
- **Capacidades Base del Stack**:
  - **HTTP / Runtime Asíncrono**: `axum`, `tokio` (full), `tower-http` (cors, compresión/descompresión gzip).
  - **Persistencia**: `mongodb` (BSON, TLS, DNS resolver, instrumentación), `redis` (operaciones asíncronas).
  - **Observabilidad**: `tracing`, `tracing-subscriber`, `opentelemetry`, `tracing-opentelemetry`, exportador de trazas GCP.
  - **HTTP Saliente**: `reqwest` (cliente instrumentado con middleware de tracing).
  - **Serialización y Validación**: `serde`, `serde_json`, `rmp-serde` (MessagePack), `validator`.
  - **Manejo de Errores y Utilidades**: `thiserror`, `anyhow` (restringido a bootstrap en `shared/tracer.rs`), `uuid`, `chrono`, `dotenvy`, `futures`, `rustls`.

### 1.2 Clasificación y Semántica de Símbolos

- **Prefijo `Demo*`**: Entidades y servicios de referencia descartables (`DemoUser`, `DemoProduct`, `DemoOrder`, `DemoPricingService`). Se eliminan al inicializar un servicio productivo.
- **Símbolos de Producción**: Nombres directos en singular sin prefijos tecnológicos ni de dominio (`User`, `Order`, `UserRepositoryPort`, `CreateUserInput`, `UserModel`, `UserRepository`).

### 1.3 Taxonomía de Archivos y Responsabilidades

Organización flexible de módulos según la estructura y cohesión del código:

- **Convención `foo.rs` junto a `foo/` (Rust 2018 / Módulos abiertos o de enrutamiento)**: Preferente para enrutadores de capas y raíces donde se evita la ambigüedad de múltiples archivos `mod.rs` en editores (ej. `src/domain.rs` + `src/domain/`, `src/domain/entities.rs` + `src/domain/entities/`).
- **Convención `foo/mod.rs` (Módulos autocontenidos o subpaquetes cohesivos)**: Válida y recomendada cuando el directorio conforma una unidad encapsulada con lógica interna, sub-flujos o servicios agrupados (ej. `src/domain/services/mod.rs`, `src/application/shared/mod.rs`).
- **Invariante de consistencia**: Mantener coherencia en el mismo nivel jerárquico o subárbol (evitar mezclar estilos arbitrariamente dentro de un mismo módulo).

```
AGENTS.md                                              Constitución y memoria de arquitectura técnica
PROJECT.md                                             Memoria de negocio, dominio, políticas y flujos funcionales
CLAUDE.md / GEMINI.md                                  Symlinks canónicos a AGENTS.md
Cargo.toml                                             Configuración de paquete, dependencias y [lints.clippy]
build/Dockerfile, build/cloudbuild.yaml                Empaquetado y despliegue en GCP
.env.example                                           Esquema contractual de variables de entorno

src/main.rs                                            Composition Root: inicialización fail-fast, tracer flush y servidor

src/domain.rs                                          Enrutador del núcleo de dominio
src/domain/entities.rs                                 Enrutador de entidades
src/domain/entities/{entity}.rs                        Struct de entidad + marker de tipo + alias DomainId
src/domain/port.rs                                     Enrutador de puertos de repositorio y servicios
src/domain/port/{entity}.rs                            Trait {Entity}RepositoryPort (Send + Sync + async_trait)
src/domain/services/mod.rs                             Enrutador de servicios de dominio
src/domain/services/{service}.rs                       Lógica de negocio pura (stateless, determinista, cero I/O)
src/domain/error.rs                                    DomainError + ErrorSeverity + DomainResult<T>
src/domain/values.rs                                   Typed ID: DomainId<T, V = String> + DomainIdValue
src/domain/pagination.rs                               Estructura Pagination (skip, limit, page)
src/domain/macros.rs                                   Enrutador de macros
src/domain/macros/json.rs                              Macro as_json! exportada en crate root (crate::as_json)

src/application.rs                                     Enrutador de servicios de aplicación
src/application/{entity}.rs                            {Entity}Service: orquestación de casos de uso y validación semántica
src/application/shared/mod.rs                          Sub-flujos reutilizables con I/O

src/shared.rs                                          Enrutador de capacidades transversales
src/shared/config.rs                                   Struct Env cargado en OnceLock (fail-fast en arranque)
src/shared/http_client.rs                              Cliente reqwest instrumentado con tracing y presupuestos de timeout
src/shared/tracer.rs                                   Inicialización de OpenTelemetry, GCP Cloud Trace y TracerGuard
src/shared/tracer/format.rs                            Formateador JSON estructurado para GCP Cloud Logging

src/infrastructure.rs                                  Enrutador de adaptadores
src/infrastructure/driven.rs                           Enrutador de adaptadores conducidos (driven)
src/infrastructure/driven/mongo.rs                     Enrutador MongoDB
src/infrastructure/driven/mongo/provider.rs            MongoProvider (pool de conexiones, health ping, otel)
src/infrastructure/driven/mongo/{entity}.rs            Enrutador del adaptador de entidad
src/infrastructure/driven/mongo/{entity}/model.rs      {Entity}Model: esquema BSON / Serde con From bidireccional
src/infrastructure/driven/mongo/{entity}/repository.rs {Entity}Repository: new() asíncrono con create_indexes() privado
src/infrastructure/driven/redis.rs                     RedisProvider (conexión multiplexada, paths de claves, ping)

src/infrastructure/driving.rs                          Enrutador de adaptadores conductores (driving)
src/infrastructure/driving/http_axum.rs                Enrutador HTTP Axum (ServerLauncher, AppState)
src/infrastructure/driving/http_axum/routes.rs         app_router(): anidamiento de routers de entidades
src/infrastructure/driving/http_axum/routes/{entity}.rs      router() + handlers Axum (5 pasos inmutables)
src/infrastructure/driving/http_axum/routes/{entity}/dtos.rs DTOs: *Input (Validate) y *Output (From<Entity>)
src/infrastructure/driving/http_axum/server.rs         ServerLauncher: ensamblado y ejecución de middlewares
src/infrastructure/driving/http_axum/server/error.rs   ApiError: choke-point único de logging estructurado
src/infrastructure/driving/http_axum/server/health.rs  Endpoints /healthz, /readyz y señal atómica de drenado
src/infrastructure/driving/http_axum/server/middleware.rs   trace_context (W3C/GCP) y request_timeout (504)
src/infrastructure/driving/http_axum/server/response.rs     GenericApiResponse, GenericPagination, NegotiablePayload
src/infrastructure/driving/http_axum/server/state.rs   AppState + macros de inyección impl_from_ref!
src/infrastructure/driving/http_axum/server/validation.rs   Extractor ValidatedBody (JSON y MessagePack)
```

---

## 2. Fronteras y Matrices de Visibilidad de Tipos

### 2.1 Matriz de Imports Permitidos y Prohibidos

Dirección vectorial estricta:
$$\text{driving/http\_axum} \longrightarrow \text{application} \longrightarrow \text{domain} \longleftarrow \text{infrastructure::driven}$$

| Capa                      | Imports Autorizados                                                        | Imports Denegados                               |
| :------------------------ | :------------------------------------------------------------------------- | :---------------------------------------------- |
| `domain`                  | `crate::domain`, crates std/externos base (`serde`, `chrono`, `thiserror`) | Todo módulo fuera de `domain`                   |
| `application`             | `domain`, `shared`                                                         | `infrastructure::*` (tanto driving como driven) |
| `infrastructure::driven`  | `domain`, `shared`                                                         | `application`, `infrastructure::driving`        |
| `infrastructure::driving` | `domain`, `application`, crates externos                                   | `infrastructure::driven`, `shared::config`      |
| `shared`                  | Crates externos exclusivamente                                             | `domain`, `application`, `infrastructure`       |

### 2.2 Fronteras de Tipos de Datos

- **Tipos Universales que cruzan capas**: Primitivos (`String`, `i32`, `bool`, `f64`), `chrono::DateTime<Utc>`, entidades y enums de dominio, IDs tipados `DomainId`, `Pagination`, `DomainError`.
- **Tipos Confinados (Prohibido cruzar fronteras)**:
  - DTOs (`*Input`, `*Output`): Confinados a `infrastructure/driving/http_axum`.
  - Modelos de base de datos (`*Model`): Confinados a `infrastructure/driven/mongo`.
  - Tipos de drivers (`bson::ObjectId`, `mongodb::*`, `redis::*`): Confinados a sus adaptadores driven.
  - Tipos del framework web (`axum::*`, `http::StatusCode`): Confinados a `infrastructure/driving/http_axum`.

### 2.3 Reglas de Nomenclatura del Código

- **Archivos y Directorios**: Singular en snake_case (`user.rs`, `order_item/`, `invoice.rs`).
- **Estructuras de Entidad**: PascalCase en singular (`User`, `Product`, `Invoice`).
- **Traits de Puerto**: `{Entity}RepositoryPort` (`UserRepositoryPort`).
- **Repositorios Concretos**: `{Entity}Repository` sin prefijos de tecnología (`UserRepository`, no `MongoUserRepository`).
- **Colecciones de Base de Datos**: Plural en snake_case (`users`, `order_items`, `invoices`).
- **Campos en BSON**: snake_case estricto (`created_at`, `total_amount`).
- **Rutas HTTP**: Plural (`/api/v1/users`, `/api/v1/invoices`).
- **DTOs**: Sufijos exclusivos `*Input` y `*Output` (`CreateInvoiceInput`, `InvoiceOutput`).
- **Índices MongoDB**: Nombre explícito con sufijo `*_idx` (`email_unique_idx`, `deleted_created_compound_idx`).
- **Variables y Campos**: Palabras completas sin abreviaturas (`user_email`, `page_number`, no `usr`, `idx`).

### 2.4 Invariantes y Antipatrones Denegados

1. **`unwrap()` / `expect()` / `dbg!`**: Prohibidos en código de producción (`[lints.clippy]` deniega la compilación).
2. **Fuga de errores crudos de infraestructura**: Mapeo obligatorio en adaptadores a `DomainError::database`, `DomainError::external_service` o `DomainError::internal`.
3. **Hard Delete (`delete_one`, `$unset`)**: Prohibido. Borrado lógico universal mediante `$set: { deleted_at: now }`.
4. **Lecturas sin filtro de soft-delete**: Toda consulta BSON (`find`, `find_one`, `count_documents`, `update_one`) debe incluir `"deleted_at": { "$exists": false }`.
5. **Lectura previa a escritura sin atomicidad**: La protección contra condiciones de carrera exige actualizaciones atómicas condicionales (`$gte`, `$inc`).
6. **Doble registro de errores (Log-and-Return)**: El error se propaga con `?` y se registra una sola vez en el choke-point `ApiError` (`server/error.rs`).
7. **`tower_http::TimeoutLayer`**: Prohibido (responde 408 sin envelope). Se usa `middleware::request_timeout`.
8. **`TraceLayer`**: Prohibido (crea spans raíz desconectados). Se usa `middleware::trace_context`.
9. **`reqwest::Client` sin instrumentar**: Prohibido instanciar clientes HTTP directos; se inyecta `shared::http_client::instrumented_client()`.
10. **`Span::current().record()` sobre campos no declarados**: Se descartan silenciosamente; todo campo debe figurar en los `fields(...)` del span.
11. **`#[tracing::instrument]` sin `fields(...)`**: Todo método público de servicio debe declarar campos de correlación (`%id`, `%email`).
12. **Construcción manual de JSON en respuestas**: Prohibida. Toda respuesta usa `GenericApiResponse::{success, paginated, error}`.
13. **`TryFrom` para entidad $\leftrightarrow$ modelo**: Prohibido. Se implementa `From` en ambas direcciones resolviendo IDs no válidos en silencio.
14. **Inconsistencia arbitraria en organización de módulos**: Se debe elegir la mejor opción según la estructura (`foo.rs` junto a `foo/` para enrutadores y módulos abiertos; `foo/mod.rs` para directorios/subpaquetes autocontenidos y cohesivos), manteniendo siempre uniformidad y consistencia dentro del mismo subárbol o nivel jerárquico.
15. **Sintaxis de rutas Axum con dos puntos (`:id`)**: Prohibida en Axum. Se utiliza `{id}`.
16. **Llamadas a `create_indexes()` en `main.rs`**: Prohibidas. El constructor `Repository::new(&db).await` encapsula la creación de índices.
17. **Casts explícitos de traits (`repo as Arc<dyn ...>`)**: Prohibidos. Rust realiza la coerción implícita de `Arc<Concrete>` a `Arc<dyn Trait>`.
18. **Creación no solicitada de Mocks, Fakes o Tests**: Prohibida terminantemente. No crear dobles de prueba (mocks, fakes, stubs) ni nuevos archivos de pruebas salvo que el usuario lo solicite de forma explícita en su instrucción.

---

## 3. Esquemas Canónicos de Tipos y Código

```
[ entities/{entity}.rs ]  <───  [ port/{entity}.rs ]
           │                            │
           ▼                            ▼
[ mongo/{entity}/model.rs ]   [ application/{entity}.rs ]
           │                            ▲
           ▼                            │
[ mongo/{entity}/repository.rs ]  [ routes/{entity}.rs ]  <───  [ dtos.rs ]
```

### 3.1 Entidad de Dominio — `src/domain/entities/{entity}.rs`

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
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
```

- **Derives requeridos**: `Debug, Serialize, Deserialize, Clone`.
- `id` es `Option` (`None` previo a inserción en base de datos).
- `created_at`, `updated_at` y `deleted_at` son campos obligatorios.

### 3.2 Identificadores Tipados — `DomainId<T, V = String>` (`src/domain/values.rs`)

- Creación desde valor confiable: `UserId::new("usr_123")`.
- Parseo desde entrada no confiable: `UserId::parse(&str_val)?`.
- Acceso por referencia: `id.inner()` $\rightarrow$ `&V`.
- Consumo a valor interno: `id.into_inner()` $\rightarrow$ `V`.
- Coerción a slice string: `&**id` $\rightarrow$ `&str` (vía trait `Deref`).

### 3.3 Puerto de Repositorio — `src/domain/port/{entity}.rs`

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

- Exclusivo para Aggregate Roots.
- Requiere `#[async_trait]` y bounds `Send + Sync`.
- `count()` es obligatorio para paginación.
- `update` y `delete` retornan `DomainResult<bool>`.

### 3.4 Domain Service — `src/domain/services/{service}.rs`

```rust
use crate::domain::entities::order::Order;

/// Lógica de negocio pura: determinista, cero I/O, sin llamadas asíncronas.
#[derive(Clone, Default)]
pub struct PricingService;

impl PricingService {
    pub fn new() -> Self {
        Self
    }

    pub fn apply_discount(&self, order: &Order) -> f64 {
        if order.total_price > 1000.0 { order.total_price * 0.90 } else { order.total_price }
    }
}
```

- Inmutable, stateless, sin dependencias en constructor.
- Instanciación directa en el Application Service (sin contenedor `Arc`).

### 3.5 Application Service — `src/application/{entity}.rs`

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

- Inyección mediante `Arc<dyn Port>`.
- Métodos públicos instrumentados con `#[tracing::instrument(skip_all, fields(...))]`.
- Validación semántica (existencia, unicidad, reglas de estado).

### 3.6 Modelo MongoDB — `src/infrastructure/driven/mongo/{entity}/model.rs`

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

### 3.7 Repositorio MongoDB — `src/infrastructure/driven/mongo/{entity}/repository.rs`

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
    pub async fn new(db: &Database) -> DomainResult<Self> {
        let repo = Self { collection: db.collection::<UserModel>("users") };
        repo.create_indexes().await?;
        Ok(repo)
    }

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

### 3.8 DTOs HTTP — `src/infrastructure/driving/http_axum/routes/{entity}/dtos.rs`

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

### 3.9 Handlers y Routing — `src/infrastructure/driving/http_axum/routes/{entity}.rs`

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
    // Secuencia canónica de 5 pasos inmutables:
    // 1. Deserialización y validación sintáctica (ValidatedBody)
    // 2. Mapeo a tipos de dominio
    // 3. Invocación del servicio de aplicación
    let user: User = service.create_user(&req.name, &req.email).await?;
    // 4. Mapeo a DTO Output (.into())
    // 5. Envoltorio en GenericApiResponse::success
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

### 3.10 Estado Compartido y Composition Root

**`src/infrastructure/driving/http_axum/server/state.rs`**:

```rust
#[derive(Clone)]
pub struct AppState {
    pub health_checker: HealthChecker,
    pub user_service: Arc<UserService>,
}

macro_rules! impl_from_ref {
    ($state:ty, $field:ident, $service:ty) => {
        impl FromRef<$state> for Arc<$service> {
            fn from_ref(state: &$state) -> Self { state.$field.clone() }
        }
    };
}

impl FromRef<AppState> for HealthChecker {
    fn from_ref(state: &AppState) -> Self { state.health_checker.clone() }
}

impl_from_ref!(AppState, user_service, UserService);
```

**`src/main.rs` (Composition Root)**:

```rust
#[tokio::main]
async fn main() {
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        eprintln!("Failed to install rustls crypto provider: {:?}", e);
        return;
    }

    let env = config::get();
    let tracer_guard = match tracer::init_tracing().await { /* ... */ };

    serve(env).await;

    if let Some(guard) = tracer_guard {
        guard.shutdown();
    }
}

async fn serve(env: &'static Env) {
    let mongo = match MongoProvider::new(&env.service_name, &env.mongo_url, &env.mongo_db).await {
        Ok(mongo) => mongo,
        Err(e) => {
            tracing::error!("Failed to connect to MongoDB: {}", e);
            return;
        }
    };
    let db = mongo.database();

    let user_repo = match UserRepository::new(&db).await {
        Ok(repo) => Arc::new(repo),
        Err(e) => {
            tracing::error!("Failed to initialize UserRepository: {}", e);
            return;
        }
    };

    let user_service = Arc::new(UserService::new(user_repo));

    let state = AppState {
        health_checker: HealthChecker::new(mongo.clone()),
        user_service,
    };

    ServerLauncher::new(state)
        .with_cors_origins(env.cors_origins.clone())
        .with_http(env.port)
        .with_drain_timeout(env.drain_timeout_secs)
        .with_request_timeout(env.request_timeout_secs)
        .with_msgpack(env.msgpack_enabled)
        .run()
        .await;
}
```

---

## 4. Matrices de Derivación y Scaffolding

### 4.1 Scaffolding de Nueva Entidad / Aggregate Root

Orden estricto de creación y compilación:

1. `src/domain/entities/{entity}.rs` + registro en `src/domain/entities.rs`.
2. `src/domain/port/{entity}.rs` + registro en `src/domain/port.rs`.
3. `src/application/{entity}.rs` + registro en `src/application.rs`.
4. `src/infrastructure/driven/mongo/{entity}.rs` (`pub mod model; pub mod repository;`) + registro en `src/infrastructure/driven/mongo.rs`.
5. `src/infrastructure/driven/mongo/{entity}/model.rs` + `src/infrastructure/driven/mongo/{entity}/repository.rs`.
6. `src/infrastructure/driving/http_axum/routes/{entity}/dtos.rs` + `src/infrastructure/driving/http_axum/routes/{entity}.rs`.
7. `src/infrastructure/driving/http_axum/routes.rs` (registro de módulo y `.nest("/entities", entity::router())`).
8. `src/infrastructure/driving/http_axum/server/state.rs` (campo en `AppState` + macro `impl_from_ref!`).
9. `src/main.rs` (`serve()`: instanciación fail-fast de repositorio, servicio e inyección en `AppState`).

**Los 7 Puntos de Registro Obligatorios**:

```
1. src/domain/entities.rs                           --> pub mod {entity};
2. src/domain/port.rs                               --> pub mod {entity};
3. src/application.rs                               --> pub mod {entity};
4. src/infrastructure/driven/mongo.rs               --> pub mod {entity};
5. src/infrastructure/driving/http_axum/routes.rs   --> pub mod {entity}; + .nest(...)
6. src/infrastructure/driving/http_axum/server/state.rs --> Campo en AppState + impl_from_ref!
7. src/main.rs                                      --> Repo::new(&db).await? + Service::new + AppState
```

### 4.2 Scaffolding de Endpoint o Caso de Uso

Árbol de decisión y secuencia de capas a impactar:

- **Acceso a datos nuevo**: Método en `Port` $\rightarrow$ Implementación en `Repository` (+ índice en `create_indexes()` si filtra/ordena por nuevo campo).
- **Lógica de caso de uso**: Método en `{Entity}Service` con `#[tracing::instrument]` y validación semántica.
- **Entrada / Salida de transporte**: DTO `*Input` (con `Validate`) y DTO `*Output` (con `From<Entity>`).
- **Handler**: Función de 5 pasos en `routes/{entity}.rs`.
- **Ruta**: Registro en `router()` de la entidad (o router padre para sub-recursos relacionales `/users/{id}/orders`).

### 4.3 Scaffolding de Domain Service

- Archivo: `src/domain/services/{service}.rs` + registro `pub mod {service};` en `src/domain/services/mod.rs` (o `src/domain/services.rs` según la convención del módulo).
- Estructura: Struct unitario `pub struct {Service};` con `new() -> Self` e `impl Default`.
- Inyección: Campo directo por valor en `{Entity}Service::new()` (sin inyección `Arc`).

### 4.4 Scaffolding de Driven Adapters

#### Servicio HTTP Externo

- **Puerto**: `src/domain/port/{capability}.rs` nombrando la capacidad funcional (`PaymentGatewayPort`), no la tecnología (`StripePort`).
- **Adaptador**: `src/infrastructure/driven/{adapter}.rs` recibiendo `reqwest_middleware::ClientWithMiddleware` y `base_url`. Errores mapeados a `DomainError::external_service`.
- **Wiring**: Inyección de `shared::http_client::instrumented_client()` desde `main.rs`.

#### Cache Redis

- Provider: `src/infrastructure/driven/redis.rs` (`RedisProvider`).
- Configuración: `redis_url` y `redis_prefix` en `shared/config.rs` y `.env.example`.
- Claves: Prefijadas con `provider.get_path(&["entity", &id])`.
- **Política de Degradación (Cache-Aside)**: Fallos en Redis emiten `tracing::warn!` y degradan a MongoDB primario; no abortan la petición.

---

## 5. Máquina de Estados y Contratos Transversales

### 5.1 Dominio de Errores y Registro Choke-Point

Todo método interno retorna `DomainResult<T>` (`Result<T, DomainError>`).

| Variante          | Código Estable (`code()`)      | HTTP | Severidad | Mensaje Público (`public_message()`)         |
| :---------------- | :----------------------------- | :--- | :-------- | :------------------------------------------- |
| `NotFound`        | `NOT_FOUND`                    | 404  | Info      | Mensaje original                             |
| `AlreadyExists`   | `ALREADY_EXISTS`               | 409  | Info      | Mensaje original                             |
| `Invalid`         | `INVALID_INPUT`                | 400  | Info      | Mensaje original                             |
| `Required`        | `REQUIRED_FIELD`               | 400  | Info      | Mensaje original                             |
| `Unauthorized`    | `UNAUTHORIZED`                 | 401  | Warn      | Mensaje original                             |
| `Forbidden`       | `FORBIDDEN`                    | 403  | Warn      | Mensaje original                             |
| `BusinessRule`    | `BUSINESS_RULE_VIOLATION`      | 422  | Warn      | Mensaje original                             |
| `Timeout`         | `TIMEOUT`                      | 504  | Error     | Mensaje genérico de reintento                |
| `ExternalService` | `EXTERNAL_SERVICE_UNAVAILABLE` | 500  | Error     | Mensaje genérico con nombre del servicio     |
| `Database`        | `INTERNAL_ERROR`               | 500  | Error     | Mensaje genérico con referencia a `trace_id` |
| `Internal`        | `INTERNAL_ERROR`               | 500  | Error     | Mensaje genérico con referencia a `trace_id` |

- **Doble Vista**: `Display` contiene el detalle técnico para logs internos; `public_message()` es el payload seguro expuesto al cliente.
- **Choke-Point Único**: `ApiError` (`server/error.rs`) es el único punto donde se genera el log estructurado de error con severidad.

### 5.2 Fronteras de Validación

- **Validación Sintáctica**: Capa HTTP en DTOs `*Input` mediante `validator` dentro del extractor `ValidatedBody<T>`.
- **Validación Semántica**: Servicios de aplicación evaluando reglas contra puertos (unicidad, existencia, stock).

### 5.3 Soft Delete Universal

Toda entidad y modelo contiene `deleted_at: Option<DateTime<Utc>>`.

- Escritura: `$set: { deleted_at: now }` (prohibido `delete_one`/`delete_many`).
- Lectura: Todo filtro incluye `"deleted_at": { "$exists": false }`.
- Índices compuestos: Inician siempre con `deleted_at: 1`.

### 5.4 Concurrencia y Escrituras Atómicas

Protección mediante actualización condicional atómica única:

```rust
let result = self.collection.update_one(
    doc! { "_id": oid, "deleted_at": { "$exists": false }, "stock": { "$gte": quantity } },
    doc! { "$inc": { "stock": -quantity }, "$set": { "updated_at": now } },
).await.map_err(|e| DomainError::database(e.to_string()))?;

Ok(result.matched_count > 0)
```

Si una transacción posterior falla, se ejecuta compensación inmediata revirtiendo la mutación.

### 5.5 Observabilidad y Tracing

- Spans manejados en `middleware::trace_context` extrayendo W3C `traceparent` o `X-Cloud-Trace-Context`.
- Registro estructurado de objetos mediante macro en crate root:
  ```rust
  use crate::as_json;
  tracing::info!(user = %as_json!(&user), "User created");
  ```
- Formato Cloud Logging configurado en `shared/tracer/format.rs`.

### 5.6 Timeouts y Presupuestos de Red

- **Inbound (Entrante)**: `middleware::request_timeout` configurado por `REQUEST_TIMEOUT_SECS` (default 30s) $\rightarrow$ 504 `TIMEOUT`.
- **Outbound (Saliente)**: `shared::http_client` aplica 10s de timeout global y 3s de timeout de conexión TCP/TLS.

### 5.7 Health Probes y Drenado Graceful

- `GET /healthz`: Liveness probe (retorna 200 siempre que el proceso responda).
- `GET /readyz`: Readiness probe (retorna 200 si dependencias responden a ping; 503 en drenado o fallo).
- `SIGTERM`: `health::start_draining()` activa flag atómico invalidando `/readyz` y procede al drenado acotado por `DRAIN_TIMEOUT_SECS`.

### 5.8 Formato de Respuesta y Negociación de Contenido

Estructura del envelope `GenericApiResponse`:

```json
// Éxito
{ "trace_id": "...", "data": { "id": "u1", "name": "Ada" } }

// Error
{ "trace_id": "...", "data": { "message": "User not found" }, "cause": "NOT_FOUND" }

// Paginado
{ "trace_id": "...", "data": { "data": [...], "total": 42, "page": 1, "limit": 20 } }
```

- MessagePack: Entrada transparente con `Content-Type: application/vnd.msgpack`; salida con `Accept: application/vnd.msgpack` serializada mediante `rmp_serde::to_vec_named`.

### 5.9 Pipeline de Middlewares en `ServerLauncher`

Orden de ejecución:
$$\text{CORS} \rightarrow \text{DefaultBodyLimit} \rightarrow \text{Decompression} \rightarrow \text{Compression} \rightarrow \text{trace\_context} \rightarrow \text{request\_timeout} \rightarrow \text{msgpack\_negotiation} \rightarrow \text{Handler}$$

### 5.10 Esquema de Variables de Entorno (`shared/config.rs`)

| Variable               | Obligatoria | Default | Propósito                                |
| :--------------------- | :---------- | :------ | :--------------------------------------- |
| `SERVICE_NAME`         | **Sí**      | —       | Nombre de servicio y OTel `service.name` |
| `MONGO_URL`            | **Sí**      | —       | Cadena de conexión MongoDB               |
| `MONGO_DB`             | **Sí**      | —       | Base de datos MongoDB                    |
| `PORT`                 | No          | `3000`  | Puerto TCP                               |
| `SERVICE_ENV`          | No          | `DEV`   | Entorno: `LCL` / `SBX` / `PRD`           |
| `PROJECT_ID`           | No          | `""`    | Proyecto GCP para correlación de trazas  |
| `DEBUG_LEVEL`          | No          | `info`  | Nivel base de logs                       |
| `CORS_ORIGINS`         | No          | `*`     | Orígenes CORS                            |
| `DRAIN_TIMEOUT_SECS`   | No          | `10`    | Límite máximo de drenado                 |
| `REQUEST_TIMEOUT_SECS` | No          | `30`    | Límite de tiempo por petición            |
| `ENABLE_MSGPACK`       | No          | `true`  | Habilitación de negociación MessagePack  |

---

## 6. Esquema de Pruebas y Validación

### 6.1 Política Estricta: Cero Mocks / Fakes sin Petición Explícita

- **Invariante Absoluta**: **El agente NO debe crear mocks, fakes, stubs, dobles de prueba ni nuevos archivos de test sin una petición explícita y directa del usuario.**
- **Preservación de Pruebas Existentes**: Los tests existentes en el repositorio deben seguir compilando y pasando (`cargo test`). No se deben alterar ni relajar para ocultar errores; corregir el código subyacente ante cualquier fallo.
- **Tests Bajo Demanda Expresa**: Únicamente cuando el usuario solicite explícitamente la creación de pruebas unitarias para un servicio, se implementarán en un módulo `#[cfg(test)] mod tests` en el mismo archivo, implementando el trait del puerto correspondiente para pruebas en memoria, con aserciones sobre `error.code()` y preservando la semántica de dominio.

### 6.2 Tests HTTP de Integración Liviana (Bajo Demanda)

En caso de ser requeridos expresamente por el usuario, se ejecutan sobre el router Axum utilizando `tower::ServiceExt::oneshot`:

```rust
let response = app_router().oneshot(
    HttpRequest::post("/api/v1/users")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"Ada","email":"ada@example.com"}"#))?
).await?;
assert_eq!(response.status(), StatusCode::OK);
```

Ejecución sobre el router Axum utilizando `tower::ServiceExt::oneshot`:

```rust
let response = app_router().oneshot(
    HttpRequest::post("/api/v1/users")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"Ada","email":"ada@example.com"}"#))?
).await?;
assert_eq!(response.status(), StatusCode::OK);
```

---

## 7. Auditoría Automatizada e Invariantes de Compilación

### 7.1 Expresiones Regulares de Verificación Arquitectónica

```bash
# Fronteras de capas
grep -rn "use crate::" src/domain --include="*.rs" | grep -v "use crate::domain"
grep -rn "use crate::infrastructure" src/application --include="*.rs"
grep -rn "use crate::application\|use crate::infrastructure::driving" src/infrastructure/driven --include="*.rs"
grep -rn "use crate::infrastructure::driven" src/infrastructure/driving --include="*.rs"
grep -rn "use crate::domain\|use crate::application\|use crate::infrastructure" src/shared --include="*.rs"

# Fugas de tipos
grep -rEn "\b\w+(Input|Output)\b" src/domain src/application --include="*.rs"
grep -rn "ObjectId\|bson::" src/domain src/application src/infrastructure/driving --include="*.rs"
grep -rEn "\b\w+Model\b" src/domain src/application src/infrastructure/driving --include="*.rs"

# Persistencia y Soft-Delete
grep -rn "delete_one\|delete_many\|find_one_and_delete\|drop(" src --include="*.rs"
grep -rEn '"[a-z]+[A-Z][a-zA-Z]*"' src/infrastructure/driven/mongo --include="*.rs"

# Errores y Logging
cargo clippy --all-targets -- -D warnings 2>&1 | grep -i "unwrap\|expect"
grep -n "message = err.to_string()\|message: err.to_string()" src/infrastructure/driving/http_axum/server/error.rs

# Observabilidad
grep -rn "TraceLayer" src --include="*.rs"
grep -rn "reqwest::Client::new\|reqwest::ClientBuilder" src --include="*.rs" | grep -v "shared/http_client.rs"

# Wiring y Casts
grep -n "\.create_indexes(" src/main.rs | grep -v ":[0-9]*: *//"
grep -rn "as Arc<dyn" src/ --include="*.rs" | grep -v ":[0-9]*: *//"
```

_Salida esperada para todos los comandos: vacía._

### 7.2 Secuencia Obligatoria de Quality Gate

```bash
# 1. Formato estricto (rustfmt.toml: max_width=100, edition 2024)
cargo fmt --all -- --check

# 2. Lints con denegación de warnings y antipatrones
cargo clippy --all-targets -- -D warnings

# 3. Orden alfabético y agrupado en Cargo.toml
cargo sort --grouped --check

# 4. Suite completa de pruebas
cargo test --all-targets

# 5. Compilación del workspace
cargo check
```

### 7.3 Criterios de Completitud (Definition of Done)

- Quality Gate verificado y en verde en los 5 pasos.
- Fronteras de capas y confinamiento de tipos verificados por auditoría.
- Toda lectura y conteo en repositorios filtra `"deleted_at": { "$exists": false }`.
- Creación de índices encapsulada privadamente en `Repository::new`.
- Métodos públicos de servicio instrumentados con `#[tracing::instrument]` y al menos un campo de contexto.
- Errores externos mapeados exclusivamente a variantes de `DomainError`.
- Entidades nuevas registradas en los 7 puntos obligatorios.
- Aserciones de test realizadas sobre `error.code()` (cuando aplique por tests solicitados o preexistentes).
- Cero mocks, fakes o dobles de prueba creados a menos que hayan sido solicitados expresamente.
- Variables de entorno nuevas documentadas en `.env.example` y `shared/config.rs`.
