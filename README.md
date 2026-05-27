# Rust Hexagonal Workspace Template

Este es un template para microservicios en Rust basado en una **Arquitectura Hexagonal pura** utilizando **Cargo Workspaces**. Su estructura está diseñada para aislar la lógica de negocio de los detalles técnicos (bases de datos, servidores web) y asegurar un código mantenible que dure décadas.

---

## 📐 Estructura del Workspace (De un Vistazo)

El compilador de Rust enforza los límites de dependencias de forma física. Las dependencias fluyen siempre hacia el **Dominio** (adentro):

```
infra-http-axum (Axum) ──> usecases (Negocio) ──> domain (Reglas puras) <── infra-mongo (MongoDB)
```

### 1. `core/domain` (El Corazón)

- **¿Qué va aquí?**: Las reglas de negocio puras que nunca cambian, independientemente de la base de datos o framework HTTP.
  - **Entidades** (`src/entities/`): Estructuras de datos puras de negocio (ej. `User`, `Product`).
  - **Puertos** (`src/port/`): `traits` (interfaces) que definen qué operaciones necesitamos de la infraestructura (ej. `UserRepositoryPort`).
  - **Errores y Valores** (`src/error.rs`, `src/values.rs`): El enum `DomainError` y los IDs tipados seguros.
- **Dependencias**: Cero dependencias locales. Solo librerías utilitarias ultra-estables (`serde`, `chrono`).

### 2. `core/usecases` (La Lógica)

- **¿Qué va aquí?**: Los servicios de aplicación que orquestan las acciones del sistema.
  - **Servicios** (`src/`): Clases de lógica (ej. `UserService`) que reciben sus dependencias por constructor vía inyección dinámica (`Arc<dyn Port>`).
- **Dependencias**: Únicamente conoce a `core/domain`. No sabe nada de bases de datos o HTTP.

### 3. `infra/mongo` (La Persistencia)

- **¿Qué va aquí?**: La implementación concreta de base de datos para MongoDB.
  - **Modelos BSON** (`src/{entity}/model.rs`): El documento físico que se guarda en la base de datos (con mapeos `From` y `Into` hacia las entidades de dominio).
  - **Repositorios** (`src/{entity}/repository.rs`): La implementación física de los puertos del dominio.
- **Dependencias**: Únicamente conoce a `core/domain`.

### 4. `infra/redis` (El Caching)

- **¿Qué va aquí?**: Clientes de conexión y utilitarios para interactuar con Redis.

### 5. `infra/http-axum` (La Presentación)

- **¿Qué va aquí?**: La puerta de entrada HTTP al sistema.
  - **DTOs** (`src/routes/{entidad}/dtos/`): Los contratos de entrada (`*Input` con validaciones `validator`) y de salida (`*Output` serializables).
  - **Handlers/Routes** (`src/routes/{entidad}/`): Controladores de Axum que validan el JSON, llaman al servicio y retornan un JSON homogeneizado.
  - **Servidor** (`src/server/`): El lanzador del servidor Axum y su apagado graceful.
- **Dependencias**: Se comporta como una librería pura exponiendo sus rutas y estado.

### 6. `service` (El Composition Root)

- **¿Qué va aquí?**: El binario principal del microservicio.
  - **Configuración y Telemetría** (`src/config.rs`, `src/telemetry.rs`): Carga de variables de entorno y configuración global de OpenTelemetry y tracing.
  - **Punto de Entrada** (`src/main.rs`): El único lugar del sistema donde se instancian los adaptadores físicos (Mongo, Redis, etc.), se inyectan en los casos de uso y se arranca el servidor HTTP expuesto por `infra-http-axum`.
- **Dependencias**: Importa todos los crates para realizar el cableado global (DI).

---

## 🛠️ Guía del Desarrollador (¿Dónde pongo mi código?)

| Si quieres hacer esto...                               | ...debes escribir/modificar código en:                                                                                                                                                                               |
| :----------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Agregar una dependencia de terceros**                | 1. Defínela en el `Cargo.toml` de la raíz en `[workspace.dependencies]`. <br>2. Hereda la dependencia en el `Cargo.toml` del sub-crate específico usando `{ workspace = true }`.                                     |
| **Añadir un campo a un registro de base de datos**     | 1. Modifica la entidad en `core/domain/src/entities/`. <br>2. Modifica el modelo de base de datos en `infra/mongo/src/{entidad}/model.rs`. <br>3. Modifica la conversión `From`/`Into` entre ambos.                  |
| **Crear una nueva regla o proceso de negocio**         | 1. Agrega el método en el servicio correspondiente en `core/usecases/src/`.                                                                                                                                          |
| **Exponer un nuevo endpoint REST**                     | 1. Crea los DTOs de entrada/salida en `infra/http-axum/src/{entidad}/dtos/`. <br>2. Añade el handler y su ruteo en `infra/http-axum/src/{entidad}/routes.rs`.                                                        |
| **Consumir un servicio externo (ej. Stripe o un ERP)** | 1. Declara el trait/puerto en `core/domain/src/port/{servicio}.rs`. <br>2. Crea un nuevo sub-crate de infraestructura (ej. `infra/stripe`) para implementar dicho puerto. <br>3. Inyéctalo en `service/src/main.rs`. |

---

## 🚀 Comandos Rápidos de Uso Frecuente

- **Validar compilación de todo el Workspace**:
  ```bash
  cargo check
  ```
- **Correr todas las pruebas**:
  ```bash
  cargo test
  ```
- **Arrancar el servidor Axum en modo desarrollo**:
  ```bash
  cargo run -p service
  ```

---

## ⚙️ Funcionalidades Operacionales (DevOps / Kubernetes)

El template incluye varios mecanismos pensados para entornos de producción orquestados. Ninguno requiere configuración adicional para funcionar en local.

### Health Checks (`/healthz` y `/readyz`)

El servidor expone dos endpoints de sondeo fuera de `/api/v1`, ideales para probes de Kubernetes:

| Endpoint       | Propósito       | Respuesta                                   |
| -------------- | --------------- | ------------------------------------------- |
| `GET /healthz` | Liveness probe  | `200` siempre (proceso vivo)                |
| `GET /readyz`  | Readiness probe | `200` si MongoDB responde ping, `503` si no |

La lógica de readiness se inyecta en `main.rs` como un closure (`HealthChecker`) que verifica la conexión real a la base de datos al momento de la llamada.

### Graceful Shutdown (`DRAIN_TIMEOUT_SECS`)

Cuando el proceso recibe `SIGTERM` (Ctrl+C o Kubernetes terminando el pod), el servidor:

1. Deja de aceptar conexiones nuevas inmediatamente
2. Espera `DRAIN_TIMEOUT_SECS` segundos para que los requests en vuelo terminen
3. Se apaga limpiamente

Esto evita cortar requests a la mitad durante un deploy. El valor por defecto es **10 segundos**. Ajustable con la variable de entorno `DRAIN_TIMEOUT_SECS`. Si tu balanceador de carga ya maneja drain, puedes dejarlo en 0.

### Timeout de Request

El template **no** impone un timeout a nivel de aplicación. El timeout por request debe ser controlado a nivel de infraestructura (load balancer, reverse proxy, Ingress controller, service mesh). Esto da flexibilidad: un endpoint de exportación pesada puede necesitar 5 minutos, mientras que un health check necesita 2 segundos.

### Middleware `X-Request-Id`

Toda respuesta HTTP incluye el header `X-Request-Id`. El middleware:

- **Propaga** el ID si el request entrante ya trae el header
- **Genera** un UUID v7 si no existe
- **Inyecta** el valor en el tracing span para correlacionar logs

Esto permite trazar un request a través de múltiples servicios sin depender de OpenTelemetry.

---

## 🌍 Variables de Entorno

### Requeridas (el servicio no arranca sin ellas)

| Variable       | Descripción                          | Ejemplo                     |
| -------------- | ------------------------------------ | --------------------------- |
| `SERVICE_NAME` | Nombre del servicio en logs y traces | `user-service`              |
| `MONGO_URL`    | URI completa de conexión a MongoDB   | `mongodb://localhost:27017` |
| `MONGO_DB`     | Nombre de la base de datos           | `users_db`                  |
| `REDIS_URL`    | URI completa de conexión a Redis     | `redis://localhost:6379`    |

### Opcionales (tienen valor por defecto)

| Variable             | Default   | Descripción                                      |
| -------------------- | --------- | ------------------------------------------------ |
| `PORT`               | `3000`    | Puerto HTTP del servidor                         |
| `APP_ENV` / `ENV`    | `DEV`     | Entorno: `DEV`, `STAGING`, `PRODUCTION`          |
| `PROJECT_ID`         | _(vacío)_ | ID del proyecto GCP para Cloud Trace             |
| `DEBUG_LEVEL`        | `info`    | Nivel de logs (`debug`, `info`, `warn`, `error`) |
| `CORS_ORIGINS`       | `*`       | Orígenes CORS permitidos (separados por coma)    |
| `REDIS_PREFIX`       | `service` | Prefijo para keys en Redis                       |
| `DRAIN_TIMEOUT_SECS` | `10`      | Segundos de espera durante graceful shutdown     |

> 📖 La telemetría usa plugins oficiales de Google Cloud. Referencia: [Rust libraries for Google Cloud](https://docs.cloud.google.com/rust/docs/reference?hl=es-419)

---

## 🎨 Estilo de Código

El proyecto incluye:

- **`rustfmt.toml`**: `max_width=100`, `tab_spaces=4`, `edition=2024`
- **`clippy.toml`**: permite `unwrap`, `expect` y `dbg!` solo en tests

Para formatear todo el workspace:

```bash
cargo fmt --all
```

Para linting:

```bash
cargo clippy --workspace
```

---

## 📦 Gestión de Dependencias Ordenadas (`cargo-sort`)

Para mantener todos los archivos `Cargo.toml` del workspace limpios, estandarizados y ordenados alfabéticamente por bloques, este proyecto utiliza la herramienta **`cargo-sort`**.

### 1. Instalación de `cargo-sort`

Para poder usar esta herramienta de forma global en tu máquina, debes instalarla ejecutando:

```bash
cargo install cargo-sort
```

### 2. Uso con la bandera de agrupamiento `-g`

En este proyecto es de carácter **obligatorio** realizar la ordenación manteniendo los bloques y tablas de dependencias separados por saltos de línea lógicos (comportamiento agrupado). Para lograr esto, se debe usar siempre la bandera `-g` (`--grouped`):

- **Formatear y ordenar todos los archivos `Cargo.toml` del workspace**:

  ```bash
  cargo sort -w -g
  ```
