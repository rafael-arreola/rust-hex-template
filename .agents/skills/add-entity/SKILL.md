---
name: add-entity
description: Scaffold completo de una nueva entidad (aggregate root) en todas las capas del template hexagonal — domain → application → mongo → http_axum → wiring. Úsala cuando pidan agregar una entidad, recurso CRUD o agregado nuevo al servicio.
---

# Agregar una entidad nueva

Fuente de verdad: `AGENTS.md`. Esta skill fija el orden de trabajo, los archivos exactos a crear y los 7 puntos de registro que siempre se olvidan. El ejemplo usa `invoice` — sustituye respetando el naming: archivo/struct singular (`invoice.rs`, `Invoice`), colección y ruta plural (`invoices`, `/api/v1/invoices`).

Trabaja **en orden de dependencias** (el mismo orden compila a la primera):

```
1. domain/entities  →  2. domain/port  →  3. application  →  4. driven/mongo  →  5. http_axum  →  6. wiring
```

## 1. Entidad — `src/domain/entities/invoice.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::values;

#[derive(Debug, Clone)]
pub struct InvoiceMarker;
pub type InvoiceId = values::DomainId<InvoiceMarker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Invoice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<InvoiceId>,
    pub number: String,
    pub amount: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Invoice {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
```

**Registrar**: `pub mod invoice;` en `src/domain/entities.rs` (orden alfabético).

Reglas: `id` es `Option` (es `None` hasta que el repositorio lo asigna en `create`); `deleted_at` es obligatorio (soft-delete universal); referencias a otras entidades usan su typed ID (`UserId`), nunca `String`.

## 2. Puerto — `src/domain/port/invoice.rs`

Solo si la entidad es aggregate root (se persiste por sí misma). Value objects y entidades hijas no llevan puerto.

```rust
use crate::domain::entities::invoice::{Invoice, InvoiceId};
use crate::domain::error::DomainResult;
use crate::domain::pagination::Pagination;
use async_trait::async_trait;

#[async_trait]
pub trait InvoiceRepositoryPort: Send + Sync {
    async fn create(&self, invoice: &Invoice) -> DomainResult<InvoiceId>;
    async fn find_by_id(&self, id: &InvoiceId) -> DomainResult<Option<Invoice>>;
    async fn find_all(&self, pagination: Pagination) -> DomainResult<Vec<Invoice>>;
    async fn update(&self, id: &InvoiceId, invoice: &Invoice) -> DomainResult<bool>;
    async fn delete(&self, id: &InvoiceId) -> DomainResult<bool>;
    async fn count(&self) -> DomainResult<u64>;
}
```

**Registrar**: `pub mod invoice;` en `src/domain/port.rs`.

Reglas: `#[async_trait]` + `Send + Sync` siempre; firmas solo con tipos de dominio y primitivos; agrega únicamente los métodos que el caso de uso necesita (no CRUD por reflejo).

## 3. Servicio de aplicación — `src/application/invoice.rs`

Sigue el template de `src/application/demo_user.rs` (ejemplo canónico). Esqueleto:

```rust
use crate::domain::entities::invoice::{Invoice, InvoiceId};
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::pagination::Pagination;
use crate::domain::port::invoice::InvoiceRepositoryPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct InvoiceService {
    repo: Arc<dyn InvoiceRepositoryPort>,
}

impl InvoiceService {
    pub fn new(repo: Arc<dyn InvoiceRepositoryPort>) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip_all, fields(%number))]
    pub async fn create_invoice(&self, number: &str, amount: f64) -> DomainResult<Invoice> {
        // Validación semántica (unicidad, existencia, reglas) va AQUÍ, contra el puerto.
        let now = chrono::Utc::now();
        let mut invoice = Invoice {
            id: None,
            number: number.to_string(),
            amount,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        let id = self.repo.create(&invoice).await?;
        invoice.id = Some(id);

        tracing::info!("Invoice created");
        Ok(invoice)
    }
}
```

**Registrar**: `pub mod invoice;` en `src/application.rs`.

Reglas: constructor con `Arc<dyn Port>`; `#[derive(Clone)]`; cada método público con `#[tracing::instrument(skip_all, fields(...))]` y **al menos un field**; parámetros = primitivos, typed IDs o valores de dominio — nunca DTOs; timestamps con `chrono::Utc::now()` inline.

## 4. Adaptador Mongo

Crear el router `src/infrastructure/driven/mongo/invoice.rs`:

```rust
pub mod model;
pub mod repository;
```

**Registrar**: `pub mod invoice;` en `src/infrastructure/driven/mongo.rs`.

### `mongo/invoice/model.rs`

Copia el patrón de `src/infrastructure/driven/mongo/demo_user/model.rs`:

- `#[serde(rename_all = "snake_case")]` en el struct; el único rename de campo permitido es `_id`.
- `From<Invoice> for InvoiceModel` y `From<InvoiceModel> for Invoice` — **nunca `TryFrom`**. IDs inválidos se manejan en silencio (`ObjectId::parse_str(..).ok()` / `.unwrap_or_default()`).
- Fechas como `bson::DateTime` (conversión `from_chrono`/`to_chrono`).
- FKs a otros agregados se guardan como `ObjectId` (ver `order/model.rs`).

### `mongo/invoice/repository.rs`

Copia el patrón de `user/repository.rs`. Puntos que el audit revisa:

- Colección plural snake_case: `db.collection::<InvoiceModel>("invoices")`.
- **`new` es `async` y falible, y crea los índices adentro** — así "los índices existen" es una propiedad del tipo, no un paso de wiring que se puede olvidar. `create_indexes` queda **privado**:

  ```rust
  impl InvoiceRepository {
      pub async fn new(db: &Database) -> DomainResult<Self> {
          let repo = Self { collection: db.collection::<InvoiceModel>("invoices") };
          repo.create_indexes().await?;
          Ok(repo)
      }

      async fn create_indexes(&self) -> DomainResult<()> { /* ... */ }
  }
  ```

- `create_indexes()` idempotente, con nombres explícitos de índice; siempre incluye el compuesto `{ deleted_at: 1, created_at: -1 }` para listados. Para colecciones efímeras, índice TTL con `IndexOptions::builder().expire_after(Duration::from_secs(...))` en vez de un job de limpieza.
- **Toda** query (`find_one`, `find`, `update_one`, `count_documents`) filtra `doc! { "deleted_at": { "$exists": false } }`.
- `delete` = `$set { deleted_at: now }`; jamás `delete_one`/`delete_many`.
- Todo error del driver se mapea: `.map_err(|e| DomainError::database(e.to_string()))`; ObjectId mal formado → `DomainError::invalid_param("id", "Invoice", &**id)`.
- Campos en `doc! { ... }` en snake_case, exactamente como los serializa el modelo.

## 5. HTTP — rutas y DTOs

Crear `src/infrastructure/driving/http_axum/routes/invoice.rs` (handlers + `router()`) y `routes/invoice/dtos.rs`, copiando `routes/user.rs` y `routes/user/dtos.rs`:

- DTOs: `CreateInvoiceInput` con `#[derive(Deserialize, Validate)]` (validación **sintáctica** aquí); `InvoiceOutput` con `impl From<Invoice>` para convertir con `.into()`.
- Handler = 5 pasos y cero lógica de negocio: `ValidatedBody` → construir typed IDs → llamar al service → `.into()` al Output → `GenericApiResponse::success(...)`. Listados usan `GenericApiResponse::paginated(data, total, page, limit)`.
- `router()` con rutas relativas: `"/"` y `"/{id}"`.

**Registrar** en `src/infrastructure/driving/http_axum/routes.rs`:

```rust
pub mod invoice;
// dentro de app_router():
.nest("/invoices", invoice::router())
```

## 6. Wiring — `state.rs` y `main.rs`

`src/infrastructure/driving/http_axum/server/state.rs`:

```rust
pub invoice_service: Arc<InvoiceService>,   // campo en AppState
impl_from_ref!(AppState, invoice_service, InvoiceService);
```

`src/main.rs`, dentro de `serve()` (mismo patrón fail-fast que las demás — el early return es seguro, el flush del tracer vive en `main`):

```rust
// `new` ya crea los índices: no hay bloque create_indexes() aparte.
let invoice_repo = match InvoiceRepository::new(&db).await {
    Ok(repo) => Arc::new(repo),
    Err(e) => {
        tracing::error!("Failed to initialize InvoiceRepository: {}", e);
        return;
    }
};

// Sin cast explícito: Rust coacciona Arc<Concrete> a Arc<dyn Trait> solo.
let invoice_service = Arc::new(InvoiceService::new(invoice_repo));

let state = AppState { /* ...campos existentes..., */ invoice_service };
```

> Nunca escribas `invoice_repo as Arc<dyn InvoiceRepositoryPort>`: es ruido que
> además arrastra el trait del port a los imports de `main.rs` sin necesidad.

## 7. Verificación

1. `cargo check` — compila.
2. Ejecuta la skill `test-entity` para el service nuevo (mínimo: caso feliz + regla semántica).
3. Ejecuta `architecture-audit` (fronteras, soft-delete, naming).
4. Ejecuta `quality-gate` (fmt, clippy, sort, test).
5. Smoke manual: `curl -X POST localhost:3000/api/v1/invoices -H 'Content-Type: application/json' -d '{...}'` y verifica el envelope (`trace_id` + `data`).

## Checklist de cierre

- [ ] Archivos singulares; colección y ruta plurales.
- [ ] 5 routers de módulo actualizados: `entities.rs`, `port.rs`, `application.rs`, `mongo.rs`, `routes.rs` — y ninguno usa `mod.rs`.
- [ ] `AppState` + `impl_from_ref!` + wiring en `main.rs` con fail-fast.
- [ ] `Repository::new` es `async`/falible y crea los índices; `create_indexes` privado. Cero llamadas a `create_indexes()` y cero `as Arc<dyn ...>` en `main.rs`.
- [ ] `deleted_at` filtrado en TODAS las queries del repositorio.
- [ ] Sin `unwrap()`/`expect()` fuera de tests (clippy lo niega en build).
- [ ] Dependencias nuevas (si las hubo) ordenadas con `cargo sort --grouped`.
