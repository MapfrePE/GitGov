# QA de Seguridad, Escalabilidad y Rendimiento (2026-03-05)

Scope revisado: `gitgov-server` (Axum), `src-tauri` (Desktop), outbox y flujo de webhooks.

---

## 1) Pregunta: Los tokens de GitHub viven solo en keyring?

Respuesta corta: Mitigado y verificado. El modo por defecto es `keyring-only`; el archivo legacy queda solo bajo flag explícito de compatibilidad y existe prueba determinística para host con archivo legacy + keyring inestable.

Riesgo:
- Exfiltracion local de token si el host esta comprometido o se filtra el perfil de usuario.

Evidencia:
- `gitgov/src-tauri/src/github/auth.rs:8-11,88-110` (`GITGOV_ALLOW_LEGACY_TOKEN_FILE`, `GITGOV_LEGACY_TOKEN_DIR`, `GITGOV_SIMULATE_KEYRING_FAILURE`, `GITGOV_SIMULATE_KEYRING_MEMORY`)
- `gitgov/src-tauri/src/github/auth.rs:197-206,373-377` (ruta legacy configurable; default `%LOCALAPPDATA%/gitgov/<user>.token`)
- `gitgov/src-tauri/src/github/auth.rs:445-480` (`save_token`: keyring obligatorio; backup local solo en compat mode)
- `gitgov/src-tauri/src/github/auth.rs:517-564` (`load_token`: fail-closed por error de keyring cuando compat está apagado; fallback legacy solo con compat explícito)
- `gitgov/src-tauri/src/github/auth.rs:625-650` (`load_token_with_expiry`: misma política)
- `gitgov/src-tauri/src/github/auth.rs:398-515` (barrido `migrate_legacy_tokens_from_disk()` para migración real de archivos legacy preexistentes)
- `gitgov/src-tauri/src/lib.rs:64-71` (migración legacy best-effort ejecutada automáticamente en startup)
- `gitgov/src-tauri/src/github/auth.rs:839-935` (tests determinísticos: fallback/fail-closed y migración real con keyring simulado)

Analisis:
- El backup mejora resiliencia cuando falla keyring, pero incrementa superficie de ataque.

Resolucion propuesta:
- Cambiar default a `keyring-only`.
- Dejar backup de archivo solo con flag explicito de compatibilidad.
- Migrar y eliminar archivos legacy al primer login exitoso.

Estado: Mitigado y verificado (Cambio 11 + validación de migración determinística en esta corrida, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 2) Pregunta: Puede autenticarse una API key revocada durante una caida transitoria de DB?

Respuesta corta: Mitigado parcialmente y reforzado. Además del hardening previo, ahora existe umbral de fallos DB consecutivos que desactiva fallback stale (fail-closed) para incidentes sostenidos.

Riesgo:
- Fail-open temporal durante errores de base de datos.

Evidencia:
- `gitgov/gitgov-server/src/db.rs:43-67` (failpoint debug determinístico para simular caída DB auth: env/flag file)
- `gitgov/gitgov-server/src/db.rs:185-202,205-218` (entrada vencida en TTL ya no se elimina antes del chequeo stale)
- `gitgov/gitgov-server/src/db.rs:2200-2244` (`validate_api_key`: fallback stale + telemetría `stale_age_secs` en error DB)
- `gitgov/gitgov-server/src/auth.rs:64-71,90-94` (fail-closed para auth stale en rutas admin sensibles)
- `gitgov/gitgov-server/src/db.rs:5664-5675,5694-5739` (tests de failpoint + semántica fresh/stale cache)
- `gitgov/gitgov-server/src/db.rs:168-192` (`GITGOV_AUTH_STALE_FAIL_CLOSED_AFTER_DB_ERRORS`, default non-dev `3`, dev `0`)
- `gitgov/gitgov-server/src/db.rs:268-279,2259-2288` (contador de racha de fallo DB + bloqueo stale al superar umbral + reset en éxito DB)
- `gitgov/gitgov-server/src/db.rs:5789-5823` (tests del umbral fail-closed y reset de racha)
- validación runtime live (host local, `127.0.0.1:3026`, `GITGOV_AUTH_STALE_FAIL_CLOSED_AFTER_DB_ERRORS=2` + failpoint por flag file):
  - warm auth: `GET /logs` -> `200`
  - con failpoint y cache expirada (>TTL): `GET /logs` -> `200,401,401` (stale permitido una vez y luego fail-closed)
  - recuperación tras quitar failpoint: `GET /logs` -> `200`

Analisis:
- El tradeoff fue reducir 401 falsos bajo presión, pero con guardrail adicional:
  - fallos DB esporádicos pueden usar stale según política.
  - fallos DB sostenidos cambian automáticamente a fail-closed.

Resolucion propuesta:
- Endurecer stale max en prod (ej. 15-30s).
- Fail-closed para rutas admin criticas (`/api-keys`, `/dashboard`, `/jobs/metrics`).
- Telemetria obligatoria para cada autenticacion por stale cache.

Estado: Mitigado parcialmente, reforzado y verificado runtime live (Cambio 6 + Cambio 15 + Cambio 19, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 3) Pregunta: Hay defaults inseguros si falta `GITGOV_ENV`?

Respuesta corta: Mitigado y verificado. El default sigue endurecido por perfil de compilación y ahora CI/deploy exige `GITGOV_ENV` explícito.

Riesgo:
- Configuracion accidentalmente permisiva en despliegues mal parametrizados.

Evidencia:
- `gitgov/gitgov-server/src/main.rs:187-202` (`parse_runtime_env` con `default_env` por perfil de compilación)
- `gitgov/gitgov-server/src/main.rs:307-311` (warning explícito cuando `GITGOV_ENV` no está seteado)
- `gitgov/gitgov-server/src/main.rs:322-339` (`GITGOV_ALLOW_INSECURE_JWT_FALLBACK` depende de `is_dev_env`)
- `gitgov/gitgov-server/src/main.rs:714-721` (`GITGOV_CORS_ALLOW_ANY` depende de `is_dev_env`)
- validación release ejecutada: `Missing GITHUB_WEBHOOK_SECRET in non-dev hardening mode runtime_env=prod` con `GITGOV_ENV` ausente.
- `.github/workflows/ci.yml:11,22-23,50-51,72-73` (`GITGOV_ENV=ci` + gate de validación por job)
- `.github/workflows/build-signed.yml:10,21-31,148-149,200-201` (`GITGOV_ENV=prod` + gate en jobs windows/macos/linux)
- `.github/scripts/assert-gitgov-env.sh:4-19` (script reusable que falla si falta `GITGOV_ENV` o valor no permitido)
- validación local del gate:
  - sin env -> exit `1`
  - con `GITGOV_ENV=ci` -> exit `0`

Analisis:
- Si se olvida `GITGOV_ENV`:
  - en `debug` local se mantiene `dev` por compatibilidad.
  - en `release` se endurece a `prod` por defecto.

Resolucion propuesta:
- Default runtime a `prod` (o `hardened`) y requerir `GITGOV_ENV=dev` explicitamente en local.
- En no-dev, abortar si faltan secretos obligatorios.

Estado: Mitigado y verificado (Cambio 7 + Cambio 14, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 4) Pregunta: El webhook de GitHub se valida siempre con HMAC?

Respuesta corta: Mitigado parcialmente. En `non-dev` ahora es obligatorio configurar `GITHUB_WEBHOOK_SECRET`; en `dev` sigue siendo opcional.

Riesgo:
- Inyeccion de eventos falsos si el endpoint queda expuesto sin secreto.

Evidencia:
- `gitgov/gitgov-server/src/main.rs:292-305` (parseo de `GITHUB_WEBHOOK_SECRET` + abort en `non-dev` si falta + warning explícito en `dev`)
- `gitgov/gitgov-server/src/handlers/github_webhook.rs:37-63` (validacion condicional por existencia de secret)

Analisis:
- En local puede ser aceptable; en prod no.

Resolucion propuesta:
- En no-dev, arrancar con error si falta `GITHUB_WEBHOOK_SECRET`.
- Exponer health warning explicito cuando webhook no tiene firma activa.

Estado: Mitigado (Cambio 5, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 5) Pregunta: El servidor devuelve 5xx cuando falla el procesamiento de webhook?

Respuesta corta: Mitigado parcialmente. Ahora devuelve `503` en error interno y mantiene `200` para duplicados idempotentes.

Riesgo:
- GitHub puede no reintentar, perdiendo eventos.

Evidencia:
- `gitgov/gitgov-server/src/handlers/github_webhook.rs:143-148` (clasificación de error interno a `StatusCode::SERVICE_UNAVAILABLE`)
- `gitgov/gitgov-server/src/handlers/github_webhook.rs:137` (duplicado idempotente mantiene `200`)

Analisis:
- Para confiabilidad del pipeline conviene que fallas reales regresen 5xx.

Resolucion propuesta:
- Responder `500/503` cuando falle persistencia/procesamiento.
- Mantener `200` solo para duplicados idempotentes.

Estado: Mitigado (Cambio 4, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 6) Pregunta: `/events` tiene limites de tamano de body y lote?

Respuesta corta: Mitigado. `/events` ya tiene limite de body y limite de lote configurables.

Riesgo:
- Presion de memoria/CPU por payloads grandes, incluso con rate-limit por request.

Evidencia:
- `gitgov/gitgov-server/src/main.rs:637` (`GITGOV_EVENTS_MAX_BODY_BYTES`)
- `gitgov/gitgov-server/src/main.rs:845` (`DefaultBodyLimit::max(events_body_limit_bytes)` en `/events`)
- `gitgov/gitgov-server/src/main.rs:496` (`GITGOV_EVENTS_MAX_BATCH`)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:11-27` (rechazo `413` cuando `batch_len` supera `events_max_batch`)

Analisis:
- El throughput puede degradar y abrir vector de DoS por tamano de request.

Resolucion propuesta:
- Agregar `DefaultBodyLimit` para `/events`.
- Limitar `events.len()` (ej. 100-500) y devolver 413/400 cuando exceda.

Estado: Mitigado (Cambio 1, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 7) Pregunta: Un evento invalido puede tumbar todo el batch en `/events`?

Respuesta corta: Mitigado. El handler ya no hace early-return por el primer evento invalido.

Riesgo:
- Perdida de throughput y reintentos innecesarios cuando un lote mixto trae eventos validos e invalidos.

Evidencia:
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:51,72,110,163` (errores de validación por evento acumulados en `pre_validation_errors`)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:239-245` (si no hay válidos, responde `200` con `errors` sin insertar)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:252-253` (combina errores de pre-validación con resultado de inserción de válidos)

Analisis:
- Operativamente aumenta ruido de outbox y retrabajo.

Resolucion propuesta:
- Validar por evento y acumular errores parciales en `errors[]`.
- Continuar ingestando eventos validos del mismo lote.

Estado: Mitigado (Cambio 3, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 8) Pregunta: El rate-limit es robusto para escalado horizontal y fallos internos?

Respuesta corta: Mitigado parcialmente y reforzado. Además del hardening fail-open/fail-closed, ahora existe modo distribuido opcional por DB para multi-instancia.

Riesgo:
- Inconsistencia entre nodos en multi-instancia.
- En limiters fail-open (no sensibles), un fallo interno prioriza disponibilidad sobre bloqueo.

Evidencia:
- `gitgov/gitgov-server/src/main.rs:54-62` (`HashMap` in-memory por proceso)
- `gitgov/gitgov-server/src/main.rs:97-119` (manejo fail-open/fail-closed ante error interno en limiter)
- `gitgov/gitgov-server/src/main.rs:206-222` (failpoint debug controlado por env para validar ruta `internal_error`)
- `gitgov/gitgov-server/src/main.rs:288-311` (middleware devuelve `503` + `RATE_LIMITER_UNAVAILABLE` cuando `internal_error=true`)
- `gitgov/gitgov-server/src/main.rs:965-1017` (rutas sensibles detrás de `admin_rate_limit`: `/api-keys`, `/admin-audit-log`, `/jobs/metrics`, jobs dead/retry)
- `gitgov/gitgov-server/src/main.rs:1263-1288` (tests de failpoint seleccionado + fail-closed determinístico)
- `gitgov/gitgov-server/src/main.rs:73-111,195-296` (`DistributedDbRateLimiter` + `RateLimiterState` con fallback fail-open/fail-closed por error DB)
- `gitgov/gitgov-server/src/main.rs:839-919` (`GITGOV_RATE_LIMIT_DISTRIBUTED_DB` + creación dinámica de limiters distribuidos/in-memory)
- `gitgov/gitgov-server/src/main.rs:860-884` (prune periódico de contadores distribuidos)
- `gitgov/gitgov-server/src/db.rs:288-415` (storage `rate_limit_counters` + check distribuido atómico + retry_after)
- `gitgov/gitgov-server/src/db.rs:393-418` (fix crítico: `count::bigint` + `try_get` en `check_distributed_rate_limit`, elimina panic por decode mismatch)
- validación runtime:
  - con failpoint `admin_endpoints`: `GET /jobs/metrics` -> `503`, body con `RATE_LIMITER_UNAVAILABLE`;
  - sin failpoint: `GET /jobs/metrics` -> `200`.
- validación runtime live distribuida (`GITGOV_RATE_LIMIT_DISTRIBUTED_DB=true`, `127.0.0.1:3027`):
  - `GET /health` -> `200`
  - `GET /jobs/metrics` -> `200`
  - sin panics nuevos en log (`panic`/`ColumnDecode` ausentes)
  - evidencia DB del limiter distribuido (`rate_limit_counters`, `admin_endpoints`): `SUM(count)` sube `13 -> 23` tras ráfaga de requests

Analisis:
- Correcto para single-node/local y ahora con camino de endurecimiento para cluster vía DB.

Resolucion propuesta:
- Migrar a rate-limit distribuido (Redis o gateway).
- Fail-closed para endpoints sensibles (admin/auth) en caso de error interno del limiter.

Estado: Mitigado parcialmente, reforzado y verificado runtime live (Cambio 8 + Cambio 16 + Cambio 19, 2026-03-05; ver `docs/PROGRESS.md`).

---

## 9) Pregunta: `/logs` esta optimizado para alto volumen y consistencia?

Respuesta corta: Mitigado y reforzado. Además del enfoque keyset-first en UI, la API `/logs` ahora publica deprecación formal de `offset` (warning estructurado) y permite desactivar `offset` por flag de runtime sin romper por defecto.

Riesgo:
- `OFFSET` alto degrada consultas en tablas grandes.
- Fallback stale puede mostrar datos desactualizados en incidentes DB.

Evidencia:
- `gitgov/gitgov-server/src/models.rs:358-367` (soporte keyset y offset)
- `gitgov/gitgov-server/src/db.rs:1121` (`LIMIT ... OFFSET ...`)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:392-395` (cache deshabilitado para offset/cursor)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:449` (`logs_cache_stale_on_error`)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:327-332` (`LogsResponse` incluye `stale?: bool`)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:635` (cuando aplica fallback stale responde `stale: true`)
- `gitgov/src/store/useControlPlaneStore.ts:487,588-637,1347-1353` (helper keyset-first + `loadLogs` usa cursor antes que offset)
- `gitgov/src/lib/types.ts:53-54` (`AuditFilter` expone `before_created_at`/`before_id` en frontend)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:327-346` (`LogsResponse` ahora incluye `deprecations?: string[]` + notice formal)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:567-588` (warning en runtime si se usa `offset` y rechazo opcional por `GITGOV_LOGS_REJECT_OFFSET_PAGINATION`)
- `gitgov/gitgov-server/src/main.rs:736-738,771,994` (nuevo env `GITGOV_LOGS_REJECT_OFFSET_PAGINATION` inyectado a `AppState` y telemetría de config)
- `gitgov/src-tauri/src/control_plane/server.rs:79-81,777-779` (cliente conserva compatibilidad pero no envía `offset` cuando es `0`)
- `gitgov/gitgov-server/src/handlers/conversational/core.rs:119` (knowledge base interna actualizada: keyset preferido y `offset` deprecado)
- `gitgov/gitgov-server/src/handlers/tests.rs:304-346` (tests unitarios de aviso deprecado y regla de rechazo opcional)

Analisis:
- Buen compromiso de resiliencia, pero hay tradeoff de frescura y costo en paginacion profunda.

Resolucion propuesta:
- Establecer keyset como camino principal (deprecando `offset` para UI principal).
- Mostrar marca explicita de respuesta stale cuando aplique fallback.

Estado: Mitigado y reforzado (Cambio 9 + Cambio 12 + Cambio 18, 2026-03-05; ver `docs/PROGRESS.md`). Pendiente: retiro total de fallback `OFFSET` en API en ventana de breaking-change planificada.

---

## 10) Pregunta: El outbox aplica backoff exponencial + jitter por evento?

Respuesta corta: Mitigado y reforzado. Además del retry por clase de fallo (`429`, `5xx`, red/otros), jitter por proceso y coordinación cross-host determinística por ventana, ahora existe lease global server-driven opcional para coordinación estricta entre hosts.

Riesgo:
- Reintentos sincronizados entre clientes (thundering herd) ante caidas del servidor.

Evidencia:
- `gitgov/src-tauri/src/lib.rs:112` (flush background cada 60s)
- `gitgov/src-tauri/src/outbox/queue.rs:16,51` (`RETRY_BASE_DELAY_MS` + `next_attempt_at`)
- `gitgov/src-tauri/src/outbox/queue.rs:397,558` (solo envía eventos listos por `is_event_ready_for_retry`)
- `gitgov/src-tauri/src/outbox/queue.rs:732-748` (`mark_chunk_retry` + `mark_event_retry`)
- `gitgov/src-tauri/src/outbox/queue.rs:751-765` (`compute_retry_delay_ms` + `stable_jitter_ms`)
- `gitgov/src-tauri/src/outbox/queue.rs:621,650,669` (errores parse/http/network ahora marcan retry/backoff del chunk)
- `gitgov/src-tauri/src/outbox/queue.rs:40-97,541-586` (`RetryDirective` + clasificación de envío por `429`/`5xx`/otros/red)
- `gitgov/src-tauri/src/outbox/queue.rs:586-607` (parseo `Retry-After` en segundos/date)
- `gitgov/src-tauri/src/outbox/queue.rs:838-851` (delay final con piso específico para `429` y `4xx` no-rate-limit)
- `gitgov/src-tauri/src/outbox/queue.rs:981-1004` (tests unitarios de `Retry-After`, `429` y crecimiento para `5xx`)
- `gitgov/src-tauri/src/outbox/queue.rs:20,238-242,655-667,884-891` (`GITGOV_OUTBOX_FLUSH_JITTER_MAX_MS` + jitter estable del intervalo periódico por worker/proceso)
- `gitgov/src-tauri/src/outbox/queue.rs:1036-1042` (test de jitter estable y acotado)
- `gitgov/src-tauri/src/outbox/queue.rs:249-297` (nuevos env vars de coordinación global: `GITGOV_OUTBOX_GLOBAL_COORD_ENABLED`, `GITGOV_OUTBOX_GLOBAL_COORD_WINDOW_MS`, `GITGOV_OUTBOX_GLOBAL_COORD_MAX_DEFERRAL_MS`)
- `gitgov/src-tauri/src/outbox/queue.rs:760-792` (worker aplica deferral determinístico por ventana antes de flush cuando coordinación global está activa)
- `gitgov/src-tauri/src/outbox/queue.rs:965-1010` (`global_coordination_identity` + `global_coordination_wait_ms`)
- `gitgov/src-tauri/src/outbox/queue.rs:1152-1175` (tests unitarios de estabilidad/ acotamiento de wait global)
- `gitgov/gitgov-server/src/db.rs:324-386` (`ensure_outbox_lease_storage` + `try_acquire_outbox_flush_lease`)
- `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs:339-422` (`POST /outbox/lease` con modo fail-open seguro)
- `gitgov/gitgov-server/src/main.rs:739-790,1016-1017,1192-1196` (flags server lease + wiring de estado + ruta auth)
- `gitgov/src-tauri/src/outbox/queue.rs:281-336,803-840,1086-1141` (cliente outbox pide lease server-driven opcional antes de flush, con fallback fail-open)
- validación runtime live (`127.0.0.1:3028`, `GITGOV_OUTBOX_SERVER_LEASE_ENABLED=true`):
  - request A (`holder=smoke-a`) -> `{"granted":true,"wait_ms":0,"lease_ttl_ms":4000,"mode":"server_lease"}`
  - request B (`holder=smoke-b`) -> `{"granted":false,"wait_ms":3676,"lease_ttl_ms":4000,"mode":"server_lease"}`
- `gitgov/gitgov-server/src/handlers/prelude_health.rs` + `gitgov/gitgov-server/src/handlers/client_ingest_dashboard.rs` (telemetría acumulada de lease + endpoint admin `GET /outbox/lease/metrics`)
- `gitgov/gitgov-server/src/main.rs` (ruta `GET /outbox/lease/metrics` con rate-limit admin y default server lease TTL ajustado a `2000ms`)
- `gitgov/src-tauri/src/outbox/queue.rs` (defaults ajustados por tuning: `window=20000ms`, `max_deferral=1600ms`, `server_lease_ttl=2000ms`)
- `gitgov/gitgov-server/tests/tune_outbox_coordination.py` (harness de tuning con carga real y recomendación de envs)
- artefactos de telemetría real:
  - `gitgov/gitgov-server/tests/artifacts/outbox_coord_tuning_live_2026-03-05_v2.json`
  - `gitgov/gitgov-server/tests/artifacts/outbox_coord_tuning_live_2026-03-05_v3.json`
  - `gitgov/gitgov-server/tests/artifacts/outbox_coord_tuning_live_2026-03-05_v4_dbpool60.json`
  - evidencia de cuello de botella DB (session pool saturado): `gitgov/gitgov-server/tests/tmp_tuning_server_3031.out.log`

Analisis:
- Funciona para MVP, pero no es ideal para incidentes de red prolongados o picos masivos.

Resolucion propuesta:
- Backoff exponencial con jitter por evento/lote.
- Persistir `next_attempt_at` y respetarlo en worker.
- Separar retries por clase de error (429/5xx/network).

Estado: Mitigado, reforzado y operativizado (Cambio 10 + Cambio 13 + Cambio 17 + Cambio 20 + Cambio 21 + Cambio 22, 2026-03-05; ver `docs/PROGRESS.md`). Tuning aplicado con telemetría real: defaults opt-in ajustados a `lease_ttl_ms=2000`, `window_ms=20000`, `deferral_ms=1600`.

---

## Priorizacion sugerida

P1:
- hardening opcional de breaking-change para retiro total de `OFFSET` en `/logs` (cuando se programe ventana de migración de clientes)

P2:
- tuning operativo de outbox lease/global coord en despliegues con alta concurrencia
















ANALISIS GPT /// 


1) Arquitectura general

Lo que ya está bien
La separación de responsabilidades está clara: Desktop App, Control Plane Server, integración GitHub y Web App pública. No parece un monolito improvisado; parece una plataforma con límites definidos entre captura, ingestión, correlación y exposición.

Lo que falta
Falta terminar de alinear la arquitectura “documentada” con la estructura real del repo y endurecer la historia de operación para que alguien externo pueda clonar, entender y levantar cada componente sin tropezar. El repo raíz todavía deja señales de onboarding imperfecto, y GitHub incluso mostró referencias a gitgov-server/tests que no reflejan del todo la anidación real del servidor dentro de gitgov/gitgov-server/.

Riesgo
Medio. No rompe el producto, pero sí daña credibilidad técnica, onboarding y velocidad futura.

Prioridad
P1

Acción concreta
Haz una limpieza de estructura y documentación: un solo diagrama del repo real, un solo camino de arranque por componente, y un README raíz que no deje ambigüedad sobre dónde vive cada servicio.

2) Modelo de datos

Lo que ya está bien
Sí tienes un modelo de datos de verdad. En el servidor aparecen entidades y tipos de dominio como organizaciones, repositorios, miembros, eventos GitHub, eventos del cliente, violaciones y roles (Admin, Architect, Developer, PM), y la arquitectura documenta un esquema versionado que evoluciona desde base hasta Jenkins y Jira.

Lo que falta
Falta volver el modelo más “presentable” para terceros: ERD, tabla de ownership por dominio, explicación de qué es append-only, qué es mutable, qué es derivado y qué es evidencia. Hoy existe, pero todavía vive más como implementación que como contrato técnico visible.

Riesgo
Bajo a medio. El riesgo no es técnico inmediato; es de mantenibilidad y claridad cuando el proyecto crezca.

Prioridad
P2

Acción concreta
Publica un DATA_MODEL.md con 10–15 tablas clave, relaciones, índices esperados y regla de inmutabilidad por entidad.

3) Flujo de eventos y correlación

Lo que ya está bien
Esta es una de tus partes más fuertes. Tienes un flujo explícito de Desktop → Outbox → servidor, con almacenamiento local offline, flush periódico y distinción entre eventos del cliente y eventos de GitHub. Además, el servidor ya contempla webhooks, audit stream, señales de no cumplimiento, violaciones e integraciones con Jenkins y Jira.

Lo que falta
Falta formalizar mejor los estados del ciclo de vida de un evento: recibido, persistido, correlacionado, enriquecido, fallido, reintentado, dead-letter, exportado. Ya tienes piezas de jobs y métricas, pero el modelo de estados todavía debería verse más explícito como parte del producto.

Riesgo
Medio. Cuando suba el volumen, los problemas de orden, duplicados o correlación parcial pueden erosionar la confianza.

Prioridad
P1

Acción concreta
Define una máquina de estados para eventos y jobs. Eso te ayuda tanto para trazabilidad como para debug y dashboards.

4) Seguridad y control de acceso

Lo que ya está bien
Tu base conceptual es buena: OAuth con GitHub para login en desktop, keyring del sistema para credenciales de usuario, autenticación por Bearer token hacia el servidor, hashing SHA256 de API keys, roles diferenciados y separación clara de permisos como admin vs developer. También se documenta HMAC para webhooks GitHub y secretos dedicados para Jenkins/Jira.

Lo que falta
Aquí está una de las brechas más importantes. Tu propia auditoría de performance/escala reconoce deuda real: fallback API key hardcodeada, almacenamiento de PIN/API key en texto claro en localStorage, CORS permisivo y fallback por defecto para JWT secret. Eso no invalida el proyecto, pero sí marca claramente que el hardening aún no está cerrado.

Riesgo
Alto. Esta es la categoría que más fácilmente te puede hacer daño reputacional y técnico.

Prioridad
P0

Acción concreta
Tu siguiente sprint debería cerrar cuatro cosas sin negociar: eliminar fallback secrets, mover cualquier clave sensible a almacenamiento seguro, restringir CORS por entorno y exigir secretos/JWT válidos en runtime sin defaults peligrosos.

5) Observabilidad

Lo que ya está bien
Ya tienes algo útil: /health, /health/detailed, /jobs/metrics, /logs, /stats, /dashboard, más trazas HTTP en el servidor y documentación operativa del job worker. Eso te pone por encima de muchos proyectos que apenas tienen “funciona o no funciona”.

Lo que falta
Te falta cerrar la capa enterprise de observabilidad: métricas con cardinalidad controlada, dashboards operativos mínimos, alertas accionables y una historia clara de trazabilidad por correlation_id o equivalente desde evento cliente hasta webhook/pipeline/ticket. La base existe, pero todavía está más cerca de “instrumentación útil” que de “observabilidad madura”.

Riesgo
Medio. Si el sistema falla bajo carga o hay correlaciones raras, el tiempo de diagnóstico puede dispararse.

Prioridad
P1

Acción concreta
Define 8 métricas obligatorias: ingesta/min, backlog outbox, latencia de flush, jobs running/dead, tasa de correlación, errores 4xx/5xx, latencia DB y fallos de webhook. Luego arma un dashboard operativo mínimo con alertas simples.

6) Rendimiento y escalabilidad

Lo que ya está bien
Tienes una auditoría específica de performance y escalabilidad, y eso ya es excelente señal de madurez. El documento identifica cuellos de botella concretos y no se esconde detrás de “Rust es rápido, así que todo estará bien”. Reconoce problemas de forma, polling, payload, batching y consultas de alta cardinalidad.

Lo que falta
Falta ejecutar y cerrar sistemáticamente las correcciones priorizadas de esa auditoría. En especial, el propio documento señala presión sobre el outbox, consultas costosas y deuda de configuración/seguridad que amplifica la inestabilidad.

Riesgo
Alto a medida que suba el volumen.

Prioridad
P1

Acción concreta
No abras más features de alto tráfico hasta completar el paquete de estabilización: batching mejorado, paginación/keyset en logs, reducción de payload por defecto y controles de polling.

7) Job system y resiliencia

Lo que ya está bien
El servidor documenta un worker con TTL para jobs atascados, polling periódico, backoff tras error, métricas, cola de muertos y reintentos. Eso ya es una base seria de resiliencia operativa.

Lo que falta
Falta hacer más visible el contrato de idempotencia: qué jobs pueden reintentarse sin duplicar efectos, cómo se detectan jobs zombis, y cómo se verifica integridad de resultados tras retry. La estructura está; la formalización todavía puede subir mucho.

Riesgo
Medio.

Prioridad
P2

Acción concreta
Añade un documento corto de JOB_SEMANTICS.md con reglas de retry, deduplicación e idempotencia.

8) Despliegue y operación

Lo que ya está bien
La historia de despliegue ya existe: Ubuntu + Nginx + systemd o Docker para el server, Supabase/PostgreSQL, web en Vercel, releases de escritorio y workflows de CI. Para una sola persona, eso está muy bien encaminado.

Lo que falta
Falta más estandarización de entornos: matriz dev/stage/prod, checklist de variables obligatorias, bootstrap sin magia y rollback documentado. Hoy se siente funcional, pero todavía no completamente productizado.

Riesgo
Medio.

Prioridad
P2

Acción concreta
Crea una “Deployment Contract” de una sola página: variables mínimas, secretos obligatorios, puertos, health checks, migraciones y rollback.

9) Evidencia, auditoría e inmutabilidad

Lo que ya está bien
El valor de GitGov está bien orientado hacia trazabilidad y evidencia: logs, audit stream, governance events, violations, compliance y export. La intención del producto está muy clara y bien posicionada.

Lo que falta
Aquí te falta demostrarlo de forma más contundente. El claim de evidencia fuerte/inmutable necesita una presentación más verificable: reglas append-only, quién puede borrar qué, si existe soft delete, cómo se preserva integridad, qué índices y constraints sostienen esa promesa.

Riesgo
Medio a alto, porque es parte del corazón del producto.

Prioridad
P1

Acción concreta
Haz una sección técnica llamada “Why this audit trail is trustworthy” y documenta exactamente qué protege la evidencia y qué no.

10) Producto y enfoque V1

Lo que ya está bien
La visión está fuerte. No estás haciendo “otro dashboard DevOps”; estás haciendo gobernanza con evidencia y correlación desde el origen. Eso es bastante más interesante que un panel bonito con números tristes.

Lo que falta
Ahora el enemigo no es la falta de ideas, sino la dispersión. Tienes suficiente superficie como para perder meses en features periféricas mientras lo crítico sigue con deuda. Tu propia auditoría ya te dice dónde duele de verdad.

Riesgo
Alto si te desenfocas.

Prioridad
P0

Acción concreta
Congela features “wow” por un ciclo corto y ejecuta un hardening sprint.

Prioridades recomendadas, ya ordenadas

P0 — cerrar ya

Hardening de secretos, API keys, JWT y storage local.

Enfoque V1: evitar dispersión y no abrir más superficie crítica antes de estabilizar.

P1 — siguiente bloque

Flujo de eventos con estados explícitos.

Observabilidad mínima seria.

Cerrar performance/scalability prioritario.

Demostrabilidad de audit trail/inmutabilidad.

Consistencia repo/docs/onboarding.

P2 — después

Formalizar el modelo de datos para terceros.

Semántica de jobs/idempotencia.

Contrato de despliegue más estricto.

Lectura final estilo CTO

Lo mejor de GitGov hoy:
La arquitectura ya tiene columna vertebral, el flujo de eventos está bien pensado y el proyecto transmite que entiende governance de verdad.

Lo más peligroso hoy:
No es Rust, no es complejidad distribuida, ni falta de ideas. Es la combinación de deuda de seguridad/configuración, observabilidad todavía incompleta y riesgo de dispersarte antes de endurecer el núcleo.

Mi conclusión:
Vas bien. Pero el siguiente salto no es “más features”; es más confianza. 


  ANALISIS VERIFICADO LOCAL (CODEX) /// 2026-03-07

  Resumen corto
  - El análisis GPT es útil y mayormente correcto a nivel estratégico.
  - Precisión estimada contra el repo local actual: ~70-80%.
  - Tiene partes desactualizadas (sobre todo seguridad/performance/docs) frente a cambios recientes ya aplicados.

  Verificación por bloque

  1) Arquitectura general
  - Estado: CORRECTO.
  - Evidencia:
    - `docs/ARCHITECTURE.md:7` (4 componentes).
    - `docs/ARCHITECTURE.md:19-33` (diagrama Desktop/Server/GitHub/Web App).
    - `docs/ARCHITECTURE.md:172-176` (Web App pública separada).

  2) Repo/docs/onboarding
  - Estado: CORRECTO (sí hay drift real).
  - Evidencia:
    - `README.md:13` usa `cd gitgov-server` (ruta no alineada con estructura anidada).
    - `AGENTS.md:81-82` usa `cd gitgov/gitgov-server` (ruta canónica).
    - `docs/QUICKSTART.md:56` vs `docs/QUICKSTART.md:94` (inconsistencia interna de rutas).

  3) Modelo de datos
  - Estado: PARCIALMENTE CORRECTO.
  - Lo correcto:
    - Roles y entidades están implementados.
    - Evidencia: `gitgov/gitgov-server/src/models.rs:42-45`, `docs/ARCHITECTURE.md:459-499`.
  - Lo desactualizado:
    - El análisis se queda en v6; el repo local ya llega a v12.
    - Evidencia: `gitgov/gitgov-server/supabase/` contiene `supabase_schema_v7.sql` ... `supabase_schema_v12.sql`.
    - `docs/PROGRESS.md:2953` (v9), `docs/PROGRESS.md:2699` (v10), `docs/PROGRESS.md:2606` (v11), `docs/PROGRESS.md:4107` (v12).

  4) Flujo de eventos y correlación
  - Estado: CORRECTO.
  - Evidencia:
    - Outbox offline y envío a `/events`: `docs/ARCHITECTURE.md:61`, `gitgov/src-tauri/src/outbox/queue.rs:254`, `queue.rs:632`.
    - Dedupe por `event_uuid`: `gitgov/gitgov-server/src/db.rs:1142`.
    - Integraciones/correlaciones presentes: `gitgov/gitgov-server/src/main.rs:1077-1115`.

  5) Seguridad y control de acceso
  - Estado: PARCIALMENTE CORRECTO.
  - Correcto:
    - Bearer + SHA256 + keyring: `gitgov/gitgov-server/src/auth.rs:42-52`, `gitgov/src-tauri/src/github/auth.rs:647-689`.
    - PIN/API key en `localStorage` sigue siendo deuda: `gitgov/src/store/useAuthStore.ts:40`, `useAuthStore.ts:210`, `gitgov/src/store/useControlPlaneStore.ts:697-704`.
  - Desactualizado:
    - “fallback API key hardcodeada” ya no aplica tal cual: ahora es fallback por env y con gate.
    - Evidencia: `gitgov/src/store/useControlPlaneStore.ts:507-513`, `useControlPlaneStore.ts:1044`.
  - Matiz importante:
    - CORS/JWT tienen fallback en dev, pero en modo no-dev hay fail-closed.
    - Evidencia: `gitgov/gitgov-server/src/main.rs:498-515`, `main.rs:970-983`.

  6) Observabilidad
  - Estado: CORRECTO (base buena) + PARCIAL (madurez enterprise pendiente).
  - Evidencia base:
    - `/health`, `/health/detailed`, `/logs`, `/stats`, `/dashboard`, `/jobs/metrics`: `gitgov/gitgov-server/src/main.rs:1028-1061`, `main.rs:1240`, `main.rs:1288-1289`.
    - Trazas HTTP: `gitgov/gitgov-server/src/main.rs:1301` (`TraceLayer`).

  7) Rendimiento y escalabilidad
  - Estado: PARCIALMENTE CORRECTO (diagnóstico válido, pero parcialmente viejo).
  - Correcto:
    - Existe auditoría profunda: `docs/PERFORMANCE_SCALABILITY_AUDIT_2026-03-04.md:1`.
  - Desactualizado:
    - Ya se aplicaron mejoras grandes post-auditoría: SSE + debounce + fallback polling + batching outbox + tuning.
    - Evidencia: `docs/PROGRESS.md:79-135`, `gitgov/src/components/control_plane/ServerDashboard.tsx:62-103`, `gitgov/src-tauri/src/outbox/queue.rs:570-575`, `gitgov/src-tauri/src/commands/git_commands.rs:27-29`.
  - Riesgo remanente puntual:
    - Aún hay un path con `thread::spawn` flush en `branch_commands`.
    - Evidencia: `gitgov/src-tauri/src/commands/branch_commands.rs:30-34`.

  8) Job system y resiliencia
  - Estado: CORRECTO.
  - Evidencia:
    - Worker con TTL/poll/backoff: `gitgov/gitgov-server/src/main.rs:42-44`, `main.rs:617`, `main.rs:704`, `main.rs:716`.
    - Dead-letter + retry + métricas: `gitgov/gitgov-server/src/db.rs:3752-3768`, `db.rs:3831-3855`, `db.rs:3932-3996`.
    - Endpoints admin de jobs: `gitgov/gitgov-server/src/main.rs:1240-1255`.

  9) Despliegue y operación
  - Estado: CORRECTO.
  - Evidencia:
    - Docker + EC2 + Nginx + systemd: `docs/DEPLOYMENT.md:8-16`, `docs/DEPLOYMENT.md:100-123`.
    - CI real activo: `.github/workflows/ci.yml:14-33`, `ci.yml:64-82`.

  10) Evidencia, auditoría e inmutabilidad
  - Estado: CORRECTO (más fuerte de lo que GPT sugiere).
  - Evidencia:
    - Triggers append-only: `gitgov/gitgov-server/supabase_schema.sql:211-227`.
    - Violations con update limitado (solo campos de resolución): `supabase_schema.sql:238-265`.
    - COALESCE en agregaciones JSON: `supabase_schema.sql:428`, `supabase_schema.sql:436-437`, `gitgov/gitgov-server/supabase/supabase_schema_v12.sql:23`, `v12.sql:31-32`.

  Hallazgos desactualizados clave en el análisis GPT
  - “Fallback API key hardcodeada” como situación actual: DESACTUALIZADO (ahora es env-gated).
  - “Performance pendiente sin ejecutar”: PARCIAL (ya hubo sprint fuerte de fixes SSE/outbox).
  - “Schema llega a v6”: DESACTUALIZADO (localmente hay v12).

  NO VERIFICADO
  - “GitHub mostró referencias incorrectas” en UI remota: no se verificó en GitHub web durante esta revisión; solo en repo local.

  Conclusión validada
  - El diagnóstico GPT sirve como brújula ejecutiva, pero para decisiones técnicas inmediatas debe usarse esta versión corregida con evidencia local.
