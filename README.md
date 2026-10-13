# Rust Hexagonal Template

Plantilla de servicio con inversión de dependencias, composición manual y autenticación Auth0/Hydra por JWT/JWKS lista para activar. `AGENTS.md` contiene las reglas técnicas; `PROJECT.md` está reservado para las políticas del negocio que se implemente sobre ella.

## Organización

```text
src/
├── main.rs                    # Configuración, criptografía, tracing y ExitCode
├── app.rs                     # Construcción y conexión de la aplicación
├── domain/                    # Dominio puro y contratos de puertos
│   ├── core/                  # Contexto, errores, IDs tipados, identidad y paginación
│   ├── entities/              # Agregados y tipos específicos del negocio
│   │   └── mod.rs             # Declaración de módulos de entidades
│   └── port/                  # Contratos por capacidad, incluido AuthProvider
├── application/               # Casos de uso; dependencias explícitas y reutilizables
├── infrastructure/
│   ├── http/                  # Kit de transporte Axum; no conoce application
│   │   ├── mod.rs             # Arranque (launcher, capas, señales, drenado, 404/405) y fachada pública
│   │   ├── health.rs          # Sondas y señal de drenado
│   │   ├── format.rs          # JSON/MessagePack: Content-Type, negociación por Accept y codificación
│   │   ├── context.rs         # Extractor de Context y ContextRequestExt
│   │   ├── error.rs           # ApiError y registro de errores HTTP
│   │   ├── response.rs        # Envelope, Paginated y paginated()
│   │   ├── request/           # mod.rs (PaginationQuery), body.rs, path.rs y query.rs
│   │   ├── middleware.rs      # Timeout y validación de Content-Encoding
│   │   ├── middleware/        # trace.rs
│   │   └── auth/              # mod.rs (extractor de AuthPrincipal), middleware.rs y permissions.rs
│   ├── rest/                  # Endpoints: mod.rs + <recurso>/{mod,handlers,dtos}.rs
│   ├── persistence.rs         # Declara el único adaptador de almacenamiento (estilo 2018)
│   ├── persistence/
│   │   └── mongo/             # mod.rs, error.rs y <entidad>/{mod,model,repository}.rs
│   └── providers/             # Recursos e integraciones externas
│       ├── mod.rs             # Declaración de proveedores
│       ├── auth/              # Verificación Auth0/Hydra/JWKS
│       ├── gcp/               # mod.rs, gcs.rs (GcsProvider) y gct.rs (CloudTasksProvider)
│       ├── mongo.rs           # mongo::connect: cliente y conectividad
│       └── redis.rs           # RedisProvider
└── shared/                    # Configuración, cliente HTTP y telemetría
```

Los endpoints de `infrastructure/rest` consumen servicios de aplicación y el kit `infrastructure/http`, que solo conoce el núcleo. Los repositorios de `infrastructure/persistence` y el engine de autenticación de `infrastructure/providers` implementan sus puertos. `mongo::connect`, en `providers/mongo.rs`, configura el cliente y comprueba conectividad; no conoce entidades ni documentos. `src/app.rs` lo invoca y entrega `Database` a los repositorios, que reciben `&Database` sin importar el provider. Los proveedores que solo preparan conexiones técnicas no necesitan un puerto del dominio. La aplicación no importa MongoDB ni Axum.

`persistence` y `providers` dependen del dominio, de utilidades técnicas de `shared` y de su propio árbol, no de aplicación ni HTTP, y no se importan directamente entre sí. HTTP no importa persistencia, providers ni `shared`. `envs::get()` solo se usa en `main.rs`, `app.rs` y `shared/`. `src/app.rs` es la raíz de composición que conecta esos árboles mediante dependencias explícitas; la reorganización cambia ubicaciones e imports, no comportamiento ni contratos del dominio o de aplicación.

`http/mod.rs` arranca el servidor (`ServerLauncher`/`ServerOptions`, capas, bind, señales, drenado y fallbacks 404/405) y es la fachada del kit: reexporta todo lo que consumen `rest/` y `app.rs`, mientras sus módulos internos son privados. Solo `health` sigue siendo público, para `health::router`, que monta las sondas con la comprobación de readiness que inyecta `app.rs`. `format.rs` concentra JSON/MessagePack (detección de `Content-Type`, negociación por `Accept` y codificación), que consumen `request/body.rs` y `response.rs`. Los endpoints viven en `infrastructure/rest/`: `rest/mod.rs` compone los recursos y cada uno separa router (`mod.rs`), handlers y DTOs; el kit `http/` no importa `application` ni `rest`.

`domain/core/` agrupa tipos base reutilizables: `DomainError`/`DomainResult`, `DomainId`, `Actor`/`AuthPrincipal` y `Pagination`/`PageResult`. Las entidades y los puertos pueden consumirlos; estos módulos base no dependen de agregados ni de puertos de negocio. No es una capa `src/core/` paralela ni un lugar para drivers, configuración, DTOs o utilidades de logging.

Un repositorio puede compartirse entre varios servicios y un servicio entre varios routers. Cada router recibe sus dependencias concretas; no existe un `AppState` global que obligue a registrar cada servicio en varios lugares.

## Arranque

La toolchain está fijada en `rust-toolchain.toml` y coincide con CI y Docker. La configuración se carga una vez; valores inválidos, recursos obligatorios inaccesibles o fallos de bind terminan con código de error. La telemetría se cierra antes de salir.

La autenticación no se cablea por defecto y los endpoints demo son públicos. **Antes de desplegar endpoints de negocio, activa la autenticación** (ver [Activación](#activación)) o deja registrado en `PROJECT.md` por qué el servicio es público: `build/cloudbuild.yaml` despliega con `--allow-unauthenticated`; el ingress solo admite tráfico del balanceador, pero si este es público, sin autenticación en la aplicación el servicio queda abierto a Internet. Valida esa decisión con IT/Security. Las sondas `/healthz` y `/readyz` son siempre públicas.

| Variable                  | Requerida | Default / significado                                                           |
| ------------------------- | --------- | ------------------------------------------------------------------------------- |
| `SERVICE_NAME`            | Sí        | Nombre del servicio y recurso de telemetría.                                    |
| `MONGO_URL`               | Sí        | URI MongoDB, suministrada fuera del repositorio.                                |
| `MONGO_DB`                | Sí        | Base de datos.                                                                  |
| `PORT`                    | No        | `8080`; Cloud Run inyecta su propio `PORT`.                                     |
| `SERVICE_ENV`             | No        | `DEV`. Única variable de ambiente.                                              |
| `PROJECT_ID`              | No        | Ausente utiliza tracing JSON local, sin intentar descubrir credenciales GCP.    |
| `DEBUG_LEVEL`             | No        | `info`; acepta filtros válidos de tracing.                                      |
| `REQUEST_TIMEOUT_SECS`    | No        | `30`, mayor que cero.                                                           |
| `DRAIN_TIMEOUT_SECS`      | No        | `10`, mayor que cero.                                                           |
| `BODY_LIMIT_BYTES`        | No        | `33554432`, aplicado al cuerpo descomprimido.                                   |
| `ENABLE_MSGPACK`          | No        | `true`; controla negociación de salida, no entrada.                             |

Las listas rechazan elementos vacíos y deduplican sin alterar mayúsculas/minúsculas. Un valor opcional inválido no se sustituye silenciosamente por el default. No se distribuyen credenciales ni archivos de entorno en la plantilla.

## Autenticación y permisos

Un único `AuthEngine<X>` implementa el puerto `AuthProvider`. `BuildAuthProvider<X>` configura JWKS, issuer, audiences, algoritmo y clientes admitidos; `X` describe las claims del emisor e implementa `AuthClaims`. El módulo no lee configuración global ni crea tareas al importarse: `build(http).await` valida la configuración e inicializa JWKS.

```text
src/infrastructure/providers/auth/
├── mod.rs        # Declaración y reexports del módulo
├── engine.rs     # Builder, engine, AuthClaims y VerifiedJwt<X>
├── auth0.rs      # Esquema Auth0 y extracción de datos
├── hydra.rs      # Esquema Hydra y perfil M2M explícito
├── jwks.rs       # Llaves, caché, refresco y verificación internos
└── document.rs   # Deserialización JSON sin claves duplicadas
```

Las claims registradas se validan independientemente de `X`: RS256, firma, expiración, issuer, audiences de API, subject y `nbf` cuando existe. El esquema propio puede incluir esos campos o no; no puede omitir las comprobaciones comunes. El engine rechaza claves JSON duplicadas también en objetos anidados. `.algorithm(...)` selecciona el algoritmo de confianza, no el del header; actualmente solo RS256 está soportado y otro valor hace fallar el arranque.

- **Auth0:** `Auth0Claims` obtiene cliente de `oauth_cid`, `cid` o `azp`; conserva la unión existente de `permissions` y `scope`. Mantiene los aliases legacy de `https://oauth.99minutos.com/cid` y `permissions`. `get_user()` obtiene únicamente `user_id` explícito; no infiere un usuario humano de cualquier `sub` ni de email.
- **Hydra:** `HydraClaims` obtiene cliente de `client_id` o `ext.client_id`. Conserva `scp` como string o array, pero hereda permisos vacíos y usuario ausente. Para un contrato donde scopes sí representen permisos, se define una estructura propia con override de `get_permissions()`; `HydraScopes::tokens()` facilita la extracción. El perfil `HydraMachineToMachineClaims` permite usar `sub` como cliente únicamente al seleccionarlo en código; el builder exige una allowlist no vacía para ese perfil.
- **Otro emisor JWT:** se define su estructura deserializable e implementación de `AuthClaims`; no se modifica el engine ni el middleware.
- **Tokens opacos:** no están soportados por este engine; una integración de introspección puede implementar el mismo puerto `AuthProvider` por separado.

El Bearer se descarta tras autenticarlo. `Actor` distingue aplicación cliente, sujeto verificado y usuario opcional; no contiene token ni utiliza email como identidad canónica. Un handler obtiene la identidad verificada declarando `AuthPrincipal` como extractor; sin el middleware montado responde 401.

### Construcción y uso

Ejemplo ilustrativo para construir una integración; las URLs son valores de ejemplo, no configuración del proyecto:

```rust
use jsonwebtoken::Algorithm;

use crate::domain::core::error::DomainResult;
use crate::infrastructure::providers::auth::{Auth0Claims, AuthEngine, BuildAuthProvider};
use crate::shared::http_client::HttpClient;

async fn build_auth0(http: HttpClient) -> DomainResult<AuthEngine<Auth0Claims>> {
    BuildAuthProvider::<Auth0Claims>::new()
        .name("auth0")
        .jwks_url("https://identity.example.com/.well-known/jwks.json")
        .issuer("https://identity.example.com/")
        .audiences(["https://api.example.com"])
        .algorithm(Algorithm::RS256)
        .allowed_client_ids(["configured-client"])
        .build(http)
        .await
}
```

Para Hydra se utiliza `BuildAuthProvider::<HydraClaims>` con su configuración independiente. Para M2M se selecciona `HydraMachineToMachineClaims`. Cada engine se puede compartir mediante `Arc` y montar como `Arc<dyn AuthProvider>` en grupos de rutas distintos, igual que en el proyecto de referencia. No hay selector por entorno: el emisor se elige con el tipo de claims en código (`Auth0Claims`, `HydraClaims`, `HydraMachineToMachineClaims` o uno propio) y, si conviven varios, cada uno recibe sus propias variables y su grupo de rutas.

### Activación

1. Añade a `shared/envs.rs` y `.env.example` las variables del emisor: `AUTH_JWKS_URL` (HTTPS; HTTP solo hacia loopback), `AUTH_ISSUER` (valor exacto de `iss`) y `AUTH_AUDIENCES` (audiences de API, no de ID token) como `required`, y `AUTH_ALLOWED_CLIENT_IDS` con `or(.., Vec::new())`.
2. Construye el engine en `src/app.rs` y móntalo por fuera del router de la API. `build` descarga el JWKS antes de servir; si falla, el arranque falla:

   ```rust
   let auth: Arc<dyn AuthProvider> = Arc::new(
       BuildAuthProvider::<Auth0Claims>::new()
           .name("auth0")
           .jwks_url(env.auth_jwks_url.clone())
           .issuer(env.auth_issuer.clone())
           .audiences(env.auth_audiences.clone())
           .allowed_client_ids(env.auth_allowed_client_ids.clone())
           .build(HttpClient::try_new().map_err(|_| DomainError::internal("HTTP client"))?)
           .await?,
   );
   let api = rest::router(/* servicios */)
       .route_layer(middleware::from_fn_with_state(auth, auth_middleware));
   ```

3. Declara los permisos por método en `rest/<resource>/mod.rs` con `require_permission`, `require_any_permission` o `require_all_permissions`.

`engine.verify(credential).await` devuelve `VerifiedJwt<X>` y permite acceder a `claims()`, `subject()`, `issuer()`, `expires_at()` o consumir `into_claims()`. Solo comprueba JWT y esquema: no aplica allowlists ni permisos. `AuthProvider::authenticate()` añade la extracción y comprobación del cliente para entregar `AuthPrincipal`; el middleware utiliza esta segunda operación.

### Extensión de claims

Ejemplo ilustrativo de una estructura propia, sin incorporar otro emisor al arranque:

```rust
use serde::Deserialize;

use crate::domain::core::error::DomainResult;
use crate::infrastructure::providers::auth::AuthClaims;

#[derive(Deserialize)]
struct CustomClaims {
    application_id: String,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    permissions: Vec<String>,
}

impl AuthClaims for CustomClaims {
    fn get_client_id(&self) -> DomainResult<&str> {
        Ok(&self.application_id)
    }

    fn get_user(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    fn get_permissions(&self) -> Vec<String> {
        self.permissions.clone()
    }
}
```

`get_client_id()` es obligatorio; el engine rechaza identificadores vacíos o con caracteres de control. `get_user()` devuelve `None` y `get_permissions()` una lista vacía por defecto, por lo que se sobrescriben solo cuando el contrato del emisor lo necesita. El engine normaliza y deduplica los permisos. Los perfiles que derivan cliente de sujeto M2M declaran `REQUIRES_CLIENT_ALLOWLIST = true` para exigir la allowlist durante construcción.

### Autorización de rutas

Esquema de permisos ilustrativo para los demos una vez activada la autenticación:

| Recurso            | GET             | POST / PATCH / DELETE existentes |
| ------------------ | --------------- | -------------------------------- |
| `/api/v1/users`    | `users:read`    | `users:write`                    |
| `/api/v1/products` | `products:read` | `products:write`                 |
| `/api/v1/orders`   | `orders:read`   | `orders:write`                   |

Para un grupo que solo requiera autenticación, se montan handlers sin capas `require_*_permission`, manteniendo el middleware de autenticación. Hydra sin permisos recibirá 403 en las rutas que los exijan. El catálogo Crossborder del proyecto de referencia no forma parte de este boilerplate.

Las rutas declaran permisos por método, por ejemplo con `post(create_user).merge(get(list_users).require_permission("users:read"))`; cada verbo se combina con `merge` para que su capa no afecte a los demás.


La extensión admite también `require_any_permission` y `require_all_permissions`. Los requisitos vacíos se mantienen cerrados; `"*"` no concede permisos arbitrarios. La autenticación se monta por fuera de esas capas. Las verificaciones de propiedad, tenant y estado pertenecen al caso de uso, no se deducen de tener un permiso de ruta.

El caché JWKS tiene TTL de 15 minutos, cooldown de 30 segundos y un worker single-flight creado en bootstrap. La cancelación de una petición no cancela el refresco compartido. El worker se detiene al caer su dueño; no representa trabajo de negocio garantizado en Cloud Run.

Se limita cada credencial a 16 KiB, la descarga JWKS a 1 MiB y 64 llaves, el `kid` a 128 bytes y RSA a 2048–4096 bits compatibles con el backend. No se siguen redirecciones ni URLs indicadas por un JWT.

Una credencial inválida produce 401 con `WWW-Authenticate: Bearer`; permisos insuficientes producen 403. Si no puede verificarse por indisponibilidad de JWKS y no hay caché vigente utilizable, se devuelve 503. Los errores públicos no incluyen detalles del decoder.

## Proveedores GCP de referencia

`GcsProvider` y `CloudTasksProvider` usan el SDK oficial (`google-cloud-storage`, `google-cloud-tasks-v2`, `google-cloud-auth`) sin features por defecto, con el proveedor `ring` instalado en `main.rs` y Application Default Credentials. No se cablean por defecto ni tienen variables en `Env`, porque no todos los servicios los usan. Implementan los puertos `ObjectStorage` (`domain/port/object_storage.rs`) y `TaskQueue` (`domain/port/task_queue.rs`), que no mencionan el SDK: un servicio los recibe por constructor como `Arc<dyn ObjectStorage>`/`Arc<dyn TaskQueue>`. Para activarlos cuando una capacidad los necesite:

1. Añade sus variables a `shared/envs.rs` (obligatorias, u opcionales con validación todo o nada) y a `.env.example`: `GCS_BUCKET` para Storage; `CLOUD_TASKS_LOCATION`, `CLOUD_TASKS_QUEUE`, `CLOUD_TASKS_TARGET_URL`, `CLOUD_TASKS_SERVICE_ACCOUNT` y opcionalmente `CLOUD_TASKS_AUDIENCE` para Tasks (requiere `PROJECT_ID`). Redis usa `REDIS_URL` y un prefijo de claves.
2. Construye el provider en `app.rs` (`GcsProvider::new(bucket)`, `CloudTasksProvider::new(CloudTasksSettings { .. })`, `RedisProvider::new(url, prefix)`) e inyéctalo como `Arc<dyn ObjectStorage>`/`Arc<dyn TaskQueue>` en el servicio que lo consuma. Si no puede construirse, el arranque debe fallar.

- `GcsProvider` (`providers/gcp/gcs.rs`) implementa `ObjectStorage`: `upload_file(path, bytes, content_type)`, `download_file(path)` (en memoria; `NotFound` si no existe) y `get_public_url(path, expires_in, disposition)`, una URL firmada V4 de 1 s a 7 días. Las credenciales de usuario (`gcloud auth application-default login`) no pueden firmar; en local, usa ADC impersonada (`--impersonate-service-account`).
- `CloudTasksProvider` (`providers/gcp/gct.rs`) implementa `TaskQueue` y encola tareas HTTP bajo `CLOUD_TASKS_TARGET_URL` con token OIDC:
  - `dispatch(&payload)` (sobre `dyn TaskQueue`) para tipos que implementan `TaskPayload` (`PATH`, `METHOD` y `task_id()` opcional), enviados como JSON.
  - `enqueue(TaskRequest::post(path).json(&body)?.header(k, v).id(id).schedule_at(t).dispatch_deadline(d))` para cualquier verbo, cuerpo crudo (`body(bytes, content_type)`) y cabeceras.
  - Devuelve `Enqueued::Created { name }` o `Enqueued::AlreadyEnqueued` (409 de una tarea con nombre). Solo las tareas con nombre son idempotentes: el SDK las reintenta (3 intentos, 10 s); las anónimas se envían una vez.
  - Rechaza cabeceras reservadas (`Authorization`, `Host`, `Content-Type`, `Content-Length`, `User-Agent`, `X-Google-*`, `X-AppEngine-*`, `X-CloudTasks-*`), cuerpo en verbos distintos de POST/PUT/PATCH y rutas fuera del destino.

Permisos IAM mínimos (validar con IT/Security antes de producción):

| Recurso | Principal | Rol |
| --- | --- | --- |
| Bucket | Service account de ejecución | `roles/storage.objectUser` |
| Service account de ejecución (sobre sí misma) | Service account de ejecución | `roles/iam.serviceAccountTokenCreator`, para firmar URLs sin clave |
| Cola | Service account de ejecución | `roles/cloudtasks.enqueuer` |
| Servicio de destino (Cloud Run) | `CLOUD_TASKS_SERVICE_ACCOUNT` | `roles/run.invoker` |

El SDK arrastra `opentelemetry 0.33`; sus spans no se unen a las trazas de la aplicación (0.32).

## Errores

`DomainError` separa lo que lee el usuario de lo que se registra en logs. El constructor indica a dónde va su texto: en errores de cliente (`invalid`, `not_found`, `business_rule`, ...) es público; en errores de servidor y autenticación (`internal`, `service_unavailable`, `unauthorized`, ...) es interno y el cliente recibe un mensaje genérico.

```rust
DomainError::business_rule("Insufficient stock for the requested quantity")
    .with_code("INSUFFICIENT_STOCK")                       // cause que recibe el cliente
    .with_internal(format!("Requested {quantity}, available {stock}")); // solo logs

repo.try_reserve_stock(id, quantity, now).await.context("reserving stock for a new order")?;
```

El handler registra una vez `error.code`, `error.internal`, `error.context` y `http.status_code`; un error que no llega a HTTP se registra con `error.log()`. El detalle interno nunca incluye mensajes crudos de drivers o SDKs ni datos personales.

## Contexto de operación

`Context` (`domain/core/context.rs`) transporta datos comunes de la operación con un valor por tipo, como el `context.Context` de Go. Un middleware los inserta, el handler lo recibe como extractor y lo pasa a los servicios que lo usan. Ejemplo ilustrativo (`Country` no existe en la plantilla):

```rust
// Middleware: inserta lo que conoce y validó
request.context().insert(Country::try_from(header)?);

// Handler → servicio
async fn create(ctx: Context, ValidatedBody(input): ValidatedBody<CreateInput>) -> Result<..> {
    service.create(&ctx, input.try_into()?).await
}

// Servicio: require si es obligatorio (su ausencia es un 500 de cableado), get si es opcional
let country = ctx.require::<Country>()?;
let actor = ctx.get::<Actor>();

// Worker o tarea: lo reconstruye desde su payload
let ctx = Context::new().with(country).with(actor);
```

Con la autenticación activa, `auth_middleware` inserta el `Actor`. Un dato primitivo se modela como newtype (`struct TenantId(String)`) para que su tipo lo identifique.

## HTTP y persistencia

Los handlers importan `ValidatedBody`, `ValidatedQuery`, `PathParam`, `PaginationQuery` y `paginated` desde la fachada `crate::infrastructure::http`; sus implementaciones están en `http/request/` y `http/response.rs`. Los rechazos conservan estados apropiados, el envelope y la negociación JSON/MessagePack. El timeout devuelve 504; 404/405 también usan el envelope y 405 mantiene `Allow`.

- Creación: 201 con `{ trace_id, data }`.
- Consulta/actualización: 200 con `{ trace_id, data }`.
- Eliminación: 204 sin cuerpo.
- Error: `{ trace_id, data: { message }, cause }`.
- Paginación: `{ trace_id, data: { data, total, page, limit } }`.

El límite de cuerpo (`DefaultBodyLimit`) se aplica al cuerpo descomprimido cuando un extractor lo lee, es decir, después de autenticar. Un endpoint que necesite streaming deberá tener una composición específica.

La paginación valida sus límites también fuera de HTTP. `PageResult<T>` agrupa elementos y total con el mismo filtro; no promete una lectura snapshot. MongoDB mantiene soft-delete, índices construidos por el repositorio, orden estable y conversiones fallibles sin IDs inventados. Los DTOs exigen el ID de recursos persistidos y los estados de salida tienen un mapeo explícito que conserva su representación, sin depender de `Debug`.

OpenTelemetry permanece en la familia 0.32 compatible con `reqwest-tracing`. El exportador se actualizó a 0.27 y los spans de persistencia provienen de los repositorios: no se habilita la instrumentación nativa de MongoDB que depende de otra familia. Los timeouts salientes son 10 segundos totales y 3 de conexión.

## Ejemplos y límites

Se conservan `DemoUser`, `DemoProduct` y `DemoOrder` como tres ejemplos conectados. Para iniciar un servicio productivo, sustituye sus capacidades, puertos, adaptadores y routers y actualiza la composición en `src/app.rs`.

Estos ejemplos **no son un modelo de compras listo para producción**: conservan precios `f64` y compensación inline. Una cancelación después de reservar stock o una escritura de resultado incierto requiere una garantía de consistencia acordada; la plantilla no la oculta como transacción. Dinero, idempotencia y transacciones se definen con las políticas del servicio en `PROJECT.md`.

Se retiraron todas las pruebas, fakes, harnesses, la dev-dependency de pruebas y sus pasos de CI/Docker. La ausencia de una suite no implica cobertura funcional. No se genera otra durante scaffolding sin solicitud explícita.

## Comprobaciones

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo sort --grouped --check
cargo check --locked --all-targets
cargo build --locked
```

No hay un control automático de fronteras entre capas: dentro de un único crate el compilador permite que cualquier módulo importe otro, así que las reglas de `AGENTS.md` se revisan en el diff. Clippy bloquea `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` y `dbg!` desde `Cargo.toml`. CI mantiene estas compuertas y una auditoría RustSec separada. Las versiones efectivas y sus features están en `Cargo.toml`/`Cargo.lock`.

Para construir la imagen:

```sh
docker build --progress=plain -f build/Dockerfile -t rust-hex-template:verify .
```

`.dockerignore` excluye archivos de entorno, llaves, Git, artefactos locales y documentación (`*.md`) del contexto de build. La compilación usa el lockfile y la misma toolchain del proyecto; la etapa builder exige `cargo fmt --check` y copia el binario como `/app/service` sea cual sea el nombre del paquete.

## Despliegue

`build/cloudbuild.yaml` construye la imagen con caché remota en Artifact Registry y despliega en Cloud Run con ingress `internal-and-cloud-load-balancing` (solo vía balanceador) y salida por la VPC compartida. El servicio corre con la service account del build (`$SERVICE_ACCOUNT_EMAIL`), igual que global99.

| Substitution | Origen | Uso |
| --- | --- | --- |
| `_SERVICE_NAME` | Trigger | Nombre del servicio, de la imagen y `SERVICE_NAME` |
| `_MONGO_DB` | Trigger | `MONGO_DB` |
| `_MONGO_URL` | Trigger | Secreto de Secret Manager para `MONGO_URL` (p. ej. `mongo-url:latest`) |
| `_VPC_CONNECTOR` / `_VPC_CONNECTOR_SUBNET` | Trigger | Red y subred de la VPC compartida |
| `_SERVICE_ENV` | Default `PRD` | `SERVICE_ENV` |
| `_DEBUG_LEVEL` | Default `info` | `DEBUG_LEVEL` |
| `_REGION` | Default `us-central1` | Región del servicio y de la subred |
| `_VPC_HOST_PROJECT` | Default `shared-resources-451520` | Proyecto anfitrión de la VPC |
| `_CACHE` | Default `${_SERVICE_NAME}-cache` | Imagen de caché de Buildx |

`PROJECT_ID` toma el proyecto del build y `PORT` no se declara porque Cloud Run lo reserva. Las variables opcionales sin substitution (timeouts, límite de cuerpo, MessagePack) usan su default; al activar un provider se añaden aquí junto con sus variables.
