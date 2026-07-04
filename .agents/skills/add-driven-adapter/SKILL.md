---
name: add-driven-adapter
description: Integrar una dependencia externa como driven adapter — un servicio HTTP de terceros o cache Redis — definiendo el puerto en domain e implementándolo en infrastructure/driven. Úsala cuando pidan consumir una API externa, agregar cache, o conectar cualquier sistema del que el servicio depende.
---

# Agregar un driven adapter

Regla de oro: el dominio define **qué** necesita (puerto); infraestructura define **cómo** se obtiene (adapter). El application service solo conoce el trait.

## Receta A — Servicio HTTP externo

### 1. Puerto en `src/domain/port/{name}.rs`

Nómbralo por la capacidad, no por la tecnología: `PaymentGatewayPort`, no `StripeClientPort`.

```rust
use crate::domain::error::DomainResult;
use async_trait::async_trait;

#[async_trait]
pub trait PaymentGatewayPort: Send + Sync {
    async fn charge(&self, order_id: &str, amount: f64) -> DomainResult<String>;
}
```

Registrar `pub mod payment_gateway;` en `src/domain/port.rs`. Firmas solo con tipos de dominio y primitivos — nada de tipos de `reqwest` ni structs de la API externa.

### 2. Adapter en `src/infrastructure/driven/{name}.rs`

Registrar `pub mod {name};` en `src/infrastructure/driven.rs`. Si crece (modelos de request/response propios), usa el patrón carpeta: `driven/{name}.rs` como router + `driven/{name}/model.rs` + `driven/{name}/client.rs`.

```rust
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::port::payment_gateway::PaymentGatewayPort;
use async_trait::async_trait;
use reqwest_middleware::ClientWithMiddleware;

pub struct PaymentGateway {
    client: ClientWithMiddleware,
    base_url: String,
}

impl PaymentGateway {
    pub fn new(client: ClientWithMiddleware, base_url: &str) -> Self {
        Self { client, base_url: base_url.to_string() }
    }
}

#[async_trait]
impl PaymentGatewayPort for PaymentGateway {
    #[tracing::instrument(skip_all, fields(%order_id))]
    async fn charge(&self, order_id: &str, amount: f64) -> DomainResult<String> {
        let response = self
            .client
            .post(format!("{}/charges", self.base_url))
            .json(&serde_json::json!({ "order_id": order_id, "amount": amount }))
            .send()
            .await
            .map_err(|e| DomainError::external_service("payment-gateway", e.to_string()))?;
        // ... parsear respuesta; todo error se mapea igual, nunca se propaga crudo
        todo!()
    }
}
```

Reglas duras:

- El cliente HTTP **siempre** es `shared::http_client::instrumented_client()`, inyectado desde `main.rs`. Un `reqwest::Client::new()` directo no propaga `traceparent` y rompe el tracing distribuido — el audit lo caza.
- Los structs de request/response de la API externa viven en el adapter; jamás cruzan hacia application/domain.
- **No loggees el error que propagas con `?`**: el choke point de la frontera driving (`server/error.rs`) ya lo loggea una vez con el detalle interno y la severidad de `DomainError::severity()`. Pon el detalle crudo en el `message` del error — `Display` es la vista interna (logs); el cliente recibe automáticamente la vista genérica de `public_message()`.
- Config nueva (`PAYMENT_GATEWAY_URL`) entra por `shared/config.rs` (struct `Env` + `require_url`) y se documenta en `.env.example`.

### 3. Wiring en `src/main.rs` → `serve()`

```rust
let http_client = shared::http_client::instrumented_client();
let payment_gateway = Arc::new(PaymentGateway::new(http_client, &env.payment_gateway_url));
// se inyecta al service que lo necesita como Arc<dyn PaymentGatewayPort>
```

Si el adapter valida conectividad al arrancar, sigue el patrón fail-fast (early `return` tras `tracing::error!`).

## Receta B — Cache Redis

El provider ya existe (`src/infrastructure/driven/redis.rs`) con el wiring comentado en `main.rs` y las vars comentadas en `shared/config.rs`.

1. Descomenta `redis_url`/`redis_prefix` en `shared/config.rs` y el bloque `RedisProvider` en `serve()` (fail-fast); agrega `REDIS_URL`/`REDIS_PREFIX` a `.env.example`.
2. Define el puerto en domain solo si application necesita la capacidad de forma abstracta (p. ej. `UserCachePort` con `get`/`set`). Si el cache es un detalle interno de un repositorio Mongo (cache-aside), implémentalo dentro del adapter Mongo recibiendo el `RedisProvider` por constructor — sin puerto nuevo.
3. Claves siempre con prefijo: `provider.get_path(&["users", &id])` → `"{prefix}:users:{id}"`.
4. Decide y documenta la política de fallo: para cache-aside, un error de Redis se **degrada** (log `warn!` + continuar contra Mongo), no tumba la petición. Solo mapea a `DomainError::database` cuando Redis es fuente de verdad (locks, rate limits).
5. Serializa valores con `serde_json` y TTL explícito siempre — sin TTL no hay invalidación.

## Checklist de cierre

- [ ] Puerto en `domain/port/` con nombre por capacidad, `#[async_trait]` + `Send + Sync`.
- [ ] Ningún tipo de la tecnología (reqwest, redis) en las firmas del puerto.
- [ ] Errores externos mapeados con los constructores de `DomainError` (`external_service` / `database`); nunca crudos, nunca loggeados localmente si se propagan.
- [ ] Cliente HTTP instrumentado inyectado desde `main.rs`.
- [ ] Config nueva en `Env` + `.env.example`; deps nuevas ordenadas (`cargo sort --grouped`).
- [ ] `architecture-audit` y `quality-gate` en verde.
