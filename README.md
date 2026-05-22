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
  - **DTOs** (`src/{entity}/dtos/`): Los contratos de entrada (`*Input` con validaciones `validator`) y de salida (`*Output` serializables).
  - **Handlers/Routes** (`src/{entity}/routes.rs`): Controladores de Axum que validan el JSON, llaman al servicio y retornan un JSON homogeneizado.
  - **Composition Root** (`src/main.rs`): **El único lugar del sistema** donde se instancian los adaptadores (Mongo, Redis) y se inyectan en los casos de uso para arrancar el servidor Axum.

---

## 🛠️ Guía del Desarrollador (¿Dónde pongo mi código?)

| Si quieres hacer esto...                               | ...debes escribir/modificar código en:                                                                                                                                                                                       |
| :----------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Agregar una dependencia de terceros**                | 1. Defínela en el `Cargo.toml` de la raíz en `[workspace.dependencies]`. <br>2. Hereda la dependencia en el `Cargo.toml` del sub-crate específico usando `{ workspace = true }`.                                             |
| **Añadir un campo a un registro de base de datos**     | 1. Modifica la entidad en `core/domain/src/entities/`. <br>2. Modifica el modelo de base de datos en `infra/mongo/src/{entidad}/model.rs`. <br>3. Modifica la conversión `From`/`Into` entre ambos.                          |
| **Crear una nueva regla o proceso de negocio**         | 1. Agrega el método en el servicio correspondiente en `core/usecases/src/`.                                                                                                                                                  |
| **Exponer un nuevo endpoint REST**                     | 1. Crea los DTOs de entrada/salida en `infra/http-axum/src/{entidad}/dtos/`. <br>2. Añade el handler y su ruteo en `infra/http-axum/src/{entidad}/routes.rs`.                                                                |
| **Consumir un servicio externo (ej. Stripe o un ERP)** | 1. Declara el trait/puerto en `core/domain/src/port/{servicio}.rs`. <br>2. Crea un nuevo sub-crate de infraestructura (ej. `infra/stripe`) para implementar dicho puerto. <br>3. Inyéctalo en `infra/http-axum/src/main.rs`. |

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
  cargo run -p infra-http-axum
  ```
