# CLAUDE.md — Instrucciones para Claude Code en GitGov

> Este archivo se carga automáticamente en cada sesión. Léelo completo antes de hacer cualquier cambio.

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
cd gitgov/gitgov-server/tests && API_KEY="57f1ed59-371d-46ef-9fdf-508f59bc4963" ./jenkins_integration_test.sh

# Jira integration test
cd gitgov/gitgov-server/tests && API_KEY="57f1ed59-371d-46ef-9fdf-508f59bc4963" ./jira_integration_test.sh

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
| `/health` | GET | None | Health check básico |
| `/health/detailed` | GET | None | Health check con latencia DB y uptime |
| `/webhooks/github` | POST | HMAC | Webhooks de GitHub (push, create) |
| `/events` | POST | Bearer | Ingesta batch de eventos del cliente |
| `/logs` | GET | Bearer | Eventos combinados (dev: solo propios) |
| `/stats` | GET | Bearer (admin) | Estadísticas globales + pipeline 7d |
| `/dashboard` | GET | Bearer (admin) | Datos del dashboard |
| `/jobs/metrics` | GET | Bearer (admin) | Métricas del job queue |
| `/jobs/dead` | GET | Bearer (admin) | Lista de jobs muertos |
| `/jobs/{job_id}/retry` | POST | Bearer (admin) | Reintentar job muerto |
| `/integrations/jenkins` | POST | Bearer (admin) | Pipeline events de Jenkins |
| `/integrations/jenkins/status` | GET | Bearer (admin) | Health check Jenkins |
| `/integrations/jenkins/correlations` | GET | Bearer (admin) | Correlaciones commit↔pipeline |
| `/integrations/jira` | POST | Bearer (admin) | Ingesta webhook de Jira |
| `/integrations/jira/status` | GET | Bearer (admin) | Health check Jira |
| `/integrations/jira/correlate` | POST | Bearer (admin) | Correlación batch commit↔ticket |
| `/integrations/jira/ticket-coverage` | GET | Bearer (admin) | Cobertura de tickets |
| `/integrations/jira/tickets/{id}` | GET | Bearer (admin) | Detalle de ticket |
| `/compliance/{org_name}` | GET | Bearer (admin) | Dashboard de compliance |
| `/signals` | GET | Bearer | Noncompliance signals |
| `/signals/{signal_id}` | POST | Bearer | Actualizar signal |
| `/signals/{signal_id}/confirm` | POST | Bearer (admin) | Confirmar signal (bypass detectado) |
| `/signals/detect/{org_name}` | POST | Bearer (admin) | Disparar detección de signals |
| `/violations/{id}/decisions` | GET | Bearer | Historial de decisiones sobre violación |
| `/violations/{id}/decisions` | POST | Bearer (admin) | Añadir decisión a violación |
| `/policy/{repo_name}` | GET | Bearer | Obtener política del repo (gitgov.toml) |
| `/policy/check` | POST | Bearer (admin) | Policy check advisory (Jenkins) |
| `/policy/{repo_name}/history` | GET | Bearer | Historial de cambios de política |
| `/policy/{repo_name}/override` | PUT | Bearer (admin) | Override de política |
| `/export` | POST | Bearer | Exportar eventos (crea audit log de export) |
| `/api-keys` | POST | Bearer (admin) | Crear nueva API key |
| `/audit-stream/github` | POST | Bearer (admin) | Ingestar GitHub audit log stream |
| `/governance-events` | GET | Bearer | Eventos de governance (branch protection, etc.) |

> **Nota:** `/logs` aplica scoping: `Admin` ve todos, `Developer` solo ve sus propios eventos (filtrado por `user_login = client_id`).

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
- **Fix:** `ServerStats` y `CombinedEvent` deben ser idénticos en ambos lados
- Las tres copias que deben sincronizarse: `models.rs` (server), `control_plane/server.rs` (Tauri), `useControlPlaneStore.ts` (frontend)

### localhost ≠ 127.0.0.1 (split-brain local)
- **Síntoma:** Desktop envía eventos pero el dashboard no los muestra
- **Causa:** `localhost` puede resolver a IPv6 (`::1`), pegando a un proceso diferente
- **Fix:** Usar `127.0.0.1` canónico en toda la configuración. El código ya normaliza `localhost→127.0.0.1` en 4 lugares (server.rs, server_commands.rs, lib.rs, useControlPlaneStore.ts)

---

## Archivos Críticos

| Archivo | Propósito | Precaución |
|---------|-----------|------------|
| `gitgov/gitgov-server/src/handlers.rs` | API handlers | Response structures, integraciones |
| `gitgov/gitgov-server/src/models.rs` | Data structures | Serde attributes, defaults |
| `gitgov/gitgov-server/src/auth.rs` | Middleware auth | Token validation |
| `gitgov/gitgov-server/src/db.rs` | Database queries | COALESCE, append-only |
| `gitgov/gitgov-server/src/main.rs` | Routes, startup | Wiring de rutas y middleware |
| `gitgov/gitgov-server/supabase_schema_v6.sql` | Schema actual (v6) | V1-V6 son incrementales |
| `gitgov/src-tauri/src/outbox/outbox.rs` | Cola de eventos offline | Auth headers, retry logic |
| `gitgov/src-tauri/src/control_plane/server.rs` | Cliente HTTP al server | Structs deben coincidir con models.rs |
| `gitgov/src-tauri/src/commands/git_commands.rs` | Operaciones Git | Event logging, trunca a 500 files |
| `gitgov/src-tauri/src/commands/server_commands.rs` | Tauri commands del server | Expone cmds al frontend |
| `gitgov/src/store/useControlPlaneStore.ts` | Estado del dashboard | Sync con server structs, cache Jira 2min |
| `gitgov/src/components/control_plane/ServerDashboard.tsx` | Dashboard UI | Auto-refresh 30s, carga paralela |
| `gitgov/src/components/control_plane/RecentCommitsTable.tsx` | Tabla commits | Correlación CI badge, ticket badges |
| `gitgov/src/components/control_plane/dashboard-helpers.tsx` | Lógica UI dashboard | buildDashboardRows, extractTicketIds |
| `gitgov-web/lib/config/site.ts` | Config web app | URL Vercel, versión, download path |
| `gitgov-web/lib/i18n/translations.ts` | Traducciones EN/ES | Agregar clave en ambos idiomas |

---

## Variables de Entorno

### Server (`gitgov/gitgov-server/.env`)
- `DATABASE_URL` — PostgreSQL connection string (Supabase pooler)
- `SUPABASE_URL` / `SUPABASE_ANON_KEY` / `SUPABASE_SERVICE_KEY`
- `GITGOV_JWT_SECRET` — JWT signing
- `GITGOV_SERVER_ADDR` — `0.0.0.0:3000`
- `GITGOV_API_KEY` — API key para desktop clients (se inserta en DB al arrancar si no existe)
- `GITHUB_WEBHOOK_SECRET` — Validación HMAC de webhooks GitHub
- `JENKINS_WEBHOOK_SECRET` — Secret adicional para Jenkins (header: `x-gitgov-jenkins-secret`, opcional)
- `JIRA_WEBHOOK_SECRET` — Secret adicional para Jira (header: `x-gitgov-jira-secret`, opcional)
- `GITHUB_PERSONAL_ACCESS_TOKEN` — Token para GitHub MCP
- `RUST_LOG` — Log level (`gitgov_server=info,tower_http=info`)

**Rate limiting (configurables, valores por defecto):**
- `GITGOV_RATE_LIMIT_EVENTS_PER_MIN` — default `240` (ruta `/events`)
- `GITGOV_RATE_LIMIT_AUDIT_STREAM_PER_MIN` — default `60` (ruta `/audit-stream/github`)
- `GITGOV_RATE_LIMIT_JENKINS_PER_MIN` — default `120` (ruta `/integrations/jenkins`)
- `GITGOV_RATE_LIMIT_JIRA_PER_MIN` — default `120` (ruta `/integrations/jira`)
- `GITGOV_RATE_LIMIT_ADMIN_PER_MIN` — default `60` (logs, stats, dashboard)
- `GITGOV_JENKINS_MAX_BODY_BYTES` — default `262144` (256 KB)
- `GITGOV_JIRA_MAX_BODY_BYTES` — default `524288` (512 KB)

**Job Worker (hardcodeado en main.rs):**
- `JOB_WORKER_TTL_SECS = 300` — TTL antes de considerar job estancado
- `JOB_POLL_INTERVAL_SECS = 5` — Frecuencia de polling de jobs
- `JOB_ERROR_BACKOFF_SECS = 10` — Backoff base para errores (exponencial, máx ×32)

### Desktop (`gitgov/.env` y `gitgov/src-tauri/.env`)
- `VITE_SERVER_URL=http://localhost:3000` — URL del server para el frontend (Vite)
- `VITE_API_KEY` — API key del servidor para el frontend
- `GITGOV_SERVER_URL` — URL del server para el backend Tauri (leído en lib.rs al arrancar)
- `GITGOV_API_KEY` — API key del servidor para el backend Tauri

> **IMPORTANTE:** Existen DOS fuentes de config del server URL: `VITE_SERVER_URL` (frontend React) y `GITGOV_SERVER_URL` (Tauri Rust). El store `useControlPlaneStore` tiene prioridad: input > previous > env > localStorage > default `127.0.0.1:3000`. Hay un fallback legacy hardcodeado: `'57f1ed59-371d-46ef-9fdf-508f59bc4963'`.

### Web App (`gitgov-web/.env.local`) — producción en Vercel
- No requiere variables de entorno especiales actualmente
- URL canónica: `https://git-gov.vercel.app`

---

## Estado del Proyecto (Feb 2026)

### Funcional
- Pipeline E2E: Desktop → Server → PostgreSQL → Dashboard
- GitHub OAuth + API Keys
- Outbox offline con reintentos
- V1.2-A (Jenkins MVP): ingesta, correlación, widget Pipeline Health, policy/check advisory
- V1.2-B Preview (Jira): ingesta, correlación batch, ticket coverage, badges

### Tests (estado actual)
- `cargo test` — 36 unit tests en CI (models, handlers, auth) — **funcional**
- `smoke_contract.sh` — 14 contract checks live (paginación + Golden Path) — **funcional, local**
- E2E scripts (`e2e_flow_test.sh`, `jenkins_integration_test.sh`, `jira_integration_test.sh`) — **manuales**
- Tests de integración con DB mock — **pendiente**

### Pendiente (Alta Prioridad)
- Correlación de `related_prs` automática
- HTTPS en EC2 (dominio + Let's Encrypt)
- Tests de integración backend con DB mock (sin Supabase real)

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
- `npm run lint` → errores nuevos introducidos: 0

**4. Impacto en Golden Path:**
- ¿Modifica auth/token/API key/handlers/dashboard? → Sí/No
- Si Sí: evidencia de que el flujo Desktop→/events→PostgreSQL→Dashboard sigue intacto

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
  -d '{"events": [{"event_type": "commit", "user_login": "test", "status": "success", "timestamp": 0}]}'
# Esperar: {"accepted":1,"duplicates":0,"errors":0}

# 3. Verificar que /stats responde sin 401
curl http://127.0.0.1:3000/stats \
  -H "Authorization: Bearer {api_key}"
# Esperar: JSON con ServerStats (no {"error":"..."})
```

Archivos cuyo cambio OBLIGA a ejecutar esta validación:
- `gitgov-server/src/auth.rs`
- `gitgov-server/src/handlers.rs`
- `gitgov-server/src/main.rs`
- `gitgov-server/src/models.rs`
- `src-tauri/src/outbox/outbox.rs`
- `src-tauri/src/control_plane/server.rs`
- `src/store/useControlPlaneStore.ts`
