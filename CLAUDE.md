# CLAUDE.md — Instrucciones para Claude Code en GitGov

> Este archivo se carga automáticamente en cada sesión. Léelo completo antes de hacer cualquier cambio.

---

## Qué es GitGov

Sistema de gobernanza de Git distribuido con tres componentes:
1. **Desktop App** — Tauri v2 + React (en `gitgov/`)
2. **Control Plane Server** — Axum + Rust (en `gitgov/gitgov-server/`)
3. **GitHub/Jenkins/Jira Integrations** — Webhooks + OAuth

---

## Golden Path (NUNCA ROMPER)

Este flujo es sagrado. Cualquier cambio debe preservarlo:

1. Desktop detecta archivos cambiados
2. Usuario hace commit desde la app
3. Usuario hace push desde la app
4. Control Plane recibe eventos (`stage_files`, `commit`, `attempt_push`, `successful_push`)
5. Dashboard muestra logs/commits sin errores `401`

**Checklist de validación:** `docs/GOLDEN_PATH_CHECKLIST.md`

**Regla:** Cualquier cambio en auth/token/API key/dashboard/handlers DEBE validar este flujo o documentar por qué no pudo.

---

## Comandos de Desarrollo

```bash
# Desktop App
cd gitgov && npm run tauri dev

# Control Plane Server
cd gitgov/gitgov-server && cargo run

# Tests E2E
cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh

# Jenkins integration test
cd gitgov/gitgov-server/tests && API_KEY="57f1ed59-371d-46ef-9fdf-508f59bc4963" ./jenkins_integration_test.sh

# Jira integration test
cd gitgov/gitgov-server/tests && API_KEY="57f1ed59-371d-46ef-9fdf-508f59bc4963" ./jira_integration_test.sh
```

### Linting (EJECUTAR ANTES DE COMMIT)

```bash
# Server Rust
cd gitgov/gitgov-server && cargo clippy -- -D warnings

# Desktop Rust
cd gitgov/src-tauri && cargo clippy -- -D warnings

# Frontend TypeScript
cd gitgov && npm run lint && npm run typecheck
```

---

## Arquitectura de Autenticación

### Desktop → Control Plane
- Header: `Authorization: Bearer {api_key}` (NUNCA `X-API-Key`)
- Server calcula SHA256 del token y busca en tabla `api_keys` por `key_hash`

### Roles
- **admin:** Acceso total (stats, dashboard, integrations)
- **developer:** Solo sus propios eventos

### GitHub Webhooks
- Validación HMAC con `GITHUB_WEBHOOK_SECRET`

---

## Endpoints del Servidor

| Endpoint | Auth | Propósito |
|----------|------|-----------|
| `/health` | None | Health check |
| `/events` | Bearer | Ingesta de eventos del cliente |
| `/webhooks/github` | HMAC | Webhooks de GitHub |
| `/stats` | Bearer (admin) | Estadísticas |
| `/logs` | Bearer | Eventos combinados |
| `/dashboard` | Bearer (admin) | Datos del dashboard |
| `/jobs/metrics` | Bearer (admin) | Métricas del job queue |
| `/integrations/jenkins` | Bearer | Pipeline events de Jenkins |
| `/integrations/jenkins/status` | Bearer (admin) | Health check Jenkins |
| `/integrations/jenkins/correlations` | Bearer (admin) | Correlaciones commit↔pipeline |
| `/policy/check` | Bearer | Policy check advisory |
| `/integrations/jira` | Bearer | Ingesta de issues Jira |
| `/integrations/jira/status` | Bearer (admin) | Health check Jira |
| `/integrations/jira/correlate` | Bearer (admin) | Correlación batch commit↔ticket |
| `/integrations/jira/ticket-coverage` | Bearer (admin) | Cobertura de tickets |
| `/integrations/jira/tickets/{id}` | Bearer (admin) | Detalle de ticket |

---

## Convenciones de Código

### Rust (Server + Desktop backend)
- Errores: `thiserror`
- Logging: `tracing` (info/debug/warn/error)
- Serde: `#[serde(default)]` en Option y HashMap (CRÍTICO para evitar panics con NULL de PostgreSQL)
- Async: tokio runtime
- SQL: SIEMPRE usar `COALESCE` en `json_object_agg()` y agregaciones que pueden ser NULL
- Tablas de auditoría: append-only (no UPDATE/DELETE)
- Deduplicación: por `event_uuid` único

### TypeScript (Frontend)
- Estado: Zustand stores en `src/store/`
- Tipos: Interfaces en `src/lib/types.ts`
- Componentes: Functional components con hooks
- Estilos: Tailwind classes

### Commits
- Formato: `tipo: descripción` (feat, fix, refactor, docs, test)
- Ejemplo: `feat: add pipeline health widget`

---

## Errores Comunes (NO REPETIR)

### Panic: "invalid type: null, expected a map"
- **Causa:** `json_object_agg()` devuelve NULL sin filas
- **Fix:** `COALESCE(json_object_agg(...), '{}')` + `#[serde(default)]`

### 401 Unauthorized
- **Causa:** Header incorrecto (`X-API-Key` en vez de `Authorization: Bearer`)
- **Fix:** Siempre usar `Authorization: Bearer {key}`

### Serialization error
- **Causa:** Structs no coinciden entre cliente y servidor
- **Fix:** `ServerStats` y `CombinedEvent` deben ser idénticos en ambos lados

---

## Archivos Críticos

| Archivo | Propósito | Precaución |
|---------|-----------|------------|
| `gitgov/gitgov-server/src/handlers.rs` | API handlers | Response structures, integraciones |
| `gitgov/gitgov-server/src/models.rs` | Data structures | Serde attributes, defaults |
| `gitgov/gitgov-server/src/auth.rs` | Middleware auth | Token validation |
| `gitgov/gitgov-server/src/db.rs` | Database queries | COALESCE, append-only |
| `gitgov/gitgov-server/src/main.rs` | Routes, startup | Wiring de rutas y middleware |
| `gitgov/src-tauri/src/outbox/` | Cola de eventos offline | Auth headers, retry logic |
| `gitgov/src-tauri/src/commands/git_commands.rs` | Operaciones Git | Event logging |
| `gitgov/src/store/useControlPlaneStore.ts` | Estado del dashboard | Sync con server structs |
| `gitgov/src/components/control_plane/ServerDashboard.tsx` | Dashboard UI | Widgets, badges |

---

## Variables de Entorno

### Server (`gitgov/gitgov-server/.env`)
- `DATABASE_URL` — PostgreSQL connection string (Supabase pooler)
- `SUPABASE_URL` / `SUPABASE_ANON_KEY` / `SUPABASE_SERVICE_KEY`
- `GITGOV_JWT_SECRET` — JWT signing
- `GITGOV_SERVER_ADDR` — `0.0.0.0:3000`
- `GITGOV_API_KEY` — API key para desktop clients
- `GITHUB_WEBHOOK_SECRET` — Validación HMAC de webhooks GitHub
- `GITHUB_PERSONAL_ACCESS_TOKEN` — Token para GitHub MCP
- `RUST_LOG` — Log level (`gitgov_server=info,tower_http=info`)

### Desktop (`gitgov/.env`)
- `VITE_SERVER_URL=http://localhost:3000`
- `VITE_API_KEY` — API key del servidor

---

## Estado del Proyecto (Feb 2026)

### Funcional
- Pipeline E2E: Desktop → Server → PostgreSQL → Dashboard
- GitHub OAuth + API Keys
- Outbox offline con reintentos
- V1.2-A (Jenkins MVP): ingesta, correlación, widget Pipeline Health, policy/check advisory
- V1.2-B Preview (Jira): ingesta, correlación batch, ticket coverage, badges

### Pendiente (Alta Prioridad)
- Endurecer pruebas E2E reales Jenkins + Jira
- Correlación de `related_prs` automática
- Tests automatizados backend (cobertura)

### Roadmap
- V1.2-A: Jenkins-first MVP (funcional)
- V1.2-B: Jira + Ticket Coverage (preview)
- V1.2-C: Correlation Engine V2 + Compliance Signals (pendiente)
- V1.3: AI Governance Insights (futuro)

---

## Repo GitHub

- **Organización:** MapfrePE
- **Repo:** GitGov
- **Branch principal:** main
- **URL:** https://github.com/MapfrePE/GitGov

---

## Seguridad

1. Tokens en keyring — NUNCA en archivos
2. API keys hasheadas — SHA256 antes de guardar en DB
3. HTTPS obligatorio en producción
4. Append-only — Eventos de auditoría no se modifican
5. Deduplicación — `event_uuid` único
6. No exponer secretos en logs de error
7. `.env` NUNCA debe commitearse (verificar `.gitignore`)

---

## Reglas para Agentes

1. **Leer antes de modificar** — No proponer cambios a código que no hayas leído
2. **Golden Path primero** — Validar que el flujo base sigue funcionando
3. **Linting antes de commit** — `cargo clippy` + `npm run lint`
4. **No romper structs compartidas** — `ServerStats`, `CombinedEvent` deben coincidir en frontend y backend
5. **Append-only** — No intentar UPDATE/DELETE en tablas de auditoría
6. **COALESCE siempre** — En cualquier SQL con agregaciones
7. **Bearer, no X-API-Key** — Para autenticación
8. **Documentar cambios** — Actualizar `docs/PROGRESS.md` con cambios significativos
