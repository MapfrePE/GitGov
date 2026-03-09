# CLAUDE.md — Instrucciones para Claude Code en GitGov

> Este archivo se carga automáticamente en cada sesión. Léelo completo antes de hacer cualquier cambio.

## Guardrail Ejecutivo (leer primero)

1. **No inventar:** toda afirmación técnica requiere evidencia `archivo:línea`.
2. **Si no se pudo verificar:** responder `NO VERIFICADO:` + bloqueadores concretos.
3. **Golden Path no negociable:** commit/push/events/dashboard sin 401 deben seguir funcionando.
4. **Auth obligatoria:** `Authorization: Bearer` (nunca `X-API-Key`).
5. **SQL seguro:** tablas de auditoría append-only + `COALESCE` en agregaciones JSON.
6. **Structs compartidas:** no romper contrato entre backend, Tauri y frontend.
7. **Lint/testing mínimo:** `cargo test` + `tsc -b` + `0 errores nuevos` en archivos tocados.
8. **No secretos:** nunca pegar tokens/keys/secrets en chat, logs o commits.
9. **Anti split-brain local:** server local en `127.0.0.1:3000`; Docker server en `127.0.0.1:3001`.
10. **Documentar cambios relevantes:** actualizar `docs/PROGRESS.md`.

---

## Qué es GitGov

Sistema de gobernanza de Git distribuido con cuatro componentes:
1. **Desktop App** — Tauri v2 + React 19 + Tailwind v4 + Zustand v5 (en `gitgov/`)
2. **Control Plane Server** — Axum + Rust (en `gitgov/gitgov-server/`)
3. **GitHub/Jenkins/Jira Integrations** — Webhooks + OAuth
4. **Web App (marketing/docs)** — Next.js 14 + React 18 + Tailwind v3 (en `gitgov-web/`), desplegada en Vercel (`https://git-gov.vercel.app`)

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

# Web App (marketing/docs)
cd gitgov-web && pnpm dev     # o npm run dev

# Tests unitarios server (sin DB, sin server — también corre en CI)
cd gitgov/gitgov-server && cargo test
# o con Makefile:
cd gitgov/gitgov-server && make test

# Smoke / contrato live (requiere server corriendo)
cd gitgov/gitgov-server && make smoke

# Tests E2E completos
cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh

# Jenkins integration test
cd gitgov/gitgov-server/tests && API_KEY="<YOUR_API_KEY>" ./jenkins_integration_test.sh

# Jira integration test
cd gitgov/gitgov-server/tests && API_KEY="<YOUR_API_KEY>" ./jira_integration_test.sh

# Tests unitarios Desktop (vitest)
cd gitgov && npm test
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

> Si hay deuda histórica de lint en el repo, la regla de aceptación es: **0 errores nuevos en archivos tocados** (ejecutar ESLint sobre esos archivos), además de `npm run typecheck`.

---

## Arquitectura de Autenticación

### Desktop → Control Plane
- Header: `Authorization: Bearer {api_key}` (NUNCA `X-API-Key`)
- Server calcula SHA256 del token y busca en tabla `api_keys` por `key_hash`

### Roles
- **Admin:** Acceso total (stats, dashboard, integrations)
- **Architect:** (reservado para futuras restricciones)
- **Developer:** Solo sus propios eventos
- **PM:** (reservado para futuras restricciones)

> El enum `UserRole` tiene 4 variantes: `Admin`, `Architect`, `Developer`, `PM`. `from_str` desconocido → `Developer`.

### Jenkins / Jira Webhook Secrets (opcionales)
- Jenkins: header `x-gitgov-jenkins-secret` con valor de `JENKINS_WEBHOOK_SECRET`
- Jira: header `x-gitgov-jira-secret` con valor de `JIRA_WEBHOOK_SECRET`
- Si no se configuran, el endpoint acepta cualquier request autenticado con Bearer admin key.

### GitHub Webhooks
- Validación HMAC con `GITHUB_WEBHOOK_SECRET`

---

## Endpoints del Servidor

| Endpoint | Método | Auth | Propósito |
|----------|--------|------|-----------|
| **Públicos** | | | |
| `/health` | GET | None | Health check básico |
| `/health/detailed` | GET | None | Health check con latencia DB y uptime |
| `/webhooks/github` | POST | HMAC | Webhooks de GitHub (push, create) |
| `/org-invitations/preview/{token}` | GET | None | Preview de invitación (público) |
| `/org-invitations/accept` | POST | None | Aceptar invitación con token |
| **Ingesta** | | | |
| `/events` | POST | Bearer | Ingesta batch de eventos del cliente |
| `/audit-stream/github` | POST | Bearer (admin) | Ingestar GitHub audit log stream |
| `/outbox/lease` | POST | Bearer | Lease server-driven para coordinación outbox |
| `/outbox/lease/metrics` | GET | Bearer (admin) | Telemetría de leases outbox |
| **Consulta** | | | |
| `/logs` | GET | Bearer | Eventos combinados (dev: solo propios, admin: todos) |
| `/stats` | GET | Bearer (admin) | Estadísticas globales + pipeline 7d |
| `/stats/daily` | GET | Bearer (admin) | Actividad diaria (chart) |
| `/dashboard` | GET | Bearer (admin) | Datos del dashboard |
| `/me` | GET | Bearer | Info del usuario autenticado |
| `/clients` | GET | Bearer (admin) | Sesiones de clientes conectados |
| `/governance-events` | GET | Bearer | Eventos de governance (branch protection, etc.) |
| `/pr-merges` | GET | Bearer (admin) | PR merges recientes |
| `/admin-audit-log` | GET | Bearer (admin) | Log de acciones admin |
| `/sse` | GET | Bearer | SSE stream real-time (max 50 conexiones, heartbeat 30s) |
| **Team / Org** | | | |
| `/team/overview` | GET | Bearer (admin) | Overview del equipo |
| `/team/repos` | GET | Bearer (admin) | Repos del equipo |
| `/orgs` | POST | Bearer (admin) | Crear organización |
| `/org-users` | GET/POST | Bearer (admin) | Listar / crear usuarios de org |
| `/org-users/{id}/status` | PATCH | Bearer (admin) | Activar/desactivar usuario |
| `/org-users/{id}/api-key` | POST | Bearer (admin) | Crear API key para usuario |
| `/org-invitations` | GET/POST | Bearer (admin) | Listar / crear invitaciones |
| `/org-invitations/{id}/resend` | POST | Bearer (admin) | Reenviar invitación |
| `/org-invitations/{id}/revoke` | POST | Bearer (admin) | Revocar invitación |
| `/api-keys` | GET/POST | Bearer (admin) | Listar / crear API keys |
| `/api-keys/{id}/revoke` | POST | Bearer (admin) | Revocar API key |
| **Integraciones** | | | |
| `/integrations/jenkins` | POST | Bearer (admin) | Pipeline events de Jenkins |
| `/integrations/jenkins/status` | GET | Bearer (admin) | Health check Jenkins |
| `/integrations/jenkins/correlations` | GET | Bearer (admin) | Correlaciones commit↔pipeline |
| `/integrations/jira` | POST | Bearer (admin) | Ingesta webhook de Jira |
| `/integrations/jira/status` | GET | Bearer (admin) | Health check Jira |
| `/integrations/jira/correlate` | POST | Bearer (admin) | Correlación batch commit↔ticket |
| `/integrations/jira/ticket-coverage` | GET | Bearer (admin) | Cobertura de tickets |
| `/integrations/jira/tickets/{id}` | GET | Bearer (admin) | Detalle de ticket |
| **Compliance / Signals / Policy** | | | |
| `/compliance/{org_name}` | GET | Bearer (admin) | Dashboard de compliance |
| `/signals` | GET | Bearer | Noncompliance signals |
| `/signals/{signal_id}` | POST | Bearer | Actualizar signal |
| `/signals/{signal_id}/confirm` | POST | Bearer (admin) | Confirmar signal |
| `/signals/detect/{org_name}` | POST | Bearer (admin) | Disparar detección de signals |
| `/violations/{id}/decisions` | GET | Bearer | Historial de decisiones |
| `/violations/{id}/decisions` | POST | Bearer (admin) | Añadir decisión a violación |
| `/policy/{repo_name}` | GET | Bearer | Obtener política del repo |
| `/policy/check` | POST | Bearer (admin) | Policy check advisory |
| `/policy/{repo_name}/history` | GET | Bearer | Historial de política |
| `/policy/{repo_name}/override` | PUT | Bearer (admin) | Override de política |
| **Jobs / Export / Chat / GDPR** | | | |
| `/jobs/metrics` | GET | Bearer (admin) | Métricas del job queue |
| `/jobs/dead` | GET | Bearer (admin) | Lista de jobs muertos |
| `/jobs/{job_id}/retry` | POST | Bearer (admin) | Reintentar job muerto |
| `/export` | POST | Bearer | Exportar eventos |
| `/exports` | GET | Bearer | Listar exports anteriores |
| `/chat/ask` | POST | Bearer | Bot conversacional Gemini |
| `/feature-requests` | POST | Bearer | Crear feature request |
| `/identities/aliases` | GET/POST | Bearer (admin) | Alias de identidad (merge logins) |
| `/users/{login}/erase` | POST | Bearer (admin) | GDPR: borrar datos de usuario |
| `/users/{login}/export` | GET | Bearer (admin) | GDPR: exportar datos de usuario |

> **Nota:** `/logs` aplica scoping: `Admin` ve todos, `Developer` solo ve sus propios eventos (filtrado por `user_login = client_id`).
> **Nota SSE:** `/sse` usa broadcast channel con generation counter para cancelación limpia. Protegido por `Arc<Semaphore>` (max 50 conexiones, configurable via `GITGOV_SSE_MAX_CONNECTIONS`) + rate limit en intentos de conexión. Heartbeat cada 30s. Frontend cae a polling si SSE se desconecta.

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

### TypeScript (Desktop Frontend — `gitgov/src/`)
- Estado: Zustand v5 stores en `src/store/`
- Tipos: Interfaces en `src/lib/types.ts`
- Componentes: Functional components con hooks
- Estilos: Tailwind v4 classes
- Router: react-router-dom v7
- Tests: vitest + @testing-library/react

### TypeScript (Web App — `gitgov-web/`)
- Framework: Next.js 14 App Router
- Estilos: Tailwind v3
- i18n: EN/ES con `lib/i18n/translations.ts` + context provider
- Docs: markdown con gray-matter + remark (en `app/docs/`)
- Analytics: `lib/analytics/index.ts`

### Commits
- Formato: `tipo: descripción` (feat, fix, refactor, docs, test)
- Ejemplo: `feat: add pipeline health widget`

---

## Errores Comunes (NO REPETIR)

### Failed to deserialize query string: missing field `offset`
- **Causa:** Los structs de query (`EventFilter`, `JenkinsCorrelationFilter`, `SignalFilter`, `GovernanceEventFilter`) tenían `offset: usize` como campo requerido. Si el cliente no lo mandaba, serde fallaba.
- **Fix aplicado (Feb 2026):** `#[serde(default)]` en los campos `limit` y `offset` de los 4 structs. `usize::default() = 0`. Los handlers ya manejaban `0` con fallbacks. Backward compatible.
- **Regla:** Al añadir nuevos campos de paginación en query structs, usar siempre `#[serde(default)]` o `Option<T>`.

### JWT_SECRET inseguro en producción
- **Causa:** `GITGOV_JWT_SECRET` tiene un default hardcodeado: `"gitgov-secret-key-change-in-production"`
- **Fix:** SIEMPRE establecer un secreto fuerte y único en producción (`openssl rand -hex 32`)
- **Riesgo:** Si no se cambia, cualquiera puede forjar tokens JWT

### Panic: "invalid type: null, expected a map"
- **Causa:** `json_object_agg()` devuelve NULL sin filas
- **Fix:** `COALESCE(json_object_agg(...), '{}')` + `#[serde(default)]`

### 401 Unauthorized
- **Causa:** Header incorrecto (`X-API-Key` en vez de `Authorization: Bearer`)
- **Fix:** Siempre usar `Authorization: Bearer {key}`

### Serialization error
- **Causa:** Structs no coinciden entre cliente y servidor
- **Fix:** `AuditStats` y `CombinedEvent` deben ser idénticos en ambos lados
- Las tres copias que deben sincronizarse: `models.rs` (server), `control_plane/server.rs` (Tauri), `useControlPlaneStore.ts` (frontend)
- Governance structs también: `GitGovConfig`, `EnforcementConfig`, `RulesConfig` en `models.rs` ↔ `branch_rule.rs` ↔ `types.ts`

### localhost ≠ 127.0.0.1 (split-brain local)
- **Síntoma:** Desktop envía eventos pero el dashboard no los muestra
- **Causa:** `localhost` puede resolver a IPv6 (`::1`), pegando a un proceso diferente
- **Fix:** Usar `127.0.0.1` canónico en toda la configuración. El código ya normaliza `localhost→127.0.0.1` en 4 lugares (server.rs, server_commands.rs, lib.rs, useControlPlaneStore.ts)

### Race condition en cancelación SSE (AtomicBool)
- **Síntoma:** Streams SSE "huérfanos" que duplican notificaciones
- **Causa:** Usar `AtomicBool` global que se resetea a `false` al reconectar; stream viejo no ve el `true` a tiempo
- **Fix aplicado (Mar 2026):** Reemplazado con `AtomicU64` generation counter. Cada connect/disconnect incrementa. El stream solo continúa si su generation local == counter actual.
- **Regla:** NUNCA usar bool global para cancelar operaciones concurrentes. Siempre usar generation counter o CancellationToken con identidad.

### Listeners/timers sin cleanup
- **Síntoma:** Memory leaks, reconexiones fantasma, duplicación de fetches
- **Causa:** `tauriListen()` registrado antes de `tauriInvoke()`, pero si invoke falla, listeners quedan colgados. `setTimeout` para reconnect no se cancela en `disconnect`.
- **Fix aplicado (Mar 2026):** `.catch()` de invoke limpia listeners. Timer almacenado en `sseReconnectTimer` y cancelado en `disconnectSse()`.
- **Regla:** Todo listener/timer DEBE tener cleanup path tanto en éxito como en error.

---

## Archivos Críticos

| Archivo | Propósito | Precaución |
|---------|-----------|------------|
| **Server** | | |
| `gitgov-server/src/handlers.rs` | API handlers (include de 17+ archivos) | Response structures, integraciones |
| `gitgov-server/src/handlers/sse.rs` | SSE endpoint | Broadcast stream, semaphore 50 max, heartbeat 30s |
| `gitgov-server/src/models.rs` | Structs canónicos (`AuditStats`, `CombinedEvent`) | Serde attributes, defaults — 3 copias deben coincidir |
| `gitgov-server/src/auth.rs` | Middleware auth | Token validation, fail-closed en auth stale |
| `gitgov-server/src/db.rs` | Database queries | COALESCE, append-only, pool config |
| `gitgov-server/src/main.rs` | Routes, startup, rate limits | Wiring de rutas, middleware, env vars |
| `gitgov-server/supabase_schema_v6.sql` | Schema actual (v6) | V1-V6 son incrementales |
| **Desktop (Tauri)** | | |
| `src-tauri/src/outbox/queue.rs` | Cola de eventos offline | Retry diferenciado, jitter, lease server-driven, coord global |
| `src-tauri/src/control_plane/server.rs` | Cliente HTTP al server | Structs deben coincidir con models.rs |
| `src-tauri/src/commands/git_commands.rs` | Operaciones Git + governance check | Event logging, trunca a 500 files, pre-push enforcement |
| `src-tauri/src/commands/server_commands.rs` | Tauri commands del server | SSE connect con generation counter, 67 comandos registrados |
| `src-tauri/src/models/branch_rule.rs` | GitGovConfig, EnforcementLevel, RulesConfig | Contrato compartido: server ↔ Tauri ↔ frontend |
| `src-tauri/src/lib.rs` | Wiring Tauri: plugins, managed state, heartbeat | Outbox, SSE, notification plugin |
| `src-tauri/tauri.conf.json` | Config Tauri: CSP, window, plugins | CSP restringe scripts/conexiones |
| `src-tauri/capabilities/default.json` | Permisos Tauri v2 | notification, dialog, updater, shell |
| **Frontend** | | |
| `src/store/useControlPlaneStore.ts` | Estado del dashboard | Sync con server structs, SSE lifecycle, notification dispatch |
| `src/store/useRepoStore.ts` | Estado del repo local | Push con notification en bloqueo |
| `src/store/useAuthStore.ts` | Auth GitHub OAuth + PIN local | Tokens, sesión, keyring |
| `src/store/useUpdateStore.ts` | Auto-update desktop | Canales stable/staging |
| `src/components/control_plane/ServerDashboard.tsx` | Dashboard UI principal | SSE connect/disconnect, polling fallback |
| `src/components/control_plane/GovernanceRulesPanel.tsx` | UI reglas de gobernanza | Presets, enforcement levels, policy override |
| `src/components/control_plane/PolicyEditorPanel.tsx` | Editor de políticas interactivo | Serialización debe coincidir con GitGovConfig |
| `src/components/control_plane/RecentCommitsTable.tsx` | Tabla commits | Correlación CI badge, ticket badges |
| `src/components/control_plane/dashboard-helpers.ts` | Lógica UI dashboard | buildDashboardRows, extractTicketIds |
| `src/lib/tauri.ts` | Bridge Tauri: invoke, listen, parseError | `tauriListen()` devuelve UnlistenFn — cleanup crítico |
| `src/lib/notifications.ts` | Notificaciones nativas OS | Cooldown 60s, permisos, prefs en localStorage |
| `src/lib/types.ts` | Interfaces TypeScript | GitGovConfig, EnforcementConfig, PolicyCheckResponse, etc. |
| **Web App** | | |
| `gitgov-web/lib/config/site.ts` | Config web app | URL Vercel, versión, download path |
| `gitgov-web/lib/i18n/translations.ts` | Traducciones EN/ES | Agregar clave en ambos idiomas |

---

## Variables de Entorno

### Server (`gitgov/gitgov-server/.env`)

**Core:**
- `DATABASE_URL` — PostgreSQL connection string (Supabase pooler)
- `SUPABASE_URL` / `SUPABASE_ANON_KEY` / `SUPABASE_SERVICE_KEY`
- `GITGOV_JWT_SECRET` — JWT signing (CAMBIAR en prod: `openssl rand -hex 32`)
- `GITGOV_ALLOW_INSECURE_JWT_FALLBACK` — default `true` en dev, `false` en prod
- `GITGOV_SERVER_ADDR` — `0.0.0.0:3000`
- `GITGOV_API_KEY` — API key para desktop clients (se inserta en DB al arrancar si no existe)
- `GITGOV_ENV` — `development` | `staging` | `production` (default: `development` en debug, `production` en release)
- `RUST_LOG` — Log level (`gitgov_server=info,tower_http=info`)

**Secrets e integraciones:**
- `GITHUB_WEBHOOK_SECRET` — Validación HMAC de webhooks GitHub
- `GITHUB_PERSONAL_ACCESS_TOKEN` — Token para GitHub API
- `JENKINS_WEBHOOK_SECRET` — Secret para Jenkins (header: `x-gitgov-jenkins-secret`, opcional)
- `JIRA_WEBHOOK_SECRET` — Secret para Jira (header: `x-gitgov-jira-secret`, opcional)
- `GEMINI_API_KEY` — API key para bot conversacional Gemini
- `GEMINI_MODEL` — Modelo Gemini (default: `gemini-2.5-flash`)
- `FEATURE_REQUEST_WEBHOOK_URL` — Webhook para feature requests (opcional)
- `GITGOV_ALERT_WEBHOOK_URL` — Webhook para alertas operativas (opcional)

**Seguridad / hardening:**
- `GITGOV_STRICT_ACTOR_MATCH` — default `true` — valida actor match en eventos
- `GITGOV_REJECT_SYNTHETIC_LOGINS` — default `false` — rechaza logins sintéticos
- `GITGOV_CORS_ALLOW_ORIGINS` — origins permitidos (default: `""`, dev: `GITGOV_CORS_ALLOW_ANY=true`)

**DB pool:**
- `GITGOV_DB_MAX_CONNECTIONS` — default `10` (recomendado `60` en prod)
- `GITGOV_DB_MIN_CONNECTIONS` — default `1`
- `GITGOV_DB_MAX_LIFETIME_SECS` — TTL de conexiones

**Rate limiting:**
- `GITGOV_RATE_LIMIT_EVENTS_PER_MIN` — default `240` (`/events`)
- `GITGOV_RATE_LIMIT_AUDIT_STREAM_PER_MIN` — default `60` (`/audit-stream/github`)
- `GITGOV_RATE_LIMIT_JENKINS_PER_MIN` — default `120` (`/integrations/jenkins`)
- `GITGOV_RATE_LIMIT_JIRA_PER_MIN` — default `120` (`/integrations/jira`)
- `GITGOV_RATE_LIMIT_ADMIN_PER_MIN` — default `60` (stats, dashboard, admin)
- `GITGOV_RATE_LIMIT_LOGS_PER_MIN` — default = `ADMIN_PER_MIN` (`/logs`)
- `GITGOV_RATE_LIMIT_STATS_PER_MIN` — default = `ADMIN_PER_MIN` (`/stats`)
- `GITGOV_RATE_LIMIT_CHAT_PER_MIN` — default `40` (`/chat/ask`)
- `GITGOV_RATE_LIMIT_DISTRIBUTED_DB` — default `false` — rate limiting distribuido por DB
- `GITGOV_RATE_LIMIT_DISTRIBUTED_PRUNE_INTERVAL_SECS` — default `300`
- `GITGOV_RATE_LIMIT_DISTRIBUTED_RETENTION_SECS` — default `3600`

**Body limits:**
- `GITGOV_EVENTS_MAX_BODY_BYTES` — default `2097152` (2 MB)
- `GITGOV_EVENTS_MAX_BATCH` — default `1000` eventos por request
- `GITGOV_JENKINS_MAX_BODY_BYTES` — default `262144` (256 KB)
- `GITGOV_JIRA_MAX_BODY_BYTES` — default `524288` (512 KB)

**Cache:**
- `GITGOV_STATS_CACHE_TTL_MS` — default `3000` (3s)
- `GITGOV_LOGS_CACHE_TTL_MS` — default `800` (0.8s)
- `GITGOV_LOGS_CACHE_STALE_ON_ERROR_MS` — default `5000` (grace window)
- `GITGOV_LOGS_REJECT_OFFSET_PAGINATION` — default `false` (deprecación hard de offset)

**SSE:**
- `GITGOV_SSE_MAX_CONNECTIONS` — default `50` — máx conexiones SSE concurrentes

**Chat/LLM:**
- `GITGOV_CHAT_LLM_MAX_CONCURRENCY` — default `4` — máx llamadas LLM simultáneas
- `GITGOV_CHAT_LLM_QUEUE_TIMEOUT_MS` — default `500` — timeout en cola
- `GITGOV_CHAT_LLM_TIMEOUT_MS` — default `9000` — timeout por llamada

**Outbox lease (server-side, opt-in):**
- `GITGOV_OUTBOX_SERVER_LEASE_ENABLED` — default `false`
- `GITGOV_OUTBOX_SERVER_LEASE_TTL_MS` — default `2000`

**Job Worker (hardcodeado en main.rs):**
- `JOB_WORKER_TTL_SECS = 300` — TTL antes de considerar job estancado
- `JOB_POLL_INTERVAL_SECS = 5` — Frecuencia de polling de jobs
- `JOB_ERROR_BACKOFF_SECS = 10` — Backoff base (exponencial, máx ×32)

### Desktop (`gitgov/.env` y `gitgov/src-tauri/.env`)

**Server connection:**
- `VITE_SERVER_URL=http://localhost:3000` — URL del server para el frontend (Vite)
- `VITE_API_KEY` — API key del servidor para el frontend
- `GITGOV_SERVER_URL` — URL del server para el backend Tauri (leído en lib.rs al arrancar)
- `GITGOV_API_KEY` — API key del servidor para el backend Tauri

**Outbox coordinación (client-side, opt-in):**
- `GITGOV_OUTBOX_GLOBAL_COORD_ENABLED` — default `false`
- `GITGOV_OUTBOX_GLOBAL_COORD_WINDOW_MS` — default `20000`
- `GITGOV_OUTBOX_GLOBAL_COORD_MAX_DEFERRAL_MS` — default `1600`
- `GITGOV_OUTBOX_SERVER_LEASE_ENABLED` — default `false`
- `GITGOV_OUTBOX_SERVER_LEASE_TTL_MS` — default `2000`
- `GITGOV_OUTBOX_FLUSH_JITTER_MAX_MS` — jitter aleatorio por instancia

**Keyring / security:**
- `GITGOV_ALLOW_LEGACY_TOKEN_FILE` — default `false` — permite fallback a archivo
- `GITGOV_LEGACY_TOKEN_DIR` — directorio legacy de tokens

> **IMPORTANTE:** Existen DOS fuentes de config del server URL: `VITE_SERVER_URL` (frontend React) y `GITGOV_SERVER_URL` (Tauri Rust). El store `useControlPlaneStore` tiene prioridad: input > previous > env > localStorage > default `127.0.0.1:3000`.

### Web App (`gitgov-web/.env.local`) — producción en Vercel
- No requiere variables de entorno especiales actualmente
- URL canónica: `https://git-gov.vercel.app`

---

## Estado del Proyecto (Mar 2026)

### Funcional
- Pipeline E2E: Desktop → Server → PostgreSQL → Dashboard
- GitHub OAuth + API Keys + PIN local
- Outbox offline con reintentos (retry diferenciado, jitter, lease server-driven, coordinación global, tuning operativo)
- V1.2-A (Jenkins MVP): ingesta, correlación, widget Pipeline Health, policy/check advisory
- V1.2-B Preview (Jira): ingesta, correlación batch, ticket coverage, badges
- Multi-tenant: ~80% implementado (org_id scoping en DB, auth, handlers, invitaciones)
- Org Management: crear org, invitar usuarios, API key por usuario, activar/desactivar
- Governance Rules Engine: enforcement Off/Warn/Block, presets, pre-push check, UI en Settings
- Policy Editor: modal interactivo para editar GitGovConfig, presets (startup/enterprise/regulated)
- SSE (Server-Sent Events): dashboard real-time con fallback a polling, generation-based cancelación, semaphore 50 max
- Notificaciones nativas: `tauri-plugin-notification` con cooldown 60s, prefs en Settings, push bloqueado
- Bot conversacional: Gemini-powered con 11+ query types, semaphore LLM, feature requests
- Auto-update: canales stable/staging con firma, verificación, UI completa
- CSP en webview: Content Security Policy configurado (previene XSS)
- GDPR: endpoints `/users/{login}/erase` y `/users/{login}/export`
- Identity aliases: merge de logins (`/identities/aliases`)
- Export: exportar eventos con audit trail (`/export`, `/exports`)

### Frontend — Componentes principales
- **Dashboard:** ServerDashboard, MetricsGrid, DailyActivityWidget, EventBreakdownGrid, PipelineHealthWidget, TicketCoverageWidget, RecentCommitsTable, DashboardHeader, Bar
- **Admin:** AdminOnboardingPanel, TeamManagementPanel, ApiKeyManagerWidget, DeveloperAccessPanel, ExportPanel
- **Governance:** GovernanceRulesPanel, PolicyEditorPanel
- **Chat:** ConversationalChatPanel
- **Config:** ServerConfigPanel, MaintenanceOverlay
- **Stores:** useAuthStore, useRepoStore, useControlPlaneStore, useAuditStore, useUpdateStore
- **Lib:** tauri.ts, types.ts, notifications.ts, timezone.ts, constants.ts, largeChangeset.ts, updater.ts

### Tests (estado actual — Mar 2026)
- `cargo test` (server) — **99 unit tests** (models, handlers, auth, NLP, compliance) — **funcional**
- `cargo test` (desktop) — **17 unit tests** — **funcional**
- `vitest` (frontend) — **1 smoke test** (sin tests de componentes ni stores) — **gap conocido**
- `smoke_contract.sh` — 14 contract checks live (paginación + Golden Path) — **funcional, local**
- E2E scripts (`e2e_flow_test.sh`, `jenkins_integration_test.sh`, `jira_integration_test.sh`) — **manuales**
- Tests de integración con DB mock — **pendiente**

### Pendiente (Alta Prioridad)
- Correlación de `related_prs` automática
- HTTPS en EC2 (dominio + Let's Encrypt)
- Tests de integración backend con DB mock (sin Supabase real)
- `notifyGovernanceWarning()` implementado pero sin caller (requiere cambio de contrato `cmd_push`)

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

1. Tokens en keyring — NUNCA en archivos (fallback legacy con `GITGOV_ALLOW_LEGACY_TOKEN_FILE`)
2. API keys hasheadas — SHA256 antes de guardar en DB
3. HTTPS obligatorio en producción
4. Append-only — Eventos de auditoría no se modifican
5. Deduplicación — `event_uuid` único
6. No exponer secretos en logs de error
7. `.env` NUNCA debe commitearse (verificar `.gitignore`)
8. CSP en webview — `tauri.conf.json` restringe `script-src`, `connect-src`, etc.
9. Rate limiting — todas las rutas tienen rate limiter (incluido `/sse` con semaphore)
10. GDPR — endpoints de borrado y exportación de datos de usuario

---

## Reglas para Agentes

### Fundamentales (siempre)
1. **Leer antes de modificar** — No proponer cambios a código que no hayas leído
2. **Golden Path primero** — Validar que el flujo base sigue funcionando
3. **Linting antes de commit** — `cargo clippy` + `npm run typecheck` + `0 errores nuevos` en ESLint de archivos tocados
4. **No romper structs compartidas** — `AuditStats`, `CombinedEvent` deben coincidir en frontend y backend
5. **Append-only** — No intentar UPDATE/DELETE en tablas de auditoría
6. **COALESCE siempre** — En cualquier SQL con agregaciones
7. **Bearer, no X-API-Key** — Para autenticación
8. **Documentar cambios** — Actualizar `docs/PROGRESS.md` con cambios significativos
9. **No inventar** — Si no se pudo verificar, responder `NO VERIFICADO` y listar exactamente qué falta para verificar
10. **No exponer secretos** — Nunca pegar tokens/API keys/secrets en chat, logs o commits
11. **Anti split-brain local** — Usar canónico `127.0.0.1:3000` para server local; Docker server en `127.0.0.1:3001`

### Anti-pattern: recomendar sin verificar (aprendido en sesión Mar 2026)
12. **Verificar antes de recomendar** — ANTES de sugerir "falta X", buscar en el codebase si X ya existe. Usar Grep/Glob/Read, no asumir. Errores pasados:
    - Sugerí "construir multi-tenant" cuando ya estaba ~80% implementado
    - Sugerí "construir policy editor" cuando el backend ya existía completo
    - Dije "36 tests" cuando eran 99 — siempre ejecutar `cargo test` para contar
13. **Self-review antes de declarar "listo"** — Después de implementar, re-leer el código propio buscando:
    - **Lifecycle leaks:** ¿se limpian todos los recursos? (listeners, timers, conexiones)
    - **Race conditions:** ¿qué pasa si se llama connect→disconnect→connect rápido?
    - **Duplicación de efectos:** ¿un evento dispara múltiples fetches idénticos?
    - **Contrato SSE/parser:** ¿el parser maneja todas las variantes del protocolo?
    - **Cancelación:** ¿se puede detener limpiamente cualquier operación long-running?
14. **No usar flags booleanos globales para cancelación** — Usar generation counters (`AtomicU64`) o tokens de cancelación con identidad. Un bool global tiene race condition si se resetea al reconectar.
15. **Debounce eventos rápidos** — Si un evento del servidor puede llegar en ráfaga (SSE, webhooks), agregar debounce (200ms+) antes de disparar fetches al servidor.
16. **Timers siempre cancelables** — Todo `setTimeout`/`setInterval` debe almacenarse en variable y limpiarse en la función de cleanup correspondiente.

### Anti-pattern: implementación incompleta
17. **Checklist de lifecycle para features con conexión persistente:**
    - [ ] ¿`connect()` invalida conexiones anteriores? (generation counter)
    - [ ] ¿`disconnect()` cierra el recurso real? (no solo listeners)
    - [ ] ¿Reconexión automática tiene timer cancelable?
    - [ ] ¿Fallo en connect limpia listeners registrados antes del fallo?
    - [ ] ¿El server tiene rate limit / max connections para el endpoint?
    - [ ] ¿Múltiples clientes simultáneos no causan problemas?

---

## Modo Auditor — Obligatorio para respuestas técnicas

Toda afirmación técnica sobre el codebase DEBE seguir este formato antes de ser aceptada:

```
Respuesta: <afirmación concreta>
Evidencia en código: <archivo>:<línea>, <archivo>:<línea>
Nivel de certeza: Alto (leído en esta sesión) | Medio (leído en sesión anterior) | Bajo (inferencia)
Supuestos: <qué se asume si los hay>
Riesgo si estoy equivocado: <consecuencia del error>
```

**Regla absoluta:** Si no hay `archivo:línea`, la afirmación no se hace.
**Regla absoluta:** Si es inferencia, debe decir "INFERENCIA:" explícitamente antes de la afirmación.
**Regla absoluta:** Si no se pudo validar, usar `NO VERIFICADO:` y detallar bloqueadores.

Ejemplos de lo que NO se acepta:
- "El outbox usa SQLite" → sin File:Line = no se dice
- "El enum incluye PullRequest" → sin File:Line = no se dice
- "Los diffs se envían al servidor" → sin File:Line = no se dice

---

## Modo Implementación — Checklist obligatorio

Antes de escribir cualquier línea de código:

**1. Archivos leídos (listar todos antes de empezar):**
- [ ] Archivo a modificar — leído con Read tool en esta sesión
- [ ] Archivos dependientes relevantes — leídos

**2. Cambios realizados (listar al terminar):**
- `archivo:línea_inicio-línea_fin` — descripción del cambio

**3. Validación ejecutada (comando + resultado real):**
- `cargo test` → `X passed; 0 failed` (pegar resultado real)
- `tsc -b` → sin errores (pegar resultado real)
- `npx eslint <archivos_tocados>` → errores nuevos introducidos: 0
- Si `npm run lint` global falla por deuda histórica: reportar explícitamente que es preexistente y no causada por el cambio

**4. Impacto en Golden Path:**
- ¿Modifica auth/token/API key/handlers/dashboard? → Sí/No
- Si Sí: evidencia de que el flujo Desktop→/events→PostgreSQL→Dashboard sigue intacto

**5. Si no se pudo validar algo:**
- Responder `NO VERIFICADO: <qué no se validó>`
- Especificar comando faltante, entorno faltante y cómo reproducirlo

---

## Validación empírica del Golden Path

Tras cualquier cambio en los archivos críticos listados abajo, ejecutar:

```bash
# 1. Compilar y testear server
cd gitgov/gitgov-server && cargo test

# 2. Verificar que /events acepta eventos con Bearer auth
curl -X POST http://127.0.0.1:3000/events \
  -H "Authorization: Bearer {api_key}" \
  -H "Content-Type: application/json" \
  -d '{"events":[{"event_uuid":"00000000-0000-0000-0000-000000000001","event_type":"commit","user_login":"test","files":[],"status":"success","timestamp":0}],"client_version":"manual-check"}'
# Esperar shape: {"accepted":["..."],"duplicates":[],"errors":[]}

# 3. Verificar que /stats responde sin 401
curl http://127.0.0.1:3000/stats \
  -H "Authorization: Bearer {api_key}"
# Esperar: JSON con AuditStats (no {"error":"..."})

# 4. Validar contrato /logs (sin romper compatibilidad)
curl "http://127.0.0.1:3000/logs?limit=5&offset=0" \
  -H "Authorization: Bearer {api_key}"
# Esperar: {"events":[...]} (sin error de deserialización)

# 5. Smoke contractual recomendado (live)
cd gitgov/gitgov-server && make smoke

# 6. E2E Golden Path recomendado
cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh
```

Archivos cuyo cambio OBLIGA a ejecutar esta validación:
- `gitgov-server/src/auth.rs`
- `gitgov-server/src/handlers.rs`
- `gitgov-server/src/main.rs`
- `gitgov-server/src/models.rs`
- `src-tauri/src/outbox/queue.rs`
- `src-tauri/src/control_plane/server.rs`
- `src/store/useControlPlaneStore.ts`
