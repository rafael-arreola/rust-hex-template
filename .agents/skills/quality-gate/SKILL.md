---
name: quality-gate
description: Secuencia de verificación previa a cada commit — formato, lints, orden de dependencias, tests y compilación. Úsala antes de commitear, al cerrar cualquier otra skill, o cuando pidan "verificar que todo está bien".
---

# Quality gate

Ejecutar **en este orden** (de más barato a más caro). Un paso en rojo detiene la secuencia: se arregla y se reinicia desde el paso 1.

```bash
# 1. Formato (rustfmt.toml manda; max_width=100, edition 2024)
cargo fmt --all -- --check

# 2. Lints — el template niega unwrap_used, expect_used y dbg_macro en build
cargo clippy --all-targets -- -D warnings

# 3. Orden de dependencias en Cargo.toml (alfabético, agrupado)
cargo sort --grouped --check

# 4. Tests
cargo test

# 5. Compilación completa (atrapa lo que --all-targets ya cubrió; opcional si 2 y 4 pasaron)
cargo check
```

## Cómo arreglar cada paso

| Paso falla | Fix |
| --- | --- |
| `fmt` | `cargo fmt --all` y revisar el diff — no ajustar `rustfmt.toml` para "hacer pasar" código. |
| `clippy: unwrap_used/expect_used` | Reescribir con `?`, `ok_or_else(DomainError::...)` o `map_err`. En tests está permitido (`clippy.toml`). **Nunca** `#[allow]` en código de producción sin discusión. |
| `clippy` (otros) | Atender el lint; si es un falso positivo real, `#[allow]` puntual con comentario de una línea explicando la restricción. |
| `sort` | `cargo sort --grouped` (aplica el orden). |
| `test` | Arreglar el código o el test — reportar cuál era el bug, no solo "ya pasa". |

## Reglas del gate

- No se remueve la sección `[lints.clippy]` de `Cargo.toml` ni se relajan sus `deny` — son la garantía en build del "no unwrap/expect".
- Si se agregó una dependencia: debe estar importada realmente en `src/` (el test definitivo: `cargo check` falla sin ella). Dependencias solo usadas en tests van a `[dev-dependencies]`.
- Si se tocó `Cargo.toml`, correr el paso 3 aunque "solo" se cambió una versión.
- Reportar el resultado con la salida real de los comandos que fallaron; si todo pasa, decirlo en una línea.

## Herramientas faltantes

```bash
cargo sort --version || cargo install cargo-sort
```

Si `cargo sort` no está disponible y no se puede instalar, verificar el orden alfabético manualmente y decirlo en el reporte.
