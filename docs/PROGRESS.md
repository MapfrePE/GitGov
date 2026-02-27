# GitGov - Registro de Progreso

## Actualización Reciente (2026-02-26)

### Pruebas E2E, Bug offset, Tests de Contrato y CI

#### Bug corregido: `offset` obligatorio en endpoints paginados

`/logs`, `/integrations/jenkins/correlations`, `/signals`, `/governance-events` fallaban con `"missing field offset"` si el cliente no lo mandaba. Causa: los structs `EventFilter`, `JenkinsCorrelationFilter`, `SignalFilter`, `GovernanceEventFilter` tenían `limit: usize` y `offset: usize` como campos requeridos en serde.

**Fix:** `#[serde(default)]` en los 4 structs → `usize::default() = 0`. Los handlers ya tenían `if limit == 0 { fallback }` así que no requirieron cambio. Backward compatible: si el cliente manda offset explícito, se respeta.

Defaults resultantes por endpoint:

| Endpoint | `limit` default | `offset` default |
|----------|----------------|-----------------|
| `/logs` | 100 | 0 |
| `/integrations/jenkins/correlations` | 20 | 0 |
| `/signals` | 100 | 0 |
| `/governance-events` | 100 | 0 |

#### Tests E2E ejecutados (Golden Path + Jenkins + Jira)

Suite completa corrida manualmente contra servidor real (Supabase):

| Suite | Tests | Resultado |
|-------|-------|-----------|
| Golden Path (`e2e_flow_test.sh`) | Health, auth, event ingest, logs, stats | ✅ |
| Jenkins V1.2-A (`jenkins_integration_test.sh`) | Status, ingest válido, duplicado, auth reject, correlations | ✅ |
| Jira V1.2-B (`jira_integration_test.sh`) | Status, ingest PROJ-123, auth reject, batch correlate, coverage, detail | ✅ |
| Correlación regex Jira | Commit con `"PROJ-123"` + branch `"feat/PROJ-123-dashboard"` → `correlations_created:1`, ticket con `related_commits` y `related_branches` poblados | ✅ |
| `/health/detailed` | `latency_ms:268`, `pending_events:0` | ✅ |

Datos reales en DB: 26 commits últimas 72h, 1 con ticket, 3.8% coverage.

También se corrigieron los scripts de test que tenían el bug de `offset`:
- `e2e_flow_test.sh` — `uuidgen` fallback para Windows + `&offset=0` en 2 llamadas a `/logs`
- `jenkins_integration_test.sh` — `&offset=0` en `/integrations/jenkins/correlations`

#### Tests unitarios de contrato (36 tests, 11 nuevos)

Añadidos en `models.rs` `#[cfg(test)]`:

**5 tests de paginación (regresión offset):**
- `event_filter_offset_optional_defaults_to_zero`
- `event_filter_all_pagination_optional`
- `event_filter_explicit_offset_respected`
- `jenkins_correlation_filter_offset_optional`
- `jenkins_correlation_filter_all_pagination_optional`

**6 tests Golden Path (contrato de payload):**
- `golden_path_stage_files_contract` — files no vacío, event_uuid presente
- `golden_path_commit_contract` — commit_sha presente
- `golden_path_attempt_push_contract` — branch correcto
- `golden_path_successful_push_contract` — status success, uuid
- `golden_path_response_accepted_shape` — `ClientEventResponse` {accepted, duplicates, errors}
- `golden_path_duplicate_detected_in_response` — UUID en `duplicates[]` al reenviar

Resultado: `36 passed; 0 failed; 0.00s`. Pure-serde — no requieren DB ni server.

#### smoke_contract.sh — validación live

`gitgov/gitgov-server/tests/smoke_contract.sh` con dos secciones:
- **A (8 checks):** endpoints sin params opcionales → responden correcto; backward compat con params explícitos
- **B (6 checks):** Golden Path live — `stage_files → commit → attempt_push → successful_push` aceptados, los 4 visibles en `/logs`, reenvío detectado en `duplicates[]`

Corrida contra servidor real: `exit 0` ✅

#### Infraestructura de testing añadida

| Archivo | Qué es |
|---------|--------|
| `gitgov/gitgov-server/Makefile` | `make check`, `make test`, `make smoke`, `make all` |
| `gitgov/gitgov-server/tests/smoke_contract.sh` | 14 contract checks (8 paginación + 6 Golden Path) |
| `.github/workflows/ci.yml` | `cargo test` añadido al job `server-lint` + artifact upload en failure |
| `docs/GOLDEN_PATH_CHECKLIST.md` | Sección "Antes de release: make test + make smoke" |

---

### Análisis Exhaustivo del Proyecto — Hallazgos de Arquitectura

Se realizó un análisis milimétrico del codebase completo. Principales hallazgos documentados:

**Componente inédito: gitgov-web**
- El proyecto tiene **4 componentes**, no 3 como indicaba la documentación
- `gitgov-web/` es un sitio Next.js 14 + React 18 + Tailwind v3 (pnpm) con i18n EN/ES
- Desplegado en Vercel en `https://git-gov.vercel.app`
- Rutas: `/`, `/features`, `/download`, `/pricing`, `/contact`, `/docs`
- La download page es un Server Component que calcula SHA256 del installer en build time
- Versión actual del installer: `0.1.0` (`GitGov_0.1.0_x64-setup.exe`)

**Diferencias de stack Desktop vs Web (importante para no confundir):**
- Desktop: React **19**, Tailwind **v4**, **npm**, `VITE_*` + `GITGOV_*` env vars
- Web: React **18**, Tailwind **v3**, **pnpm**, sin conexión al servidor

**Dual env vars en Desktop App:**
- `VITE_SERVER_URL` / `VITE_API_KEY` → solo para el frontend React (Vite)
- `GITGOV_SERVER_URL` / `GITGOV_API_KEY` → para el backend Rust de Tauri
- Son independientes. El outbox usa las `GITGOV_*`, el dashboard UI usa las `VITE_*`

**Endpoints no documentados encontrados (~15 adicionales):**
- `/compliance`, `/export`, `/api-keys`, `/governance-events`, `/signals`, `/violations`
- `/jobs/dead`, `/jobs/retry/{id}`, `/health/detailed`
- `/integrations/jenkins`, `/integrations/jenkins/status`, `/integrations/jenkins/correlations`
- `/integrations/jira`, `/integrations/jira/status`, `/integrations/jira/correlate`, etc.

**Roles del sistema (4, no 2):** Admin, Architect, Developer, PM

**Rate limiting configurado y en producción:**
- 8 variables de entorno `RATE_LIMIT_*_RPS/BURST` con defaults conservadores
- Clave de rate limiting: `{IP}:{SHA256(auth)[0:12]}`

**Job Worker hardcoded:** TTL=300s, poll=5s, backoff=10s

**Dashboard UI detalles:**
- Auto-refresh cada 30 segundos
- Máx. 10 commits en RecentCommitsTable
- Cache TTL de 2 min para detalle de tickets Jira
- Filtros Jira persisten en localStorage

**Deploy en producción:**
- Control Plane: Ubuntu 22.04 + Nginx + systemd en EC2 `3.143.150.199`
- Binario en `/opt/gitgov/bin/gitgov-server`
- HTTP (pendiente: dominio + HTTPS + Let's Encrypt)

**Toda la documentación actualizada:** CLAUDE.md, ARCHITECTURE.md, QUICKSTART.md, TROUBLESHOOTING.md

---

## Actualización Reciente (2026-02-24)

### Resumen Ejecutivo

GitGov avanzó de un estado "funcional mínimo" a una base mucho más sólida y demoable:

- Se endureció el sistema sin romper el flujo core (`Desktop -> commit/push -> server -> dashboard`)
- Se mejoró la UX del dashboard para mostrar commits de forma más estándar (estilo GitHub)
- Se implementó **V1.2-A (Jenkins-first MVP)** de forma funcional
- Se implementó un **preview fuerte de V1.2-B (Jira + ticket coverage)** con backend, UI y pruebas

### Golden Path (NO ROMPER) - Estado

El flujo base se mantiene operativo y protegido:

1. Desktop detecta cambios
2. Commit desde la app
3. Push desde la app
4. Server recibe eventos en `/events`
5. Dashboard muestra commits y logs

Documentos de soporte:
- `docs/GOLDEN_PATH_CHECKLIST.md`
- `docs/V1.2-A_DEMO.md`

---

## Avances Técnicos Implementados (2026-02-24)

### 1. Hardening y estabilización del core (post-auditoría)

**Seguridad backend**
- Scoping real en endpoints sensibles (`signals`, `export`, `governance-events`)
- Mejoras de autorización en decisiones/violations/signals
- `/events` endurecido para evitar spoofing en no-admin
- Validación HMAC de GitHub corregida usando body raw real
- Sanitización de errores en middleware de auth

**Integridad de datos / DB**
- Alineación parcial backend ↔ modelo append-only (`signals` / `signal_decisions`)
- Fallback en decisiones de violations cuando la función SQL legacy falla por triggers
- Hotfix schema adicional (`supabase_schema_v4.sql`) para comportamiento append-only

**Rendimiento / robustez**
- Correcciones de paginación y filtros en queries de eventos
- Optimización conservadora de `insert_client_events_batch()` (dedupe + transacción + fallback)
- Rate limiting básico para `/events`, `/audit-stream/github`, `/integrations/jenkins`, `/integrations/jira`
- Body limits en endpoints de integraciones

---

### 2. Dashboard y UX (Control Plane)

**Commits Recientes (reorganización)**
- La vista principal ahora muestra **una fila por commit**
- Se ocultaron eventos técnicos (`attempt_push`, `successful_push`, etc.) en la tabla principal
- `stage_files` se asocia al commit como detalle (`Ver archivos`)
- Se muestra:
  - mensaje de commit
  - hash corto
  - badge `ci:<status>` si hay correlación Jenkins
  - badges de tickets (`PROJ-123`) detectados en commit/rama

**Jira Ticket Coverage UI**
- Widget `Ticket Coverage (Jira)` con:
  - cobertura %
  - commits con/sin ticket
  - tickets huérfanos
- Botón manual `Correlacionar`
- Filtros UI:
  - repo
  - rama
  - horas
- Botón `Aplicar filtros`
- Persistencia local de filtros Jira (localStorage)

**Panel de detalle de ticket**
- Click en badge de ticket (`PROJ-123`) abre panel de detalle
- Carga detalle real desde backend (`GET /integrations/jira/tickets/{ticket_id}`)
- Muestra:
  - status
  - assignee
  - summary/title
  - link al ticket
- Spinner / estado de carga
- Cache TTL (2 min) para detalle de tickets Jira
- Panel expandible con relaciones:
  - branches relacionadas
  - commits relacionados
  - PRs relacionadas (si existen)

---

### 3. V1.2-A (Jenkins-first MVP) - Implementado

**Base de datos / schema**
- `supabase_schema_v5.sql`:
  - `pipeline_events` (append-only)
  - índices para correlación por `commit_sha`
  - dedupe inicial v1

**Backend Jenkins**
- `POST /integrations/jenkins`
- `GET /integrations/jenkins/status`
- `GET /integrations/jenkins/correlations`
- Hardening compatible:
  - `JENKINS_WEBHOOK_SECRET` (opcional)
  - rate limit específico
  - body limit específico

**Correlación commit -> pipeline**
- Correlación básica por `commit_sha` (exact match y prefijo short/full)

**Stats / Dashboard**
- `/stats` incluye `pipeline`
- Widget `Pipeline Health (7 días)` en dashboard

**Policy advisory**
- `POST /policy/check` implementado en modo advisory (no bloqueante)

**Pruebas / Demo**
- `gitgov/gitgov-server/tests/jenkins_integration_test.sh`
- `docs/V1.2-A_DEMO.md`

Estado V1.2-A: **MVP funcional y demoable**

---

### 4. V1.2-B (Jira + Ticket Coverage) - Preview avanzado

**Schema**
- `supabase_schema_v6.sql`
  - `project_tickets`
  - `commit_ticket_correlations` (append-only)

**Backend Jira**
- `POST /integrations/jira` (ingesta snapshot de issue)
- `GET /integrations/jira/status`
- `POST /integrations/jira/correlate` (correlación batch commit↔ticket)
- `GET /integrations/jira/ticket-coverage`
- `GET /integrations/jira/tickets/{ticket_id}` (detalle real de ticket)

**Correlación y enriquecimiento**
- extracción de tickets (`ABC-123`) desde commit message y branch
- dedupe de correlación por `(commit_sha, ticket_id)`
- actualización automática de `project_tickets.related_commits` / `related_branches`
  al crear correlaciones nuevas

**UI / Demo**
- widget `Ticket Coverage`
- listas preview:
  - commits sin ticket
  - tickets sin commits
- badges de ticket por commit
- detalle real de ticket en panel

**Pruebas**
- `gitgov/gitgov-server/tests/jira_integration_test.sh`
- tests unitarios de regex/extracción de tickets en `handlers.rs`

Estado V1.2-B: **preview funcional (backend + UI + scripts), listo para iterar**

---

### 5. Documentación y planificación actualizadas

- `docs/GITGOV_ROADMAP_V1.2.md` reestructurado con enfoque realista (`V1.2-A/B/C`)
- `docs/BACKLOG_V1.2-A.md` creado con tareas/épicas/estimaciones
- `AGENTS.md` actualizado con sección **Golden Path (NO ROMPER)**

---

## Pendientes Relevantes (actualizados)

### Alta prioridad (siguiente tramo)
- Endurecer pruebas reales / corrida integral de demo Jenkins + Jira en entorno local completo
- Pulir correlación de `related_prs` (aún no se puebla automáticamente)
- Mejorar cobertura de tests automatizados backend (integración Jira/Jenkins)

### Media prioridad
- Correlation Engine avanzado (GitHub webhooks + desktop + Jira + Jenkins en una sola vista)
- Drift detection más completo
- Optimización de queries para datasets grandes

---

## Documentación del Proyecto

| Documento | Propósito |
|-----------|-----------|
| [AGENTS.md](../AGENTS.md) | Instrucciones para agentes de IA |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Arquitectura del sistema explicada |
| [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) | Guía de solución de problemas |
| [QUICKSTART.md](./QUICKSTART.md) | Guía de inicio rápido |

---

## Estado Actual: Sistema Funcional

### Qué funciona hoy

La versión actual de GitGov tiene todas las funcionalidades básicas operativas:

**Desktop App**
- Inicia correctamente y muestra el dashboard principal
- Conecta con GitHub vía OAuth
- Permite hacer commits y pushes
- Registra eventos en el outbox local
- Envía eventos al servidor cuando hay conexión

**Control Plane Server**
- Corre en localhost:3000
- Recibe y almacena eventos de las desktop apps
- Autentica requests con API keys
- Proporciona endpoints para dashboards y estadísticas

**Pipeline de Eventos**
- Los eventos fluyen desde Desktop → Server → PostgreSQL → Dashboard
- La deduplicación funciona (event_uuid único)
- Los eventos se muestran en tiempo real

### Visualización del Dashboard

El dashboard muestra:

```
┌────────────────────────────────────────────────────────────────────┐
│  Conectado al Control Plane                                        │
│  URL del servidor: http://localhost:3000                           │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌───────────┐ │
│  │ Total GitHub │ │ Pushes Hoy   │ │ Bloqueados   │ │Devs Activ │ │
│  │      0       │ │      0       │ │      0       │ │     1     │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └───────────┘ │
│                                                                    │
│  Tasa de Éxito: 100.0%          │  Eventos Cliente por Estado     │
│  Repos Activos: 0               │  ┌─────────────────────────┐    │
│                                 │  │ success: 25             │    │
│                                 │  └─────────────────────────┘    │
│                                                                    │
│  Eventos Recientes:                                                │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │ Hora              │ Usuario   │ Tipo            │ Estado     ││
│  ├────────────────────────────────────────────────────────────────┤│
│  │ 22/2/2026 5:45:41 │ MapfrePE  │ successful_push │ success    ││
│  │ 22/2/2026 5:45:41 │ MapfrePE  │ attempt_push    │ success    ││
│  │ 22/2/2026 5:45:13 │ MapfrePE  │ commit          │ success    ││
│  │ 22/2/2026 5:44:43 │ MapfrePE  │ stage_files     │ success    ││
│  └────────────────────────────────────────────────────────────────┘│
└────────────────────────────────────────────────────────────────────┘
```

---

## Historia del Proyecto

### Fase 1: Sincronización Control Plane (22 de Febrero, 2026)

**El problema:** La desktop app no podía comunicarse con el servidor. Los eventos no llegaban y el dashboard permanecía vacío.

**Los bugs encontrados y resueltos:**

**Bug 1 - Panic en get_stats()**

El servidor crasheaba cuando intentaba obtener estadísticas. Resulta que PostgreSQL devuelve NULL cuando una función de agregación no tiene datos, pero Rust esperaba un objeto vacío.

La solución fue doble: modificar las queries SQL para usar COALESCE (que devuelve un valor por defecto cuando hay NULL), y agregar atributos en Rust para que los campos HashMap tengan valores default.

**Bug 2 - Serialización ServerStats**

El cliente y el servidor tenían estructuras de datos diferentes. El cliente esperaba campos planos, el servidor enviaba objetos anidados.

Se sincronizaron las estructuras en ambos lados para que coincidan exactamente.

**Bug 3 - Serialización CombinedEvent**

Similar al anterior. El endpoint /logs enviaba eventos en un formato que el cliente no esperaba.

Se agregó el tipo CombinedEvent en el cliente y se actualizó el frontend.

**Bug 4 - 401 Unauthorized**

El outbox enviaba eventos pero el servidor los rechazaba. El problema: el header de autenticación era incorrecto.

El servidor esperaba `Authorization: Bearer`, pero el outbox enviaba `X-API-Key`. Se corrigió en dos lugares del código.

**Resultado:** El pipeline completo funciona. Los eventos fluyen desde la desktop app hasta el dashboard.

---

### Fase 2: Pipeline de Eventos End-to-End (22 de Febrero, 2026)

**El logro:** El sistema ahora registra correctamente todos los eventos desde el desktop hasta el Control Plane.

**Cómo funciona el flujo:**

1. El usuario hace push en la desktop app
2. La app registra "attempt_push" en el outbox local
3. Ejecuta el push real a GitHub
4. Si tiene éxito, registra "successful_push" en el outbox
5. El worker de background envía los eventos al servidor
6. El servidor los guarda en PostgreSQL
7. El dashboard muestra los eventos en tiempo real

**Tipos de eventos registrados:**

| Evento | Cuándo se genera |
|--------|------------------|
| attempt_push | Antes de cada push |
| successful_push | Push completado |
| blocked_push | Push a rama protegida |
| push_failed | Push falló |
| commit | Commit creado |
| stage_files | Archivos agregados al staging |
| create_branch | Rama creada |
| blocked_branch | Creación de rama bloqueada |

---

### Fase 3: Production Hardening (21 de Febrero, 2026)

**El objetivo:** Preparar el sistema para producción con mejoras de robustez.

**Mejoras implementadas:**

**Job Queue Production-Grade**

El sistema de jobs en background tenía varios problemas de concurrencia que se resolvieron:

- **Race conditions:** Se implementó `FOR UPDATE SKIP LOCKED` para que múltiples workers no tomen el mismo job
- **Explosión de jobs:** Se agregó deduplicación con índice único
- **Reintentos infinitos:** Backoff exponencial con máximo de intentos y dead-letter queue
- **Reset peligroso:** Solo se pueden resetear jobs que realmente están atascados

**Cursor Incremental Seguro**

El cursor que marca qué eventos ya se procesaron usaba `created_at`, que es el tiempo del evento en GitHub. Pero los eventos pueden llegar tarde (retries, backlogs).

Se agregó un campo `ingested_at` que es el tiempo cuando el evento llegó al servidor. El cursor ahora usa este campo.

**Append-Only Triggers**

Se verificó que todas las tablas de auditoría son append-only:
- github_events: 100% inmutable
- client_events: 100% inmutable
- violations: Solo se puede cambiar el estado de resolución
- noncompliance_signals: 100% inmutable
- governance_events: 100% inmutable

**Job Metrics Endpoint**

Se agregó `/jobs/metrics` para ver el estado del queue:
- Cuántos jobs pending
- Cuántos running
- Cuántos dead
- Tiempos promedio

**Seguridad del Bootstrap**

El servidor imprimía la API key de bootstrap en los logs, lo cual es un problema en Docker/Kubernetes donde los logs son visibles.

Se implementó:
- Flag `--print-bootstrap-key` para explícitamente mostrar la key
- Detección de TTY para solo mostrar en terminal interactiva
- En Docker (sin TTY), la key no aparece en logs

**Stress Tests**

Se creó una suite de tests de stress:
- Idempotencia de webhooks
- Deduplicación de jobs
- Reset de jobs atascados
- Múltiples organizaciones
- Alto volumen de webhooks

---

### Fase 4: Audit Stream Endpoint (21 de Febrero, 2026)

**El objetivo:** Recibir eventos de gobernanza desde GitHub.

**Qué se implementó:**

Un nuevo endpoint `/audit-stream/github` que recibe batches de audit logs de GitHub. Estos logs incluyen:

- Cambios en branch protection
- Modificaciones de rulesets
- Cambios de permisos
- Cambios de acceso de teams

Se creó una nueva tabla `governance_events` para almacenar estos eventos, también append-only.

---

### Fase 5: Autenticación y Correlación (21 de Febrero, 2026)

**Middleware de Autenticación**

Se implementó un sistema completo de autenticación con roles:

- **admin:** Acceso total
- **developer:** Solo puede ver sus propios eventos

Los endpoints están protegidos según el nivel requerido:
- `/stats`, `/dashboard`: Solo admin
- `/logs`: Admin ve todo, developer solo sus eventos
- `/events`: Cualquier usuario autenticado
- `/webhooks/github`: Valida firma HMAC (sin JWT)

**Correlación y Confidence Scoring**

El sistema de detección de violaciones ahora es más sofisticado:

- **confidence = 'high':** Señal clara de bypass
- **confidence = 'low':** Telemetría incompleta, necesita investigación

No se muestra "BYPASS DETECTADO" automáticamente. Solo cuando un humano lo confirma.

**Violation Decisions**

Se separó la resolución de violaciones en una tabla separada:

Los tipos de decisión:
- acknowledged: Alguien vio la violación
- false_positive: No era una violación real
- resolved: Se resolvió el problema
- escalated: Se escaló a nivel superior
- dismissed: Se decidió ignorar
- wont_fix: Se decidió no arreglar

Esto crea un historial completo de cada violación.

---

## Qué Falta por Hacer

### Prioridad Alta

| Componente | Qué falta |
|------------|-----------|
| Jenkins + Jira E2E | Pruebas integrales reales en entorno completo (local + remoto) |
| `related_prs` | Correlación automática de PRs en `commit_ticket_correlations` |
| HTTPS en EC2 | Dominio + Let's Encrypt + redirección 80→443 |
| Webhooks GitHub | Configurar webhooks en repos de producción |

### Prioridad Media

| Componente | Qué falta |
|------------|-----------|
| Tests automatizados backend | Cobertura de integraciones Jira/Jenkins (parcial: 36 unit tests + smoke_contract.sh; falta integración real con DB mock) |
| Desktop Updater | Servidor de releases S3/CloudFront para tauri-plugin-updater |
| Correlation Engine V2 | GitHub webhooks + desktop + Jira + Jenkins en una sola vista (V1.2-C) |
| Drift Detection | Detectar cuando configuración difiere de política |
| gitgov-web: installer | Subir `GitGov_0.1.0_x64-setup.exe` a `public/downloads/` |
| Performance | Optimizar queries para datasets grandes |

---

## Build Status

Los builds compilan con warnings menores (variables no usadas, código muerto), sin errores.

- Desktop (Tauri): Compila correctamente
- Server (Axum): Compila correctamente
- Clippy: Solo warnings de estilo, sin errores

---

## Archivos Clave del Proyecto

| Ubicación | Qué hace |
|-----------|----------|
| `gitgov/src-tauri/src/outbox/` | Cola de eventos offline JSONL |
| `gitgov/src-tauri/src/commands/git_commands.rs` | Operaciones Git + logging de eventos |
| `gitgov/src-tauri/src/commands/server_commands.rs` | Comandos Tauri para comunicación con servidor |
| `gitgov/src-tauri/src/control_plane/server.rs` | HTTP client singleton (OnceLock) al Control Plane |
| `gitgov/src/store/useControlPlaneStore.ts` | Estado del dashboard, config resolution, cache Jira |
| `gitgov/src/components/control_plane/ServerDashboard.tsx` | Dashboard principal, auto-refresh 30s |
| `gitgov/gitgov-server/src/main.rs` | Rutas, rate limiters, bootstrap API key |
| `gitgov/gitgov-server/src/handlers.rs` | 30+ HTTP handlers, integraciones |
| `gitgov/gitgov-server/src/auth.rs` | Middleware SHA256 + roles |
| `gitgov/gitgov-server/src/models.rs` | Estructuras de datos (serde + defaults) |
| `gitgov/gitgov-server/src/db.rs` | Queries PostgreSQL (COALESCE siempre) |
| `gitgov/gitgov-server/supabase_schema*.sql` | Schema versionado (v1 a v6) |
| `gitgov-web/lib/config/site.ts` | Config del sitio público (URL, versión, nav) |
| `gitgov-web/lib/i18n/translations.ts` | Traducciones EN/ES del sitio |

---

## Próximos Pasos

1. **Configurar webhooks de GitHub** en los repositorios
2. **Implementar correlation engine** para detectar bypasses
3. **Agregar drift detection** para validación de políticas
4. **Expandir tests** para mayor cobertura
5. **Deploy a producción** cuando esté listo
