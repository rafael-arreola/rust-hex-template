# .agents — Skills operativas del template hexagonal

`AGENTS.md` (raíz del repo) es la **constitución**: invariantes, naming, fronteras de capas y templates canónicos. Esta carpeta contiene los **procedimientos operativos** — skills que un agente (o un humano) sigue paso a paso para trabajar con la plantilla de forma precisa, sin re-derivar las reglas en cada tarea.

Cada skill vive en `skills/<nombre>/SKILL.md` con frontmatter (`name`, `description`) compatible con el formato Agent Skills de Claude Code.

## Índice

| Skill | Cuándo usarla |
| --- | --- |
| [`add-entity`](skills/add-entity/SKILL.md) | Agregar una entidad/agregado nuevo con CRUD completo en todas las capas. |
| [`add-endpoint`](skills/add-endpoint/SKILL.md) | Agregar un endpoint/caso de uso a una entidad existente. |
| [`add-domain-service`](skills/add-domain-service/SKILL.md) | Agregar lógica de negocio pura (sin I/O) reutilizable. |
| [`add-driven-adapter`](skills/add-driven-adapter/SKILL.md) | Integrar un servicio externo (HTTP) o cache Redis como driven adapter. |
| [`test-entity`](skills/test-entity/SKILL.md) | Escribir tests de servicios de aplicación con puertos falsos y tests HTTP. |
| [`architecture-audit`](skills/architecture-audit/SKILL.md) | Verificar que el código respeta las invariantes (greps ejecutables). |
| [`quality-gate`](skills/quality-gate/SKILL.md) | Secuencia de comandos previa a cada commit (fmt, clippy, sort, test). |

## Activación en Claude Code

Las skills se descubren desde `.claude/skills/`. Este repo incluye un symlink:

```bash
.claude/skills -> ../.agents/skills
```

Si el symlink no existe (p. ej. tras un clone en un entorno que no los preserva):

```bash
ln -s ../.agents/skills .claude/skills
```

Para otros agentes (Cursor, Codex, Gemini CLI) los archivos funcionan como documentación normal: enlázalos desde el prompt o desde `AGENTS.md`.

## Reglas de mantenimiento

- Si una invariante cambia, se cambia **primero** en `AGENTS.md` y después se propaga a las skills afectadas. Nunca al revés.
- Las skills referencian archivos reales del template (`src/application/demo_user.rs`, etc.) como ejemplo canónico; si esos archivos cambian de forma estructural, actualiza las skills en el mismo PR.
- Una skill nueva se agrega solo si describe un procedimiento repetible; las decisiones puntuales van en `AGENTS.md` o en un ADR.
