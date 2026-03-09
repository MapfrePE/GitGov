# Plan: Alinear CLAUDE.md con el código real

## Hallazgos del audit (3 agentes exploradores)

### Errores factuales
1. `outbox/outbox.rs` referenciado pero NO existe → es `outbox/queue.rs`
2. `server_commands.rs` listado 2 veces en tabla de archivos críticos
3. Nombre de struct: docs dicen "ServerStats" — verificar si es "AuditStats" en código

### Endpoints faltantes (~24)
- /stats/daily, /team/overview, /team/repos
- /org-users CRUD, /org-invitations CRUD
- /api-keys/{id}/revoke, /outbox/lease, /outbox/lease/metrics
- /pr-merges, /admin-audit-log, /chat/ask, /feature-requests
- /org-invitations/preview/{token}, /org-invitations/accept
- /identities/aliases, /users/{login}/erase, /users/{login}/export
- /clients, /me, /orgs

### Variables de entorno faltantes (~25+)
- Chat/LLM: GEMINI_API_KEY, GEMINI_MODEL, GITGOV_CHAT_LLM_*
- SSE: GITGOV_SSE_MAX_CONNECTIONS
- Cache: GITGOV_LOGS_CACHE_TTL_MS, GITGOV_STATS_CACHE_TTL_MS, etc.
- Outbox coord: GITGOV_OUTBOX_GLOBAL_COORD_*, GITGOV_OUTBOX_SERVER_LEASE_*
- Security: GITGOV_ENV, GITGOV_STRICT_ACTOR_MATCH, etc.
- DB pool: GITGOV_DB_MAX/MIN_CONNECTIONS
- Rate limits: LOGS/STATS/CHAT_PER_MIN

### Archivos críticos faltantes
- notifications.ts, tauri.ts, GovernanceRulesPanel.tsx, PolicyEditorPanel.tsx
- branch_rule.rs (contiene GitGovConfig completo)

### Frontend undocumented
- 17/19 componentes control_plane
- 4/5 stores
- 9 interfaces de governance en types.ts
- 6/7 utilidades en lib/

### Estado del Proyecto desactualizado
- Notifications, CSP, SSE rate limit, policy editor UI no mencionados

## Plan de ejecución (sección por sección)

### Paso 1: Verificar discrepancias clave
- Verificar outbox/outbox.rs vs queue.rs
- Verificar ServerStats vs AuditStats
- Listar endpoints reales del main.rs

### Paso 2: Editar CLAUDE.md — Errores factuales
- Fix outbox path
- Remove duplicate server_commands entry
- Fix struct name si aplica

### Paso 3: Editar CLAUDE.md — Tabla de endpoints
- Agregar endpoints faltantes

### Paso 4: Editar CLAUDE.md — Variables de entorno
- Agregar env vars faltantes por categoría

### Paso 5: Editar CLAUDE.md — Archivos críticos
- Agregar archivos nuevos, corregir paths

### Paso 6: Editar CLAUDE.md — Estado del proyecto
- Actualizar features, tests, pendientes

### Paso 7: Validar coherencia final
- Re-leer CLAUDE.md editado
- Cross-check con código
