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
