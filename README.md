# Template Hexagonal — Rust

## ⚠️ Lo primero: borra el código de ejemplo

Las tres entidades que trae la plantilla existen **solo como referencia** y
llevan el prefijo `Demo` para que sea imposible confundirlas con código tuyo:
`DemoUser`, `DemoProduct`, `DemoOrder` (más `DemoPricingService`).

Al arrancar un servicio nuevo, bórralas:

```bash
rm -rf src/domain/entities/demo_*.rs \
       src/domain/port/demo_*.rs \
       src/domain/services/demo_pricing.rs \
       src/application/demo_*.rs \
       src/infrastructure/driven/mongo/demo_* \
       src/infrastructure/driving/http_axum/routes/demo_*
```

Y quita sus `pub mod demo_*;` de `application.rs`, `domain/entities.rs`,
`domain/port.rs`, `domain/services/mod.rs`, `driven/mongo.rs` y
`driving/http_axum/routes.rs`, más sus campos en `server/state.rs` y su wiring
en `main.rs`. `cargo check` te dice exactamente qué falta.

Lo que **no** lleva prefijo se queda: es la plantilla en sí (`DomainError`,
`DomainId`, `GenericApiResponse`, middlewares, tracer, config, health).

> Los ejemplos de nombres en este documento (`User`, `UserRepositoryPort`…)
> ilustran la convención para una entidad **real** — sin prefijo. El prefijo
> `Demo` marca únicamente el código desechable que viene en el repo.

---

```text
.
├── build/
├── Cargo.toml                      ← Manifiesto del proyecto (unificado)
├── rustfmt.toml
├── clippy.toml
└── src/                            ← Único directorio src del proyecto
    ├── main.rs                     ← entry point, DI manual, Composition Root
    ├── domain.rs                   ← Enrutador de dominio
    ├── domain/                     ← Corazón del negocio, cero dependencias externas
    │   ├── entities/               ← structs de datos puros + typed IDs
    │   │   └── {entidad}.rs
    │   ├── port/                   ← puertos de SALIDA: traits que el dominio exige
    │   │   └── {entidad}.rs        ← {Entity}RepositoryPort
    │   ├── services/               ← reglas de negocio puras, sin I/O
    │   │   └── {servicio}.rs
    │   ├── error.rs                ← DomainError enum + DomainResult<T>
    │   ├── pagination.rs           ← Pagination struct
    │   ├── values.rs               ← DomainId<T>
    │   └── macros/                 ← as_json! macro
    ├── application.rs              ← Enrutador de aplicación
    ├── application/                ← Casos de uso: entrada a la ejecución del negocio
    │   ├── {entidad}.rs            ← {Entity}Service (orquesta ports + domain services)
    │   └── shared/                 ← sub-flujos reusables CON I/O
    ├── shared.rs                   ← Enrutador de capacidades técnicas
    ├── shared/                     ← Herramientas técnicas sin lógica de negocio
    │   ├── config.rs               ← carga de .env + struct Env
    │   ├── http_client.rs          ← cliente reqwest instrumentado (traceparent)
    │   ├── tracer.rs               ← OpenTelemetry + tracing + TracerGuard
    │   └── tracer/format.rs        ← formatter JSON de Cloud Logging
    ├── infrastructure.rs           ← Enrutador general de infraestructura
    └── infrastructure/
        ├── driven.rs               ← Enrutador de driven adapters
        ├── driven/                 ← Adaptadores de SALIDA: implementan domain::ports
        │   ├── mongo.rs            ← Enrutador de Mongo
        │   ├── mongo/
        │   │   └── {entidad}/
        │   │       ├── model.rs        ← {Entity}Model (BSON)
        │   │       └── repository.rs   ← {Entity}Repository — impl {Entity}RepositoryPort
        │   └── redis.rs            ← Adaptador de conexiones y helpers Redis
        └── driving.rs              ← Enrutador de driving adapters
            └── driving/            ← Adaptadores de ENTRADA: importan application/ directo
                ├── http_axum.rs    ← Enrutador del servidor HTTP
                └── http_axum/
                    ├── routes/{entidad}.rs
                    └── server/     ← error, health, middleware, response, state, validation
```

---

## Flujo de una petición

```text
driving (HTTP) → application ({Entity}Service) → domain::port (trait)
                                                      ↓
                                              driven ({Entity}Repository en Mongo)
```

El **driving** (`http_axum/`) importa directamente `application::OrderService` (tipo concreto, sin trait).
El **application** (`OrderService`) depende de `domain::port::OrderRepositoryPort` (trait definido por el dominio).
El **driven** (`mongo/`) implementa `domain::port::OrderRepositoryPort`.

El dominio **nunca** sabe quién lo llama ni quién implementa sus puertos.

---

## Qué va en cada capa / módulo

### `src/domain/` — El negocio puro

| Módulo      | Va aquí                                                                                                                                               | NO va aquí                                                            |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `entities/` | Structs con `Serialize`/`Deserialize`. Datos puros + typed IDs (`DomainId<T>`).                                                                       | Métodos con I/O, lógica que llama a otros crates                      |
| `port/`     | **Solo traits de salida**: `create`, `find_by_id`, `update`, `delete`. Define QUÉ necesita el dominio, no CÓMO. `#[async_trait]` + `Send + Sync`.     | Tipos concretos, imports de `mongodb`, `axum`                         |
| `services/` | Lógica de negocio pura: `apply_discount`, `calculate_tax`. Opera solo sobre entidades. Sin constructor con dependencias. Sin I/O.                     | Constructores con parámetros de infraestructura. Llamadas a DB, HTTP. |
| `error.rs`  | `DomainError` enum con variantes `NotFound`, `AlreadyExists`, `Invalid`, `Internal`. Cada variante expone `.code()` estable. `DomainResult<T>` alias. | Errores de infraestructura (esos se mapean aquí)                      |
| `values.rs` | `DomainId<T>` — ID tipado con marcador fantasma.                                                                                                      | Lógica de negocio                                                     |
| `macros/`   | `as_json!` macro para serialización inline en tracing events.                                                                                         | —                                                                     |

```rust
// src/domain/entities/demo_order.rs
pub struct Order {
    pub id: Option<OrderId>,
    pub total_price: f64,
}

// src/domain/port/demo_order.rs — solo traits de SALIDA
#[async_trait]
pub trait OrderRepositoryPort: Send + Sync {
    async fn create(&self, order: &Order) -> DomainResult<OrderId>;
    async fn find_by_id(&self, id: &OrderId) -> DomainResult<Option<Order>>;
}

// src/domain/services/demo_pricing.rs — sin constructor, sin I/O
pub struct PricingService;

impl PricingService {
    pub fn apply_discount(&self, order: &Order) -> f64 {
        if order.total_price > 1000.0 { order.total_price * 0.90 } else { order.total_price }
    }
}
```

---

### `src/application/` — Casos de uso

Es la **única puerta de entrada** a la lógica de negocio. Orquesta entidades, domain services, y ports.

| Módulo          | Va aquí                                                                                                           | NO va aquí                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `{entidad}.rs`  | Struct `{Entity}Service` con métodos `create`, `get`, `update`, `delete`. Recibe `Arc<dyn Port>` por constructor. | Lógica de negocio pura (eso va en `domain/services/`) |
| `shared/mod.rs` | Sub-flujos con I/O reusables: `FraudChecker`, `InventoryReserver`. Reciben repos/clients por constructor.         | Entry points (eso va en `{entidad}.rs`)               |

```rust
// src/application/demo_order.rs
pub struct OrderService {
    order_repo: Arc<dyn OrderRepositoryPort>,   // ← trait definido en domain::port
    pricing: PricingService,                     // ← domain service, sin I/O
}

impl OrderService {
    pub fn new(order_repo: Arc<dyn OrderRepositoryPort>) -> Self {
        Self { order_repo, pricing: PricingService::new() }
    }

    #[tracing::instrument(skip_all)]
    pub async fn create_order(&self, user_id: &UserId, product_id: &ProductId, quantity: i32) -> DomainResult<Order> {
        // 1. Validar existencia (puerto)
        let product = self.product_repo.find_by_id(product_id).await?;
        // 2. Lógica de negocio pura (domain service)
        let total = self.pricing.apply_discount(&draft);
        // 3. Persistir (puerto → driven adapter)
        let id = self.order_repo.create(&order).await?;
        Ok(order)
    }
}
```

---

### `src/infrastructure/driven/` — Adaptadores de salida

Implementan los traits definidos en `domain::ports`. Conexión con el mundo real.

| Directorio         | Contiene                    | Implementa                             |
| ------------------ | --------------------------- | -------------------------------------- |
| `mongo/{entidad}/` | Operaciones CRUD en MongoDB | `domain::port::{Entity}RepositoryPort` |
| `redis/`           | Conexiones y helpers Redis  | Tipo concreto (sin trait)              |

```rust
// src/infrastructure/driven/mongo/demo_order/repository.rs
pub struct OrderRepository { collection: Collection<OrderModel> }

#[async_trait]
impl OrderRepositoryPort for OrderRepository {
    async fn create(&self, order: &Order) -> DomainResult<OrderId> {
        let model = OrderModel::from(order.clone());
        self.collection.insert_one(model).await
            .map_err(|e| DomainError::database(e.to_string()))?;
        // ...
    }
}
```

---

### `src/infrastructure/driving/` — Adaptadores de entrada

Importan **directamente** los services de `application/`. Sin trait de por medio.

```rust
// src/infrastructure/driving/http_axum/routes/demo_order.rs
pub async fn create_order(
    State(service): State<Arc<OrderService>>,  // ← tipo concreto, sin trait
    ValidatedBody(input): ValidatedBody<CreateOrderInput>,  // ← JSON o MessagePack según Content-Type
) -> Result<GenericApiResponse<OrderOutput>, ApiError> {
    let order = service.create_order(&input.user_id, &input.product_id, input.quantity).await?;
    Ok(GenericApiResponse::success(order.into()))
}
```

---

### `src/shared/` — Capacidades técnicas

| Archivo            | Qué contiene                                                          | Dependencias         |
| ------------------ | --------------------------------------------------------------------- | -------------------- |
| `config.rs`        | Carga de `.env` + struct `Env` + `OnceLock`                           | Ninguna del proyecto |
| `http_client.rs`   | Cliente reqwest instrumentado (propaga `traceparent` en salidas HTTP) | Ninguna del proyecto |
| `tracer.rs`        | OpenTelemetry + tracing subscriber setup + `TracerGuard` (flush)      | Ninguna del proyecto |
| `tracer/format.rs` | Formatter JSON de Cloud Logging (correlación log↔trace)               | Ninguna del proyecto |

**Regla:** cero lógica de negocio. Se importan como dependencia en `application/` e `infrastructure/`. El dominio **no** las usa.

---

### `src/main.rs` — Wiring (DI manual)

```rust
// 1. Conexiones
let mongo = MongoProvider::new(&env.service_name, &env.mongo_url, &env.mongo_db).await?;
let _redis = RedisProvider::new(&env.redis_url, &env.redis_prefix).await?;

// 2. Driven adapters — `new` es async y falible porque también crea los
//    índices de la colección; no hay un create_indexes() aparte que olvidar.
let order_repo = Arc::new(OrderRepository::new(&db).await?);

// 3. Application services (casos de uso)
//    Sin casts: Rust coacciona Arc<Concrete> a Arc<dyn Trait> por su cuenta.
let order_service = Arc::new(OrderService::new(order_repo, user_repo, product_repo));

// 4. Driving adapters
let state = AppState { health_checker, user_service, product_service, order_service };
ServerLauncher::new(state).with_http(env.port).run().await;
```

---

## Reglas de decisión

| Si tu código…                                            | Va en…                                     | Porque…                       |
| -------------------------------------------------------- | ------------------------------------------ | ----------------------------- |
| Es una estructura de datos con `Serialize`/`Deserialize` | `src/domain/entities/`                     | Es el modelo de dominio       |
| Es un trait que el dominio necesita (repo)               | `src/domain/port/`                         | El dominio define el contrato |
| Opera solo sobre entidades, sin I/O                      | `src/domain/services/`                     | Lógica de negocio pura        |
| Orquesta un flujo completo (entry point)                 | `src/application/{entidad}.rs`             | Caso de uso                   |
| Orquesta I/O y se reusa en varios casos de uso           | `src/application/shared/`                  | Sub-flujo reusable            |
| Habla con MongoDB, Redis                                 | `src/infrastructure/driven/{mongo,redis}/` | Adaptador de persistencia     |
| Habla con un servicio externo HTTP/gRPC                  | `src/infrastructure/driven/{servicio}/`    | Cliente externo               |
| Recibe requests HTTP                                     | `src/infrastructure/driving/http_axum/`    | Adaptador de entrada          |
| Es log, trace, config                                    | `src/shared/`                              | Herramienta técnica           |

---

## Convenciones

| Elemento             | Formato                     | Ejemplo                        |
| -------------------- | --------------------------- | ------------------------------ |
| Archivos y carpetas  | `snake_case` singular       | `order_item.rs`, `order_item/` |
| Entidad              | `PascalCase`                | `OrderItem`                    |
| Puerto (trait)       | `PascalCase` + `Port`       | `OrderItemRepositoryPort`      |
| Application Service  | `PascalCase` + `Service`    | `OrderItemService`             |
| Domain Service       | `PascalCase` + `Service`    | `PricingService`               |
| Driven Repository    | `PascalCase` + `Repository` | `OrderItemRepository`          |
| Driving Handler HTTP | función `snake_case`        | `create_order_item`            |
| DTOs                 | `*Input` / `*Output`        | `CreateOrderItemInput`         |
| Constructor          | `new(...)`                  | `OrderItemService::new(...)`   |
| Colección MongoDB    | `snake_case` plural         | `order_items`                  |
| Ruta API             | plural                      | `/api/v1/order-items`          |

- **Constructores:** retornan tipo concreto (`Arc<Service>`), nunca trait.
- **Tracing:** `#[tracing::instrument(skip_all)]` en servicios de `application/`.
- **Logging:** `tracing::info!` solo en `application/` e `infrastructure/`. El dominio no loguea.
- **Errores:** dominio devuelve `DomainResult<T>`. `application/` también. `driving/` mapea a `ApiError`.
- **Nunca:** `panic!()`, `unwrap()`, `expect()` fuera de tests.

---

## 🚀 Comandos rápidos

```bash
cargo check                          # Validar compilación del monolito
cargo test                           # Correr todas las pruebas
cargo run                            # Arrancar el servidor en modo desarrollo
cargo fmt                            # Formatear todo el proyecto
cargo sort -g                        # Ordenar dependencias alfabéticamente
cargo clippy                         # Linting
```

---

## 🌍 Variables de entorno

### Requeridas (el servicio no arranca sin ellas)

| Variable       | Descripción                          | Ejemplo                     |
| -------------- | ------------------------------------ | --------------------------- |
| `SERVICE_NAME` | Nombre del servicio en logs y traces | `user-service`              |
| `MONGO_URL`    | URI completa de conexión a MongoDB   | `mongodb://localhost:27017` |
| `MONGO_DB`     | Nombre de la base de datos           | `users_db`                  |
| `REDIS_URL`    | URI completa de conexión a Redis     | `redis://localhost:6379`    |

### Opcionales (tienen valor por defecto)

| Variable              | Default   | Descripción                                      |
| --------------------- | --------- | ------------------------------------------------ |
| `PORT`                | `3000`    | Puerto HTTP del servidor                         |
| `SERVICE_ENV` / `ENV` | `DEV`     | Entorno: `DEV`, `STAGING`, `PRODUCTION`          |
| `PROJECT_ID`          | _(vacío)_ | ID del proyecto GCP para Cloud Trace             |
| `DEBUG_LEVEL`         | `info`    | Nivel de logs (`debug`, `info`, `warn`, `error`) |
| `CORS_ORIGINS`        | `*`       | Orígenes CORS permitidos (separados por coma)    |
| `REDIS_PREFIX`        | `service` | Prefijo para keys en Redis                       |
| `DRAIN_TIMEOUT_SECS`  | `10`      | Segundos de espera durante graceful shutdown     |
