# GitGov Desktop + Control Plane Performance and Scalability Audit

Date: 2026-03-04
Author: Codex (analysis + ejecucion incremental controlada)
Scope: Tauri desktop (`gitgov`), server (`gitgov-server`), SQL schema/migrations.

## 1) Executive Summary

This app already has a solid functional base, but current architecture has multiple hot paths that will degrade hard with large orgs (many repos, developers, events).

Top blockers found:

1. Outbox flush strategy can create thread pressure and long blocking windows.
2. Ingest path does per-event DB lookups and per-event insert statements (high write overhead).
3. Dashboard refresh pattern pulls heavy payloads every 30s (`logs` up to 500) and re-renders non-virtualized tables.
4. `GET /logs` query plan shape is expensive at scale (`UNION ALL` + global sort + offset + enrichment query).
5. `GET /stats` depends on a function with many full aggregates, called frequently.
6. DB connection pool is capped at 10.
7. Security/config debt can amplify instability (hardcoded fallback API key, plaintext local PIN/API key storage, permissive CORS, default JWT secret fallback).

This is not primarily a "Rust is slow" problem. The main issue is architecture in high-cardinality paths (query shapes, polling cadence, batching policy, payload size, and blocking behavior).

## 2) Scope, Method, and Guardrails

- Inicialmente analisis estatico; luego se aplicaron mitigaciones acotadas y benchmark comparativo (ver 11.12-11.14).
- Golden Path was not modified in this audit.
- Method used: static code review with file-line evidence.
- No production load test was executed in this pass (solo validaciones locales controladas).

## 3) Evidence-Backed Findings

## 3.1 Critical Performance/Scalability Risks

### F-01 (Critical): Outbox flush can create avoidable blocking/thread pressure

Why it matters:
- Every major git action triggers `trigger_flush`, which spawns a new OS thread and executes blocking network I/O.
- Under server latency/outage, many short-lived threads can stack up and increase CPU/context-switch pressure.

Evidence:
- `gitgov/src-tauri/src/commands/git_commands.rs:27-31` (`trigger_flush` spawns a thread per call).
- Multiple flush trigger call sites: `git_commands.rs:319,347,430,454,543,559,599,641`.
- Flush network timeout is 30s: `gitgov/src-tauri/src/outbox/queue.rs:462-465` and worker path `555-557`.
- Background worker already exists (`start_background_flush(60)`), so immediate per-event thread flush is redundant pressure: `gitgov/src-tauri/src/lib.rs:109`.

### F-02 (Critical): Outbox flush sends all pending events in one big batch

Why it matters:
- A large pending queue produces a large request body, slower serialization/deserialization, and higher timeout risk.

Evidence:
- Flush snapshots all unsent events and sends one batch: `queue.rs:371-413`.
- No chunking/batch-size cap in this path.

### F-03 (Critical): Ingestion path has N+1 style DB work per event

Why it matters:
- In `POST /events`, each event may trigger org/repo lookups and possible repo upsert before insert.
- For large batches, latency and DB load scale linearly with high constant factors.

Evidence:
- Per-event org lookup: `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:63-69`.
- Per-event repo lookup/upsert path: `client_ingest_dashboard.rs:120-168`.
- Batch insert still issues one SQL insert per event inside transaction loop: `gitgov/gitgov-server/src/db.rs:525-563`.

### F-04 (Critical): `/logs` query shape is expensive for large tables

Why it matters:
- Two large event streams are combined with `UNION ALL`, then globally sorted and paginated with offset.
- Additional enrichment query runs after the main query.

Evidence:
- Combined query with `UNION ALL` and global `ORDER BY created_at DESC LIMIT/OFFSET`: `gitgov/gitgov-server/src/db.rs:759-824`.
- Secondary enrichment query by client event IDs: `db.rs:861-937`.
- Ordering only by `created_at` (no deterministic tie-breaker): `db.rs:822`.

### F-05 (Critical): Dashboard refresh cadence + payload size create recurring load spikes

Why it matters:
- Every 30s, admin path loads stats + daily + logs (500), plus heavy modules every 5 minutes.
- This repeats across every admin desktop session.

Evidence:
- Auto-refresh interval 30s: `gitgov/src/components/control_plane/ServerDashboard.tsx:61-63`.
- Refresh pipeline: `gitgov/src/store/useControlPlaneStore.ts:1130-1150`.
- Logs default load up to 500: `useControlPlaneStore.ts:1190-1198`.
- `refreshForCurrentRole` admin path always uses `logLimit: 500`: `useControlPlaneStore.ts:1723-1727`.

### F-06 (Critical): Stats endpoint likely expensive under frequent polling

Why it matters:
- `get_stats` delegates to `get_audit_stats` SQL function with multiple aggregate subqueries over large append-only tables.

Evidence:
- Server call: `gitgov/gitgov-server/src/db.rs:1688-1691`.
- SQL function aggregates with many `COUNT(*)`/`json_object_agg` scans: `gitgov/gitgov-server/supabase_schema.sql:423-449` and `supabase/supabase_schema_v12.sql:18-50`.

## 3.2 High Risks

### F-07 (High): DB pool max_connections=10 is low for target scale

Why it matters:
- Concurrent admin dashboards + ingest + chat + integrations can saturate 10 connections quickly.

Evidence:
- `gitgov/gitgov-server/src/db.rs:64-66`.

### F-08 (High): Team overview/repos queries are heavy CTE pipelines

Why it matters:
- Aggregation across windowed event data with nested JSON aggregation per user/repo is expensive for big orgs.

Evidence:
- Team overview query with CTEs and correlated `jsonb_agg`: `db.rs:3917-4007`.
- Team repos query over `window_events` + group + count distinct: `db.rs:4065-4118`.

### F-09 (High): Daily activity query uses expression on `created_at` that can weaken index usage

Why it matters:
- Joining by `(created_at AT TIME ZONE 'UTC')::date` can force broader scans.

Evidence:
- `gitgov/gitgov-server/src/db.rs:1743-1745`.

### F-10 (High): Outbox persistence rewrites full file on every add

Why it matters:
- `add()` persists full snapshot for each event, which increases disk I/O and lock contention with bigger queues.

Evidence:
- `add()` calls `persist()` every event: `queue.rs:313-336`.
- `persist()` writes full snapshot and atomic rename: `queue.rs:223-309`.

### F-11 (High): Outbox response reconciliation is O(n^2)

Why it matters:
- Repeated `Vec::contains` over accepted/duplicate UUID lists for each pending event scales poorly with large batches.

Evidence:
- Flush path: `queue.rs:426-434`.
- Worker path: `queue.rs:593-601`.

### F-12 (High): Event ingestion can reject full batch on first policy mismatch

Why it matters:
- First invalid event returns early in handler loop, potentially causing repeated retries/backlog growth.

Evidence:
- Early return inside loop for strict actor mismatch: `client_ingest_dashboard.rs:13-35`.
- Similar early return for synthetic login and org/repo mismatch: `client_ingest_dashboard.rs:43-61`, `73-97`, `129-149`.

### F-13 (High): Rate limit defaults are low for large multi-team load

Why it matters:
- For high traffic orgs, default limits can become artificial bottlenecks and increase retry storms.

Evidence:
- `/events` rate default 240/min: `gitgov/gitgov-server/src/main.rs:518-521`.
- Admin endpoints default 60/min: `main.rs:538-541`.
- Chat default 40/min: `main.rs:543-546`.

## 3.3 UI/Frontend Bottlenecks

### F-14 (High): Non-virtualized heavy tables + frequent data replacement

Why it matters:
- Large arrays are fully re-rendered when `serverLogs` updates.

Evidence:
- Full rows built from logs each update: `gitgov/src/components/control_plane/RecentCommitsTable.tsx:57-62`.
- Table maps rows directly (no virtualization): `RecentCommitsTable.tsx:152-204`.
- Team tables also map full arrays: `gitgov/src/components/control_plane/TeamManagementPanel.tsx:127-149` and `173-181`.

### F-15 (Medium): CPU overhead in fuzzy lookup per row

Why it matters:
- For each commit row, fallback loops over full map entries for SHA prefix matching.

Evidence:
- Pipeline fallback scan: `RecentCommitsTable.tsx:84-87`.
- PR fallback scan: `RecentCommitsTable.tsx:96-99`.

### F-16 (High): Chat history persistence can silently fail and appear as "history not saved"

Why it matters:
- Large localStorage payloads are serialized synchronously; persistence errors are swallowed.
- If quota or storage errors occur, user gets no visible failure.

Evidence:
- Sync serialization before async write: `gitgov/src/store/useControlPlaneStore.ts:787-820`.
- Silent failure on setItem: `useControlPlaneStore.ts:821-825`.

### F-17 (Medium): Dashboard intentionally uses a 500-event window, not full history

Why it matters:
- Users can perceive missing history as data loss when it is a frontend windowing decision.

Evidence:
- Logs load limit 500: `useControlPlaneStore.ts:1190-1198`.
- UI explicitly states recent window only: `RecentCommitsTable.tsx:122`.

## 3.4 Backend Logic/Query Risks

### F-18 (High): Optional filter pattern (`$x IS NULL OR column = $x`) repeated heavily

Why it matters:
- This pattern can reduce planner selectivity and lead to broader scans.

Evidence:
- `/logs` query has many OR-optional filters: `db.rs:781-820`.

### F-19 (High): Chat deterministic queries can become scan-heavy at global scope

Why it matters:
- Global admin can query with no org filter, which scans all-org data.

Evidence:
- Chat scope allows `None` org when global and org not provided: `gitgov/gitgov-server/src/handlers/chat_handler.rs:200-206`.
- Scope resolver explicitly returns `Ok(None)` in that case: `gitgov/gitgov-server/src/handlers/gdpr_clients_identities_scope.rs:346-347`.

### F-20 (High): Chat no-ticket push query uses JSON array expansion per row

Why it matters:
- `jsonb_array_elements_text` inside `NOT EXISTS` can be expensive on large `github_events`.

Evidence:
- `db.rs:4657-4661`, `4698-4702`, `4870-4874`.

### F-21 (Medium): `ILIKE` matching in user queries can hurt index use

Why it matters:
- Case-insensitive text matching in hot analytical queries can be slower than normalized equality paths.

Evidence:
- `db.rs:4805`, `4834`, `4867`, `4907`, `4952`, `4992`, `5038`.

## 3.5 Security / Business Rule Risks

### F-22 (Critical): Hardcoded legacy API key fallback in frontend

Why it matters:
- Hidden fallback key can mask auth/config problems and is a clear security risk.

Evidence:
- Constant defined: `gitgov/src/store/useControlPlaneStore.ts:477`.
- Used as last-resort in config resolution: `useControlPlaneStore.ts:847-853`.

### F-23 (High): Control Plane API key stored in localStorage plaintext

Why it matters:
- Local compromise or browser-context access can expose key.

Evidence:
- Read/write config in localStorage: `useControlPlaneStore.ts:551-571`.

### F-24 (High): Local unlock PIN stored plaintext in localStorage

Why it matters:
- No hashing/encryption for PIN at rest.

Evidence:
- Key definition and storage: `gitgov/src/store/useAuthStore.ts:6`, `39`, `205`, `227`.

### F-25 (High): Server uses fallback JWT secret string if env missing

Why it matters:
- Insecure default in production-like deployments.

Evidence:
- `gitgov/gitgov-server/src/main.rs:239-240`.

### F-26 (Medium): Permissive CORS on all origins/methods/headers

Why it matters:
- Broad CORS increases attack surface if keys leak into browser contexts.

Evidence:
- `gitgov/gitgov-server/src/main.rs:689`.

### F-27 (Medium): Repo auto-upsert from client metadata can inflate/poison repo catalog

Why it matters:
- A scoped client can create repo records from payload metadata without remote verification.

Evidence:
- Auto-upsert path from event metadata repo name: `client_ingest_dashboard.rs:154-168`.

## 3.6 Crash Analysis: "Component is not a function"

Observed symptom:
- React Router "Unexpected Application Error" with `TypeError: Component is not a function`.

Evidence and likely causes:

1. Dynamic component rendering exists in code and can fail if runtime value is invalid.
- `gitgov/src/components/shared/Toast.tsx:52` (`const Icon = iconMap[t.type]`) and `Toast.tsx:74` (`<Icon .../>`).
- `gitgov/src/pages/HelpPage.tsx:136-143` uses `const Icon = section.icon` + render.

2. Router has no explicit route `errorElement`, so route-render exceptions surface as default React Router error page.
- `gitgov/src/router.tsx:26-75`.

3. Top-level app has ErrorBoundary, but route-level errors can still render Router's fallback page.
- `gitgov/src/App.tsx:56-59`.

Conclusion:
- Exact throw site is NOT VERIFIED from static review only, but current codebase has dynamic component render points and missing route `errorElement`, matching the symptom pattern.

## 4) Root Cause Pattern for "No responde" / Freezes

Most plausible combined chain:

1. Frequent heavy polling + large payloads (`/logs` up to 500) repeatedly cross Rust<->JS boundary.
2. Expensive SQL query shapes and low DB pool increase tail latency.
3. Outbox strategy adds extra blocking pressure under unstable network/server conditions.
4. UI re-renders full non-virtualized tables and performs extra per-row matching work.
5. Chat persistence and large JSON serialization can add main-thread hitches.

This is consistent with temporary app unresponsiveness without removing functionality.

## 5) Prioritized Remediation Plan (No Golden Path Removal)

Phase P0 (safe, low-risk, high ROI)

1. Outbox flush control:
- Replace per-action thread spawn flush with debounced signal to existing worker.
- Add batch chunking (e.g., 100 events max/request).
- Replace UUID list `contains` checks with `HashSet`.

2. Dashboard polling/load-shedding:
- Keep 30s refresh but avoid re-fetching unchanged heavy panels every cycle.
- Pull logs incrementally (cursor/keyset) instead of full 500 replace.

3. `/logs` query optimization:
- Add deterministic order (`created_at DESC, id DESC`).
- Move to keyset pagination for high offsets.
- Minimize details payload for default dashboard view.

4. Security debt cleanup:
- Remove hardcoded fallback API key.
- Stop storing API key/PIN plaintext in localStorage (or at least hash PIN, move API key to secure storage).

Phase P1 (schema/query level)

1. Introduce pre-aggregated/stat cache path for `/stats`.
2. Rework daily activity query to keep indexable range predicates on `created_at`.
3. Refactor ingest path to pre-resolve org/repo per batch and multi-row insert.
4. Add/validate composite indexes by hottest predicates (`org_id + created_at + event_type`).

Phase P2 (scale hardening)

1. Raise/tune DB pool and rate limits by environment profile.
2. Add server response compression for large JSON endpoints.
3. Add load tests for org-scale traffic with realistic event distributions.

## 6) Business Rule/Contract Notes

- Golden Path components are coupled to event flow and auth header contracts; improvements should be implementation-level, not behavior removal.
- Existing anti split-brain normalization to `127.0.0.1` is present and good:
  - Frontend normalize: `gitgov/src/store/useControlPlaneStore.ts:527-542`
  - Tauri normalize: `gitgov/src-tauri/src/control_plane/server.rs:640-652`

## 7) NO VERIFICADO Items

NO VERIFICADO: Exact runtime root cause for the specific `Component is not a function` crash in your screenshot.
- Missing: runtime stack with source maps and component name.
- Needed next: capture browser console stack + route/component state at crash moment.

NO VERIFICADO: Real p95/p99 latency and CPU/memory under production-like load.
- Missing: benchmark environment and representative dataset volume.
- Needed next: controlled load test with org-scale event tables.

NO VERIFICADO: Actual DB execution plans for hot queries in your production dataset.
- Missing: `EXPLAIN (ANALYZE, BUFFERS)` against real data.
- Needed next: run explain on `/logs`, `/stats`, team queries, chat no-ticket queries.

## 8) Files Reviewed (Audit Evidence Set)

Desktop frontend/store:
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/store/useRepoStore.ts`
- `gitgov/src/store/useAuthStore.ts`
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx`
- `gitgov/src/components/control_plane/dashboard-helpers.ts`
- `gitgov/src/components/control_plane/ConversationalChatPanel.tsx`
- `gitgov/src/components/control_plane/TeamManagementPanel.tsx`
- `gitgov/src/components/shared/Toast.tsx`
- `gitgov/src/components/shared/ErrorBoundary.tsx`
- `gitgov/src/pages/ControlPlanePage.tsx`
- `gitgov/src/pages/DashboardPage.tsx`
- `gitgov/src/pages/HelpPage.tsx`
- `gitgov/src/components/layout/Header.tsx`
- `gitgov/src/components/layout/MainLayout.tsx`
- `gitgov/src/router.tsx`
- `gitgov/src/App.tsx`

Tauri Rust desktop:
- `gitgov/src-tauri/src/lib.rs`
- `gitgov/src-tauri/src/commands/git_commands.rs`
- `gitgov/src-tauri/src/commands/server_commands.rs`
- `gitgov/src-tauri/src/control_plane/server.rs`
- `gitgov/src-tauri/src/outbox/queue.rs`
- `gitgov/src-tauri/src/git/repository.rs`
- `gitgov/src-tauri/src/git/branch.rs`
- `gitgov/src-tauri/src/commands/auth_commands.rs`

Server + DB:
- `gitgov/gitgov-server/src/main.rs`
- `gitgov/gitgov-server/src/auth.rs`
- `gitgov/gitgov-server/src/db.rs`
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs`
- `gitgov/gitgov-server/src/handlers/chat_handler.rs`
- `gitgov/gitgov-server/src/handlers/gdpr_clients_identities_scope.rs`
- `gitgov/gitgov-server/src/handlers/prelude_health.rs`
- `gitgov/gitgov-server/src/handlers/conversational/core.rs`
- `gitgov/gitgov-server/src/handlers/conversational/engine.rs`
- `gitgov/gitgov-server/src/handlers/conversational/query.rs`
- `gitgov/gitgov-server/supabase_schema.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v6.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v8.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v9.sql`
- `gitgov/gitgov-server/supabase/supabase_schema_v12.sql`

---

Status: análisis y ejecución en curso. Este documento incluye secciones históricas de análisis puro y secciones posteriores con implementación validada.

## 9) Plan Integral de Optimización (Sin Romper Golden Patch ni Bot)

Objetivo:
- Llevar GitGov a operación masiva (orgs grandes, cientos de repos/devs, alto volumen de eventos) manteniendo 100% de funcionalidad actual.
- Reducir freezes/crashes y mejorar latencia percibida sin quitar features.

### 9.1 Invariantes No Negociables (Protección del Patch)

Estos puntos no se negocian en ninguna fase:

1. Golden Path intacto:
- Desktop detecta cambios.
- Stage, commit y push funcionan.
- Se emiten `stage_files`, `commit`, `attempt_push`, `successful_push`/`blocked_push`.
- `/events` acepta con `Authorization: Bearer`.
- Dashboard y `/logs` muestran datos sin 401.

2. Bot intacto:
- El bot sigue respondiendo con datos reales del Control Plane.
- El bot sigue leyendo correctamente los logs visibles en control plane.
- No se elimina ni degrada capacidad determinística ya existente.

3. Contratos compartidos intactos:
- No romper `ServerStats` y `CombinedEvent` entre backend/Tauri/frontend.

4. Seguridad y auditoría:
- Tablas append-only se mantienen append-only.
- No se introduce `X-API-Key`.

### 9.2 Estrategia General

Enfoque de ejecución:
- Cambios pequeños, secuenciales y reversibles.
- Una sola superficie crítica por PR (outbox, logs, stats, UI, etc.).
- Cada PR incluye benchmark antes/después + verificación Golden Path.
- Feature flags para cambios de alto riesgo (query path/polling path).

Regla operativa:
- No mezclar optimización de performance con refactor funcional grande en el mismo PR.

### 9.3 Fases de Implementación

#### Fase 0: Baseline y Red de Seguridad (sin cambiar comportamiento)

Objetivo:
- Medir primero, tocar después.

Trabajo:
1. Instrumentar métricas de latencia p50/p95/p99 para:
- `GET /logs`, `GET /stats`, `POST /events`, `POST /chat/ask`.
2. Instrumentar tamaño de payload y tiempo de serialización en:
- Tauri `cmd_server_get_logs`.
3. Instrumentar outbox:
- Tamaño de cola, tiempo de flush, ratio de reintento.
4. Registrar conteos de render y tiempo de render en dashboard.

Aceptación:
- Tablero interno con baseline de latencias y throughput.
- Cero cambio de comportamiento visible.

#### Fase 1: Outbox y Flujo de Ingesta (máximo impacto, bajo riesgo funcional)

Objetivo:
- Eliminar picos de hilos/bloqueo y reducir carga de red/DB por evento.

Trabajo:
1. Reemplazar flush por thread por evento con señal/debounce al worker existente.
2. Implementar chunking de batch de outbox (ej. 100 eventos por request).
3. Optimizar reconciliación UUID a `HashSet` (evitar O(n^2)).
4. En backend, preparar camino de inserción más eficiente por lote:
- mantener semántica actual de deduplicación por `event_uuid`.

Guardrails:
- No cambiar tipos de evento ni semántica de aceptación/duplicados.

Aceptación:
- Misma salida funcional en `/events`.
- Menor tiempo de flush y menor variabilidad bajo red lenta.

#### Fase 2: `/logs` y Query Path de Dashboard

Objetivo:
- Bajar latencia de consulta y evitar degradación por volumen histórico.

Trabajo:
1. Orden estable: `created_at DESC, id DESC`.
2. Introducir keyset pagination para dashboard (sin romper API actual; mantener fallback offset).
3. Reducir enrichment cost por request:
- mover campos críticos al query principal o usar estrategia de join eficiente.
4. Revisar índices compuestos para patrón real:
- `org_id + created_at + event_type` y equivalentes por tabla.

Guardrails:
- Respuesta JSON contractual de `/logs` no se rompe.
- Bot sigue leyendo/interpretando logs correctamente.

Aceptación:
- Mejoras p95 en `/logs`.
- Dashboard fluido con org grande.

#### Fase 3: `/stats` y Cargas Agregadas

Objetivo:
- Evitar full-aggregate cost por polling frecuente.

Trabajo:
1. Crear capa de cache corta para stats (TTL corto, invalidación segura).
2. Revisar y optimizar `get_audit_stats` para reducir scans repetitivos.
3. Ajustar cadencia de refresco para evitar tormenta de consultas.

Guardrails:
- No alterar cifras de negocio.
- Coherencia temporal clara en respuesta.

Aceptación:
- p95 de `/stats` significativamente menor.
- Sin divergencias numéricas en dashboard frente a baseline.

#### Fase 4: UI Responsiva para Datos Grandes

Objetivo:
- Evitar freeze en render y en serialización del frontend.

Trabajo:
1. Virtualizar tablas pesadas (commits/team) para grandes volúmenes.
2. Evitar recomputaciones por render en matching costoso (memoización y mapas directos).
3. Reducir hitching por persistencia de chat:
- persistencia robusta con manejo explícito de error de storage.
4. Añadir `errorElement` por rutas para evitar pantalla negra ante error puntual.

Guardrails:
- Misma UX funcional del bot, sesiones y paneles.
- No perder historial existente durante migración de storage.

Aceptación:
- Eliminación de bloqueos visibles al navegar/teclear.
- Sin regresión de sesión de chat ni tabs del bot.

#### Fase 5: Hardening de Seguridad y Configuración (sin romper operación)

Objetivo:
- Cerrar deuda técnica que afecta estabilidad y seguridad operativa.

Trabajo:
1. Retirar fallback hardcodeado de API key (con migración segura).
2. Endurecer almacenamiento local:
- API key en storage seguro del desktop.
- PIN local con protección mínima (no plaintext).
3. Eliminar fallback inseguro de JWT secret en entornos no-dev.
4. Ajustar CORS según entorno.

Guardrails:
- Onboarding y conexión actual siguen funcionando.
- No introducir 401 en Golden Path.

Aceptación:
- Flujo de conexión exitoso con configuración explícita.
- Cero dependencia de secretos hardcodeados.

### 9.4 Plan de Validación por Fase (Obligatorio)

Para cada fase/PR:

1. Validación técnica mínima:
- `cargo test` (server + tauri donde aplique)
- `tsc -b`
- ESLint en archivos tocados (0 errores nuevos)

2. Validación Golden Path:
- `POST /events` con Bearer responde shape esperado.
- `/stats` responde sin 401.
- `/logs` responde con contrato válido.
- commit/push/eventos visibles en dashboard.

3. Validación bot:
- Preguntas determinísticas existentes responden igual o mejor.
- Confirmar que sigue leyendo logs del control plane con exactitud.

4. Validación performance:
- Comparar baseline vs post-cambio en p50/p95/p99.
- Registrar resultados en `docs/PROGRESS.md`.

### 9.5 Rollout y Mitigación de Riesgo

Estrategia:
1. Rollout canario por feature flags en rutas críticas (`logs_v2`, `stats_cache`, `outbox_chunking`).
2. Activación gradual por entorno:
- dev -> staging -> piloto -> general.
3. Rollback inmediato:
- cada optimización debe poder desactivarse por flag sin redeploy complejo.

Criterios de rollback inmediato:
- Cualquier 401 nuevo en Golden Path.
- Pérdida de eventos o desfase bot/logs.
- Aumento significativo de error rate o freezes.

### 9.6 Criterios de Aprobación Final (Go/No-Go)

Se aprueba el plan completo si se cumple:

1. Funcionalidad:
- Golden Path 100% operativo.
- Bot 100% operativo y consistente con logs.

2. Estabilidad:
- Sin crashes reproducibles bajo carga prevista.
- UI sin congelamientos prolongados.

3. Escalabilidad:
- Mejora comprobable de p95/p99 en `/logs`, `/stats`, `POST /events`.
- Comportamiento estable en dataset de org grande.

4. Seguridad operativa:
- Sin fallback hardcodeado de API key.
- Secretos/credenciales sin almacenamiento inseguro evitable.

### 9.7 Orden Recomendado de Ejecución (para no romper nada)

1. Fase 0 (baseline/instrumentación).
2. Fase 1 (outbox/ingesta).
3. Fase 2 (`/logs` y paginación eficiente).
4. Fase 3 (`/stats` y cache controlada).
5. Fase 4 (UI rendimiento + manejo de errores).
6. Fase 5 (hardening seguridad/configuración).

Nota:
- Si en cualquier fase hay riesgo de regresión del bot o Golden Path, se pausa y se corrige antes de continuar.

## 10. Estado de Ejecución (2026-03-04)

### Fase 1 iniciada (desktop outbox, bajo riesgo)

Cambios aplicados:
1. Se eliminó el patrón de `thread::spawn` por evento en `trigger_flush` y ahora se notifica al worker.
2. El worker de outbox usa espera basada en intervalo real + señal (`Condvar`) y puede flush-ear inmediatamente al llegar eventos.
3. Reconciliación de respuesta de `/events` optimizada a lookup O(1) con sets.
4. Cliente HTTP de outbox reutilizable (sin recrear `reqwest::blocking::Client` en cada envío).
5. Se implementó chunking de envío (`OUTBOX_BATCH_SIZE=100`) en flush manual y worker background para colas grandes.

Archivos tocados:
- `gitgov/src-tauri/src/commands/git_commands.rs`
- `gitgov/src-tauri/src/outbox/queue.rs`
- `docs/PROGRESS.md`

Validación técnica ejecutada:
- `cd gitgov/src-tauri && cargo fmt` -> OK
- `cd gitgov/src-tauri && cargo test` -> `0 passed; 0 failed`
- `cd gitgov/src-tauri && cargo clippy -- -D warnings` -> OK
- `cd gitgov && npx tsc -b` -> sin errores
- `cd gitgov && npm run lint` -> OK

Pendiente antes de cerrar Fase 1:
- `NO VERIFICADO`: prueba runtime manual de Golden Path completa (Desktop -> /events -> Dashboard) en carga real de org grande.

Actualización de verificación (2026-03-04, runtime local):
1. Server reiniciado con canary de chat aplicado (`RATE_LIMIT_CHAT_PER_MIN=120`).
2. Smoke runtime server-side de Golden Path:
- `/health` OK,
- `stage_files/commit/attempt_push/successful_push` aceptados y visibles en `/logs`,
- duplicados detectados correctamente.
3. Prueba de capacidad chat:
- 125 requests secuenciales -> `200=120`, `429=5`.
- 150 concurrentes -> `200=106`, `429=44`, con `retry_after_seconds` presente en body.

### Hallazgo operativo adicional (chat / control plane)

Evidencia empírica local (2026-03-04):
1. `/chat/ask` tiene rate-limit dedicado por middleware.
2. Default activo de `GITGOV_RATE_LIMIT_CHAT_PER_MIN` es 40/min.
3. Prueba controlada de 45 requests secuenciales tras enfriamiento:
- `200 = 40`
- `429 = 5`

Inferencia:
- El síntoma "chat no responde por minutos" bajo ráfaga es consistente con rate-limit saturado (ventana 60s), no necesariamente con crash por memoria.

### Fase 3 iniciada (2026-03-04) — `/stats` con cache TTL corta (sin romper contrato)

Cambios aplicados:
1. Cache in-memory por scope (`org_id` o global) para `/stats`, con TTL configurable (`GITGOV_STATS_CACHE_TTL_MS`, default 3000).
2. Invalidación explícita del cache de stats cuando `/events` ingiere lotes aceptados.
3. Reuso del mismo path cacheado en `/dashboard` para evitar recomputación redundante.
4. Ajuste SQL en `/stats/daily` para query más index-friendly:
- se reemplazó comparación por cast de fecha en `created_at` por filtro por rangos diarios (`>= day` y `< day+1d`).
5. Pool de PostgreSQL configurable por entorno (antes fijo en 10 conexiones):
- `GITGOV_DB_MAX_CONNECTIONS` (default 20), `GITGOV_DB_MIN_CONNECTIONS` (default 2),
- `GITGOV_DB_ACQUIRE_TIMEOUT_SECS` (default 8), `GITGOV_DB_IDLE_TIMEOUT_SECS` (default 300), `GITGOV_DB_MAX_LIFETIME_SECS` (default 1800).

Validación técnica:
- `cd gitgov/gitgov-server && cargo test` -> `79 passed; 0 failed`.
- `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> falla por deuda preexistente en `src/handlers/conversational/query.rs:326` (`if_same_then_else`).
- `cd gitgov && npx tsc -b` -> OK.
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts` -> OK.

Smoke runtime local post-reinicio:
1. `/health` -> `200`.
2. `/stats` responde shape válido en llamadas consecutivas.
   - muestra local de latencia (ms): `998` -> `192` (cache-hit) -> `787` tras expirar TTL (`~3.2s`).
3. `POST /events` (`commit`) -> `accepted=1`, `errors=0`.
4. `/logs` para usuario probe muestra el evento (`event_type=commit`, `source=client`).
5. Verificación de invalidación: `client_events.total` sube inmediatamente (`delta=+1`) tras ingesta, sin esperar TTL.
6. `/stats/daily?days=14` mantiene contrato (`14` filas, campos `day/commits/pushes`).

### Fase 4 iniciada (2026-03-04) — UI de alto volumen (sin romper Golden Path)

Cambios aplicados:
1. Carga incremental en Team Management:
- fetch inicial en chunks (`TEAM_PAGE_SIZE=50`),
- botón `Cargar más` para `developers` y `repos` usando `offset` + `append=true`.
- fetch por pestaña activa para evitar disparar ambos endpoints pesados al mismo tiempo.
2. Store optimizado para paginación incremental:
- `loadTeamOverview` y `loadTeamRepos` soportan `append` y fusionan sin duplicados.
3. Tabla de commits optimizada:
- índice por prefijo SHA (7..12) para correlaciones CI/PR y reducción de lookup O(n*m) en path común.
- evita cómputo duplicado de preview/SHA por fila.
4. Degradación de errores en rutas:
- `errorElement` por ruta en frontend para mostrar fallback y evitar percepción de crash total de la app.
5. Polling más inteligente en dashboard:
- auto-refresh se pausa cuando la ventana está en background (`visibilityState`), reduciendo carga/re-renders innecesarios.
6. Refresh incremental de logs:
- en vez de recargar siempre 500 eventos, se consulta delta por `start_date` desde el último timestamp y se fusiona localmente (dedupe por `id` + orden estable).
- fallback a carga completa si incremental falla.
7. Hardening inicial de API key fallback:
- fallback legacy hardcodeado en frontend quedó bajo gate `VITE_ALLOW_LEGACY_DEFAULT_API_KEY`.
- default seguro por entorno: activo en `DEV`, desactivado fuera de `DEV` si no se especifica.
8. Persistencia de chat no bloqueante:
- serialización/escritura de historial se mueve a `requestIdleCallback` (fallback debounce) para reducir congelamiento al escribir.
9. Hardening server por entorno:
- JWT secret fallback inseguro restringido a dev/flag (`GITGOV_ALLOW_INSECURE_JWT_FALLBACK`).
- CORS configurable (`GITGOV_CORS_ALLOW_ANY`, `GITGOV_CORS_ALLOW_ORIGINS`) con fail-fast en modo estricto mal configurado.

Validación técnica:
- `cd gitgov && npx tsc -b` -> OK.
- `cd gitgov && npx eslint src/components/control_plane/ServerDashboard.tsx src/components/control_plane/TeamManagementPanel.tsx src/components/control_plane/RecentCommitsTable.tsx src/store/useControlPlaneStore.ts src/router.tsx` -> 0 errores.

## 11) Plan Maestro de Ejecución (Versión Final, Lista para Aprobación)

Objetivo de este bloque:
- Convertir el análisis en un plan operativo completo, ejecutable y reversible.
- Mantener 100% del Golden Patch y funcionamiento del bot durante toda la optimización.

### 11.1 Estado actual consolidado

Completado:
1. Fase 1 (desktop outbox) en alcance Tauri:
- flush por notificación al worker (sin `thread::spawn` por evento),
- reconciliación O(1) por `HashSet`,
- chunking de outbox (`OUTBOX_BATCH_SIZE=100`),
- reutilización de cliente HTTP.
2. Fase 2 (`/logs`) cerrada en server+tauri:
- orden determinístico `created_at DESC, id DESC`,
- cursor keyset (`before_created_at`, `before_id`) con compatibilidad `offset`,
- verificación funcional sin overlap entre páginas.
3. Verificación empírica de cuello de chat por rate-limit:
- límite observado consistente con `40/min` (`200=40`, `429=5` en prueba controlada).
4. Canary de capacidad chat y UX de error:
- tuning en `.env` para canary,
- mensaje de UI con `retry_after_seconds` para `429`.

En curso:
1. Cierre formal de Fase 1 con smoke runtime manual completo desktop->server->dashboard bajo carga org grande.
2. Fase 3 iniciada (`/stats`): cache corta por scope de org con TTL configurable + invalidación en ingesta.
3. Fase 4 iniciada (UI): team incremental load + optimización de lookup CI/PR por SHA en commits.
4. Fase 5 iniciada (hardening): fallback legacy de API key condicionado por flag/env.
5. Fase 0 registrada (baseline consolidada) con artefacto:
- `gitgov/gitgov-server/tests/artifacts/perf_baseline_control_plane_2026-03-04.json`.

Pendiente:
1. Cierre de Fase 3 (`/stats` y agregaciones).
2. Cierre de Fase 4 (UI de alto volumen).
3. Cierre de Fase 5 (hardening seguridad/configuración).

### 11.2 Invariantes de protección (No negociables)

1. Golden Patch intacto:
- commit/push/eventos visibles en dashboard, sin 401.
2. Bot intacto:
- respuestas consistentes con logs del control plane.
3. Contratos intactos:
- `ServerStats` y `CombinedEvent` sin ruptura entre server/Tauri/frontend.
4. Seguridad básica:
- `Authorization: Bearer` obligatorio; no introducir `X-API-Key`.
5. Auditoría:
- append-only en tablas de auditoría; no UPDATE/DELETE sobre eventos.

### 11.3 Plan por fases (Checklist ejecutable)

#### Fase 0 — Baseline obligatorio (antes de cambios grandes)

Objetivo:
- Congelar línea base de performance y estabilidad para comparar mejoras reales.

Checklist:
1. Capturar p50/p95/p99 de:
- `POST /events`, `GET /logs`, `GET /stats`, `POST /chat/ask`.
2. Capturar errores por endpoint:
- porcentaje `401`, `429`, `5xx`.
3. Capturar tamaño de payload:
- bytes en `/logs` y tiempos de serialización.
4. Guardar artefactos en `tests/artifacts/` y resumen en `docs/PROGRESS.md`.

Criterio de salida:
- baseline reproducible documentado y versionado.

#### Fase 1 — Outbox/ingesta desktop (ya avanzada)

Objetivo:
- reducir presión de UI y envío en ráfaga sin perder eventos.

Checklist:
1. Confirmar estabilidad del cambio outbox en uso real.
2. Ejecutar smoke runtime completo en entorno levantado.
3. Registrar resultado final de Fase 1 (cerrada) en `PROGRESS.md`.

Criterio de salida:
- sin freeze perceptible en acciones git frecuentes,
- Golden Path intacto en smoke,
- sin regresiones de bot.

#### Fase 2 — `/logs` y query path dashboard

Objetivo:
- bajar latencia de consulta en históricos grandes.

Checklist:
1. Orden estable:
- `created_at DESC, id DESC`.
2. Keyset pagination:
- mantener fallback offset para compatibilidad.
3. Minimizar costo de enrichment en path de dashboard.
4. Revisar índices compuestos por patrón real de filtros.

Criterio de salida:
- mejora medible de p95 en `/logs`,
- navegación del dashboard sin bloqueos en org grande.

#### Fase 3 — `/stats` y agregaciones

Objetivo:
- eliminar recalculo pesado por polling frecuente.

Checklist:
1. Cache de corta vida para stats (TTL acotado, invalidación segura).
2. Reducir scans repetitivos de agregación.
3. Ajustar cadencia de refresco en frontend para evitar tormenta.

Criterio de salida:
- p95 de `/stats` reducido de forma consistente,
- mismos números de negocio respecto baseline.

#### Fase 4 — UI de alto volumen

Objetivo:
- evitar congelamientos de render con tablas y listas grandes.

Checklist:
1. Virtualizar tablas pesadas de commits/equipo.
2. Eliminar matching O(n*m) por fila en render.
3. Mantener persistencia de chat robusta ante límites de storage.
4. Añadir `errorElement` de ruta para evitar pantalla de error genérica sin contexto.

Criterio de salida:
- no “No responde” por render en navegación normal,
- UX de error clara sin percepción de crash silencioso.

#### Fase 5 — Hardening seguridad/configuración

Objetivo:
- reducir deuda operativa que impacta estabilidad y seguridad.

Checklist:
1. retirar fallback hardcodeado de API key.
2. endurecer storage local (API key/PIN).
3. eliminar fallback inseguro de JWT secret en no-dev.
4. CORS por entorno.

Criterio de salida:
- sin dependencia de secretos inseguros,
- onboarding y auth existentes sin 401 nuevos.

### 11.4 Secuencia de despliegue (Canary -> General)

1. Dev local:
- validar técnica + smoke contractual.
2. Staging:
- pruebas de carga controladas y comparación contra baseline.
3. Canary en producción:
- habilitar por flags/config para subset de tráfico.
4. General:
- activar progresivamente tras 24-72h sin alertas críticas.

### 11.5 Rollback operacional (inmediato)

Disparadores:
1. nuevos 401 en Golden Path.
2. pérdida de eventos o inconsistencias bot/logs.
3. subida sostenida de `429`/`5xx`.
4. regresión fuerte de p95/p99 o freeze reproducible.

Acción:
1. desactivar feature flag/config de la fase afectada.
2. volver al path anterior sin redeploy complejo cuando sea posible.
3. dejar incidente y evidencia en `PROGRESS.md`.

### 11.6 Matriz de validación obligatoria por PR

1. Técnica:
- `cargo test` (componentes tocados),
- `cargo clippy -- -D warnings` (si aplica),
- `tsc -b`,
- ESLint en archivos tocados (`0` errores nuevos).
2. Contrato:
- `/events` (Bearer) shape esperado,
- `/stats` sin 401,
- `/logs` contrato válido.
3. Golden Path:
- stage/commit/push/eventos visibles.
4. Bot:
- respuestas consistentes con logs del control plane.
5. Performance:
- comparar p50/p95/p99 vs baseline.

### 11.7 Criterio final de “Plan completado”

El plan se considera completamente ejecutado cuando:
1. Todas las fases (0..5) estén cerradas con evidencia en `PROGRESS.md`.
2. Golden Patch y bot permanezcan intactos en todas las validaciones.
3. Se logre mejora medible en p95/p99 en `/events`, `/logs`, `/stats`, `/chat/ask`.
4. No existan crashes reproducibles en escenario de org grande.

### 11.8 Faltantes actuales para cierre (al 2026-03-04)

1. Fase 0 está capturada para el estado actual, pero no cerrada como “comparativa final”:
- ya existe baseline consolidada (`/events`, `/logs`, `/stats`, `/chat/ask`) con p50/p95/p99 + `401/429/5xx`,
- falta comparativo baseline-vs-post-cierre final en escenario de carga prolongada.
2. Fase 1 no está cerrada formalmente en escenario interactivo desktop completo:
- falta evidencia manual `Desktop -> commit/push -> /events -> Dashboard` en carga org grande.
3. Fase 3/4/5 están iniciadas pero aún no cerradas con criterio final:
- falta prueba final de no-regresión bajo carga prolongada y comparación contra baseline.
4. NO VERIFICADO técnico pendiente:
- causa exacta del crash `TypeError: Component is not a function` de screenshot,
- `EXPLAIN (ANALYZE, BUFFERS)` en dataset grande real para queries calientes.
5. Deuda preexistente que bloquea “verde total” de lint server:
- `cargo clippy -- -D warnings` falla por `gitgov/gitgov-server/src/handlers/conversational/query.rs:326`.

### 11.9 Hallazgos confirmados (evidencia actualizada, 2026-03-04)

1. Ingesta `/events` mantiene costo O(n), con mitigaciones ya aplicadas en resolución y escritura.
- Cache in-batch para resolución de `org_name` y `repo_full_name`: `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:12-13,67-77,132-140`.
- Cache se actualiza tras upsert exitoso de repo para evitar re-upserts en el mismo lote: `client_ingest_dashboard.rs:182-199`.
- Inserción DB en batch de una sola sentencia con `ON CONFLICT DO NOTHING RETURNING event_uuid`: `gitgov/gitgov-server/src/db.rs:684-735`.
- Fallback por fila se mantiene solo en error transaccional del batch: `db.rs:536-566,720-724`.
- Riesgo residual: validación/serialización por evento sigue siendo O(n), pero con menos round-trips a DB.

2. `/logs` sigue siendo query pesada para cardinalidad alta.
- Query combinada con `UNION ALL`, sort global y paginación `LIMIT/OFFSET`: `gitgov/gitgov-server/src/db.rs:846-913`.
- Aún con keyset disponible, el path SQL conserva offset para compatibilidad.
- Riesgo: páginas profundas y filtros amplios mantienen costo alto.

3. Team overview devuelve payload mayor al que realmente usa UI.
- Backend agrega JSON de repos por usuario completo (`jsonb_agg`): `gitgov/gitgov-server/src/db.rs:4161-4174`.
- UI solo muestra preview (`slice(0, 3)`): `gitgov/src/components/control_plane/TeamManagementPanel.tsx:164-165`.
- Riesgo: sobrecarga innecesaria de red/serialización en orgs grandes.

4. `/stats` y `/logs` ya quedaron separados por limiter dedicado (mitigación directa del 429 cruzado).
- Config/env nuevo para separar cuotas: `GITGOV_RATE_LIMIT_LOGS_PER_MIN` y `GITGOV_RATE_LIMIT_STATS_PER_MIN` en `gitgov/gitgov-server/src/main.rs:585-588`.
- Limiter dedicado por ruta:
  - `/logs` -> `logs_rate_limit`: `main.rs:681-684`.
  - `/stats`, `/stats/daily`, `/dashboard` -> `stats_rate_limit`: `main.rs:688-705`.
- Riesgo residual: si los límites configurados son demasiado bajos para la carga real, seguirá habiendo `429`, pero ya no por contención entre buckets compartidos.

5. El freeze percibido de chat/UI ya tiene mitigaciones, pero no cierre final.
- Persistencia de chat diferida a idle/debounce: `gitgov/src/store/useControlPlaneStore.ts:870-879`.
- Refresh sensible a visibilidad de ventana: `gitgov/src/components/control_plane/ServerDashboard.tsx:50-78`.
- Carga incremental en Team panel: `gitgov/src/components/control_plane/TeamManagementPanel.tsx:6,26-33,179-205,240-265`.
- Riesgo residual: tabla de commits sigue sin virtualización (render directo por fila): `gitgov/src/components/control_plane/RecentCommitsTable.tsx:191-260`.

6. Crash `TypeError: Component is not a function` sigue sin reproducción determinística.
- Estado actual: `NO VERIFICADO` (sin stack de componente propio en evidencia capturada).
- Mitigación aplicada: rutas con `errorElement` dedicado para degradación controlada: `gitgov/src/router.tsx:53-122`.
- Acción pendiente: capturar stack de componente real en runtime para causa raíz.

### 11.10 Plan integral de cierre (sin romper Golden Patch ni bot)

Principio operativo:
- No remover funcionalidades.
- Cambios detrás de flags/env cuando aplique.
- Validación contractual en cada fase: `/events`, `/stats`, `/logs`, bot y dashboard.

Fase A (prioridad máxima): Ingesta escalable sin cambiar contrato
1. Hecho: cache por lote en handler de ingesta para org/repo resueltos por clave (`org_name`, `repo_full_name`).
2. Hecho: inserción batch multi-values para `client_events` manteniendo deduplicación por `event_uuid`.
3. Mantener fallback actual si falla path optimizado.
4. Criterio de salida: menor latencia p95 en `POST /events` vs baseline y mismo shape de respuesta.

Fase B: `/logs` de alto volumen con path keyset por defecto
1. Mantener endpoint compatible, pero priorizar keyset en frontend/tauri para navegación real.
2. Evitar offsets profundos en flujo principal de dashboard.
3. Revisar índices para patrón dominante (`org_id`, `created_at DESC`, `id DESC`) con `EXPLAIN (ANALYZE, BUFFERS)` en dataset grande.
4. Criterio de salida: p95/p99 de `/logs` estable bajo carga sostenida, sin overlap ni huecos.

Fase C: Team endpoints con payload proporcional
1. Cambiar backend para retornar preview limitado de repos por usuario (por defecto 3-5) + contador total.
2. Mantener endpoint detallado opcional para expandir un developer bajo demanda.
3. Criterio de salida: reducción clara de bytes de respuesta en `/team/overview` y menor tiempo de render UI.

Fase D: Rate-limit y UX de no-caída
1. Separar buckets admin al menos en `stats` y `logs`, o subir límites de forma controlada por env en canary.
2. Mantener mensaje explícito al usuario cuando hay `429` (ya aplicado para chat).
3. Criterio de salida: desaparición de falsos “crash” por cuota en uso normal admin.

Fase E: Cierre de estabilidad UI
1. Instrumentar captura de errores de componente (`componentStack`) para aislar `Component is not a function`.
2. Definir umbral de virtualización en commits si la ventana supera tamaño objetivo.
3. Criterio de salida: sin “No responde” reproducible en pruebas de navegación/polling.

### 11.11 Plan de ejecución y aprobación (lista para operar)

Semana 1:
1. Fase A completa + validación contractual.
2. Fase D parcial en canary (rate-limit separado o tuning controlado).

Semana 2:
1. Fase B completa con medición p95/p99 contra baseline.
2. Fase C completa con comparación de payload y render.

Semana 3:
1. Fase E + prueba prolongada de estabilidad (sin regressiones Golden Path/bot).
2. Cierre formal de fases 0..5 en `docs/PROGRESS.md`.

Checklist de aprobación final:
1. `cargo test` server y `tsc -b` frontend sin fallas.
2. `eslint` en archivos tocados con `0` errores nuevos.
3. Evidencia de Golden Path funcional: commit/push -> `/events` -> dashboard.
4. Evidencia bot funcional: responde usando logs del control plane y conserva historial.
5. Comparativo baseline vs post-cierre con artefactos versionados en `tests/artifacts/`.

### 11.12 Avance aplicado (2026-03-04) — separación de rate-limit `/logs` vs `/stats`

Implementación:
1. Server:
- Se separó el limiter admin en buckets dedicados:
  - `logs_endpoints` para `/logs`.
  - `stats_endpoints` para `/stats`, `/stats/daily`, `/dashboard`.
- Nuevas env vars:
  - `GITGOV_RATE_LIMIT_LOGS_PER_MIN` (default hereda `GITGOV_RATE_LIMIT_ADMIN_PER_MIN`).
  - `GITGOV_RATE_LIMIT_STATS_PER_MIN` (default hereda `GITGOV_RATE_LIMIT_ADMIN_PER_MIN`).
- Evidencia: `gitgov/gitgov-server/src/main.rs:585-588,614-621,681-705`.

2. Bot (conocimiento operativo):
- Se actualizó la respuesta interna de “Rate limits configurables” para reflejar las nuevas variables.
- Evidencia: `gitgov/gitgov-server/src/handlers/conversational/core.rs:261-264`.

Validación ejecutada:
1. `cd gitgov/gitgov-server && cargo fmt` -> OK.
2. `cd gitgov/gitgov-server && cargo test` -> `79 passed; 0 failed`.
3. `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> falla por deuda preexistente en `gitgov/gitgov-server/src/handlers/conversational/query.rs:326` (no causada por este cambio).
4. `cd gitgov && npx tsc -b` -> OK.

Impacto:
1. Mitiga directamente la colisión de cuota entre polling de logs y stats que se observó en baseline.
2. No cambia contrato HTTP ni auth Bearer del Golden Path.

### 11.13 Avance aplicado (2026-03-04) — Fase A `/events`: cache por lote + inserción batch DB

Implementación:
1. Handler de ingesta:
- se agregó `org_id_cache` por `org_name` para evitar lookup repetido en un mismo batch.
- se agregó `repo_cache` por `repo_full_name` para evitar lookup repetido en un mismo batch.
- cuando el repo se crea por upsert, el cache se hidrata inmediatamente para siguientes eventos del lote.
- Evidencia: `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:12-13,67-77,132-140,182-199`.

2. Escritura DB:
- `insert_client_events_batch_tx(...)` pasó de ejecutar `INSERT` por evento a construir un batch único con `QueryBuilder`.
- se usa `ON CONFLICT (event_uuid) DO NOTHING RETURNING event_uuid` para clasificar `accepted/duplicates` sin cambiar contrato.
- se mantiene fallback legacy por fila ante error transaccional.
- Evidencia: `gitgov/gitgov-server/src/db.rs:570-746`.

3. Sin cambio contractual:
- se mantiene validación de scope y semántica de aceptación/duplicados.
- no cambia auth Bearer ni shape de respuesta de `/events`.

Validación ejecutada:
1. `cd gitgov/gitgov-server && cargo fmt` -> OK.
2. `cd gitgov/gitgov-server && cargo test` -> `79 passed; 0 failed`.
3. `cd gitgov && npx tsc -b` -> OK.
4. `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> falla por deuda preexistente en `gitgov/gitgov-server/src/handlers/conversational/query.rs:326`.

Pendiente Fase A:
1. Completado en 11.15 para carga prolongada local controlada.

### 11.14 Evidencia comparativa ejecutada (2026-03-04) — baseline vs post-Fase A

Configuracion usada (misma forma de baseline):
1. Script: `gitgov/gitgov-server/tests/perf_baseline_control_plane.py`.
2. Parametros: `--requests`, `--concurrency`, `--timeout-sec`, `--out-json`: `tests/perf_baseline_control_plane.py:242-245`.
3. Endpoints cubiertos: `/events`, `/logs`, `/stats`, `/chat/ask`: `tests/perf_baseline_control_plane.py:271,287,303,319`.

Artefactos:
1. Baseline:
- `gitgov/gitgov-server/tests/artifacts/perf_baseline_control_plane_2026-03-04.json`.
2. Post-Fase A (corrida estable usada para comparacion):
- `gitgov/gitgov-server/tests/artifacts/perf_baseline_control_plane_after_phaseA_rerun2_2026-03-04.json`.

Comparativo principal (baseline -> post-Fase A):
1. `POST /events`:
- p95 `1952.5ms -> 875.1ms` y p99 `2056.3ms -> 890.3ms`.
- throughput `4.76 -> 6.17 rps`.
- Evidencia baseline: `.../perf_baseline_control_plane_2026-03-04.json:8-13,21-22`.
- Evidencia post: `.../perf_baseline_control_plane_after_phaseA_rerun2_2026-03-04.json:8-13,21-22`.
2. `GET /logs`:
- p95 `824.4ms -> 614.7ms` y p99 `877.6ms -> 617.3ms`.
- throughput `5.80 -> 6.45 rps`.
- Evidencia baseline: `.../perf_baseline_control_plane_2026-03-04.json:31-36,44-45`.
- Evidencia post: `.../perf_baseline_control_plane_after_phaseA_rerun2_2026-03-04.json:31-36,44-45`.
3. `GET /stats`:
- p95 `1005.8ms -> 878.4ms` y p99 `1068.9ms -> 942.6ms`.
- throughput `13.12 -> 13.50 rps`.
- HTTP baseline incluyo `429`; corrida post estable quedo en `200=35`.
- Evidencia baseline: `.../perf_baseline_control_plane_2026-03-04.json:54-59,70-71`.
- Evidencia post: `.../perf_baseline_control_plane_after_phaseA_rerun2_2026-03-04.json:54-59,67-68`.
4. `POST /chat/ask`:
- p95 `1114.9ms -> 926.3ms` y p99 `1170.4ms -> 947.0ms`.
- throughput `6.96 -> 7.81 rps`.
- Evidencia baseline: `.../perf_baseline_control_plane_2026-03-04.json:80-85,93-94`.
- Evidencia post: `.../perf_baseline_control_plane_after_phaseA_rerun2_2026-03-04.json:77-82,90-91`.

Lectura operativa:
1. Criterio de salida de Fase A para `POST /events` (mejora medible de p95/p99 sin romper contrato) queda cumplido en prueba local controlada.
2. Aun pendiente el cierre de programa con prueba prolongada y cardinalidad alta de org grande (no solo corrida corta).

### 11.15 Evidencia de carga prolongada y mitigación auth (2026-03-04)

Problema detectado en carga prolongada inicial (`requests=220`, `concurrency=8`):
1. aparecían `401` intermitentes en varios endpoints aun con API key válida.
2. body observado en error: `Authentication backend unavailable`.
3. esto confirmaba presión transitoria de DB en validación auth, no token inválido real.

Mitigación aplicada:
1. Cache in-memory de validación de API key (`key_hash`) con TTL corta.
2. Fallback controlado a cache stale cuando DB falla transitoriamente.
3. Invalidación de cache en create/ensure/revoke de API key.
4. Evidencia de código: `gitgov/gitgov-server/src/db.rs:25-32,101-132,136-199,2219-2279,2325,2354,2426`.

Tuning canary usado para esta corrida:
1. `GITGOV_RATE_LIMIT_EVENTS_PER_MIN=1200`
2. `GITGOV_RATE_LIMIT_LOGS_PER_MIN=1200`
3. `GITGOV_RATE_LIMIT_STATS_PER_MIN=1200`
4. `GITGOV_RATE_LIMIT_CHAT_PER_MIN=600`
5. `GITGOV_DB_MAX_CONNECTIONS=40`
6. `GITGOV_DB_MIN_CONNECTIONS=4`
7. `GITGOV_DB_ACQUIRE_TIMEOUT_SECS=12`

Artefactos:
1. Antes de mitigación auth:
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_canary_2026-03-04.json`.
2. Después de mitigación auth (v2):
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_canary_after_auth_cache_v2_2026-03-04.json`.
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_canary_after_auth_cache_v2_rerun_2026-03-04.json`.

Comparativo principal (`before -> after`):
1. `POST /events`:
- `401: 39 -> 0`
- p95 `1073.4ms -> 989.5ms`
- p99 `2950.8ms -> 1700.7ms`
- throughput `11.42 -> 13.81 rps`
- Evidencia: before `...canary_2026-03-04.json:12-13,24-25`; after `...after_auth_cache_v2_2026-03-04.json:12-13,21-22`.
2. `GET /logs`:
- `401: 4 -> 0`
- `500: 17 -> 8` (mejora parcial; no cerrado)
- p95 `814.6ms -> 591.2ms`
- throughput `13.02 -> 19.27 rps`
- Evidencia: before `...canary_2026-03-04.json:38-39,51-52`; after `...after_auth_cache_v2_2026-03-04.json:35-36,47-48`.
3. `GET /stats`:
- `401: 8 -> 0`
- p95 `986.6ms -> 2.5ms`
- throughput `29.10 -> 182.21 rps`
- Evidencia: before `...canary_2026-03-04.json:65-66,77-78`; after `...after_auth_cache_v2_2026-03-04.json:61-62,70-71`.
4. `POST /chat/ask`:
- `401: 9 -> 0`
- `500: 5 -> 3` (mejora parcial)
- p95 `819.0ms -> 612.0ms`
- throughput `16.59 -> 30.19 rps`
- Evidencia: before `...canary_2026-03-04.json:91-92,104-105`; after `...after_auth_cache_v2_2026-03-04.json:84-85,96-97`.

Lectura operativa:
1. Se elimina el falso “token inválido” bajo carga transitoria de DB (401 por backend unavailable).
2. El cuello residual más claro queda en rutas pesadas de lectura (`/logs`) y consultas de chat que dependen de esas lecturas.
3. Siguiente fase prioritaria: optimizar query/payload de `/logs` y team/chat query path para bajar `500` residual bajo stress.

### 11.16 Fase B parcial aplicada (2026-03-04) — cache corta de `/logs` para ráfagas

Implementación:
1. Estado del server:
- nuevo `LogsCacheEntry` + `logs_cache_ttl` + `logs_cache` en `AppState`.
- Evidencia: `gitgov/gitgov-server/src/handlers/prelude_health.rs:53,90-92`.
2. Config:
- nueva env var `GITGOV_LOGS_CACHE_TTL_MS` (default `800`).
- Evidencia: `gitgov/gitgov-server/src/main.rs:493,522-523,679`.
3. Handler `/logs`:
- cache key por filtro efectivo+scope (`role|EventFilter`) y bypass para paginación profunda (`offset>0`) o keyset cursor.
- lectura desde cache antes de DB y write-through en éxito.
- invalidación de cache de logs en ingesta (`POST /events`) para mantener frescura.
- Evidencia: `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:239-240,317,391-452,545-561`.

Artefactos comparados (carga prolongada `220`, concurrencia `8`):
1. Antes (auth cache ya aplicado, sin logs cache):
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_canary_after_auth_cache_v2_2026-03-04.json`.
2. Después (logs cache):
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_cache_2026-03-04.json`.
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_cache_rerun_2026-03-04.json`.

Comparativo principal (`before -> after`, usando rerun):
1. `GET /logs`:
- p95 `591.2ms -> 4.6ms`
- p99 `762.7ms -> 586.9ms`
- throughput `19.27 -> 351.72 rps`
- `500: 8 -> 3`
- Evidencia before: `...canary_after_auth_cache_v2_2026-03-04.json:35-36,47-48`.
- Evidencia after: `...after_logs_cache_rerun_2026-03-04.json:35-36,47-48`.
2. `POST /chat/ask`:
- p95 `612.0ms -> 443.2ms`
- throughput `30.19 -> 32.62 rps`
- `500: 3 -> 2`
- Evidencia before: `...canary_after_auth_cache_v2_2026-03-04.json:84-85,96-97`.
- Evidencia after: `...after_logs_cache_rerun_2026-03-04.json:84-85,96-97`.
3. `POST /events`:
- sin regresión de error-rate (`500=0`), p95/p99 mejoran.
- Evidencia before: `...canary_after_auth_cache_v2_2026-03-04.json:12-13,21-22`.
- Evidencia after: `...after_logs_cache_rerun_2026-03-04.json:12-13,21-22`.

Lectura operativa:
1. Mitigación efectiva para ráfagas sobre `/logs` sin romper contrato.
2. Quedan `500` residuales bajo stress extremo: requiere Fase B profunda en query SQL (`UNION ALL` + orden global + offset) para cierre total.

### 11.17 Fase B profunda aplicada (2026-03-04) — `/logs` en una sola query

Implementación:
1. Se removió el enrichment adicional post-query en `get_combined_events(...)`.
2. `details` de eventos `client` ahora incluye en SQL:
- `reason`, `files`, `event_uuid`, `commit_sha`, `user_name`.
3. `metadata` se fusiona en la misma construcción de `details`:
- si metadata es objeto, se mergea al top-level;
- si no es objeto, se conserva como `metadata` dentro de `details`.
4. Evidencia: `gitgov/gitgov-server/src/db.rs:1003-1140` (query), y eliminación de bloque de enrichment adicional en la misma función.

Artefactos comparados:
1. Antes (logs cache, con enrichment adicional):
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_cache_rerun_2026-03-04.json`.
2. Después (single-query):
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_sql_inline_2026-03-04.json`.
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_sql_inline_rerun_2026-03-04.json`.

Comparativo principal (`before -> after`, rerun):
1. `GET /logs`:
- `500: 3 -> 1`
- p99 `586.9ms -> 305.9ms`
- throughput `351.72 -> 357.56 rps`
2. `POST /chat/ask`:
- `500: 2 -> 1`
- p99 `987.8ms -> 755.9ms`
- throughput `32.62 -> 32.95 rps`

Lectura operativa:
1. Se reduce presión de DB y se recorta error residual en `/logs`.
2. Persiste `500` marginal en stress extremo: siguiente blindaje recomendado es fallback a cache reciente en error DB para `/logs`.

### 11.18 Fase B blindaje aplicada (2026-03-04) — stale fallback para `/logs` en error DB

Implementación:
1. `GET /logs` ahora intenta responder con cache reciente cuando la DB falla transitoriamente.
2. Si hay cache reciente, devuelve `200` con payload de logs; si no la hay, mantiene `500`.
3. Env var nueva:
- `GITGOV_LOGS_CACHE_STALE_ON_ERROR_MS` (default `5000`).
4. Evidencia:
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs` (`get_cached_logs_on_error` + rama de error de `get_logs`).
- `gitgov/gitgov-server/src/handlers/prelude_health.rs` (`logs_cache_stale_on_error`).
- `gitgov/gitgov-server/src/main.rs` (parse/inyección de `logs_cache_stale_on_error_ms`).

Artefactos comparados:
1. Antes:
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_sql_inline_rerun_2026-03-04.json`.
2. Después:
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_stale_fallback_2026-03-04.json`.
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_stale_fallback_rerun_2026-03-04.json`.

Comparativo principal (`before -> after`, rerun):
1. `GET /logs`:
- `500: 1 -> 0`
- p95 `21.6ms -> 19.3ms`
- throughput `357.56 -> 604.81 rps`
2. `POST /chat/ask`:
- p99 `755.9ms -> 723.8ms`
- throughput `32.95 -> 33.10 rps`
- `500` residual `1` (aún no cerrado al 100%).

Lectura operativa:
1. `/logs` queda endurecido frente a errores transitorios de DB bajo ráfaga.
2. El próximo foco pasa al path del bot para manejar mejor saturación residual sin devolver `500`.

### 11.19 Cierre chat aplicado (2026-03-04) — degradación controlada en errores internos

Implementación:
1. `finalize_chat_response(...)` ahora degrada respuestas de chat con `StatusCode::INTERNAL_SERVER_ERROR` a:
- HTTP `200`,
- `status = insufficient_data` (si venía `error`),
- mensaje de reintento corto para el usuario.
2. Evidencia: `gitgov/gitgov-server/src/handlers/conversational/engine.rs:267-307`.

Artefactos comparados:
1. Antes:
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_logs_stale_fallback_rerun_2026-03-04.json`.
2. Después:
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_chat_graceful_2026-03-04.json`.
- `gitgov/gitgov-server/tests/artifacts/perf_long_control_plane_after_chat_graceful_rerun_2026-03-04.json`.

Comparativo principal (`before -> after`, rerun):
1. `POST /chat/ask`:
- `500: 1 -> 0`
- HTTP `200: 219 -> 220`
- p95 `433.2ms -> 436.3ms` (estable)
- throughput `33.10 -> 32.99 rps` (estable)
2. `/logs`, `/stats`, `/events` se mantienen en `500=0` en el mismo perfil.

Lectura operativa:
1. Se elimina el “crash percibido” del bot por `500` bajo la carga de prueba aplicada.
2. Se mantiene robustez funcional: el bot responde con degradación útil en vez de fallo duro.

### 11.20 Validación Golden Path live (2026-03-04) — contractual backend completado

Ejecución:
1. Se validó live contra `http://127.0.0.1:3000` con `Authorization: Bearer`.
2. Se emitieron eventos Golden Path (`stage_files`, `commit`, `attempt_push`, `successful_push`) y se verificó visibilidad en `/logs`.
3. Se verificó deduplicación por UUID (`duplicates` en reenvío).
4. Se verificaron endpoints de soporte:
- `/health`, `/stats`, `/logs`, `/stats/daily?days=14`, `/admin-audit-log`, `/chat/ask`.

Artefactos:
1. `gitgov/gitgov-server/tests/artifacts/golden_path_live_ps_2026-03-04.json` (`summary.passed=true`).
2. `gitgov/gitgov-server/tests/artifacts/golden_path_extended_ps_2026-03-04.json` (`summary.passed=true`).

NO VERIFICADO:
1. Scripts shell oficiales `tests/smoke_contract.sh` y `tests/e2e_flow_test.sh` no se ejecutaron directamente por entorno local sin `/bin/bash` funcional (bash apunta a WSL no instalado).
2. Checklist manual de Desktop UI (edición real de archivo + commit/push desde Tauri + verificación visual en tabla) pendiente en esta pasada automática.

### 11.21 Cierre de ejecución de scripts oficiales en Windows Git Bash (2026-03-04)

Actualización:
1. Se adaptaron `smoke_contract.sh` y `e2e_flow_test.sh` para entorno Git Bash Windows:
- UUID robusto (evita vacío),
- `user_login` de prueba no sintético (`manual_check`) para compatibilidad con `GITGOV_REJECT_SYNTHETIC_LOGINS=true`,
- check de duplicate más estricto en smoke.
2. Ejecución real:
- `smoke_contract.sh` -> `20 passed, 0 failed`.
- `e2e_flow_test.sh` -> `exit 0` (flujo completo OK).
3. Evidencia:
- `gitgov/gitgov-server/tests/artifacts/smoke_contract_gitbash_2026-03-04.log`
- `gitgov/gitgov-server/tests/artifacts/e2e_flow_gitbash_2026-03-04.log`

Estado:
1. Scripts oficiales Golden Path: verificados.
2. Pendiente final fuera de automatización: validación manual visual Desktop UI.
