# GitGov - Registro de Progreso

## Actualización Reciente (2026-02-28) — Fix de descarga en Web Deploy (URL externa)

### Qué se implementó
- `gitgov-web` ahora soporta descarga del Desktop por URL externa configurable:
  - Nueva configuración: `NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL`.
  - Si está definida, `siteConfig.downloadPath` usa esa URL en lugar de `/downloads/...`.
- `app/(marketing)/download/page.tsx` ya no bloquea el botón cuando el instalador se hospeda fuera de `public/`:
  - En modo URL externa (`http/https`), marca `available: true` sin hacer `fs.stat` local.
  - Mantiene el comportamiento anterior para artefactos locales en `public/downloads`.

## Actualización Reciente (2026-02-28) — Build firmado local de Desktop (Windows)

### Qué se implementó
- Nuevo script operativo: `scripts/build_signed_windows.ps1`
  - Soporta certificado por `-PfxPath`/`-PfxBase64` o `-Thumbprint`.
  - Inyecta temporalmente `certificateThumbprint` en `src-tauri/tauri.conf.json`, ejecuta `npm run tauri build`, valida firma Authenticode de MSI/NSIS y genera `.sha256`.
  - Restaura `tauri.conf.json` al finalizar (incluso si falla el build).
- Documentación de uso local añadida en `docs/ENTERPRISE_DEPLOY.md` (sección "Local signed build (Windows)").

## Actualización Reciente (2026-02-28) — Auditoría de Devs Activos + Marcado Synthetic/Test

### Qué se implementó
- **Detalle auditable para `Devs Activos 7d` en Dashboard**:
  - El card ahora abre un modal con lista de usuarios activos en 7 días, número de eventos y último timestamp.
  - Se añadió acción `loadActiveDevs7d()` en store para construir la lista desde `/logs` (ventana 7d, `limit=500`) sin romper compatibilidad con servidores que no tengan endpoints nuevos.
- **Señal de datos sospechosos en el detalle de devs**:
  - Cada usuario se marca como `suspicious/test` si coincide con patrones sintéticos (`alias_*`, `erase_ok_*`, `hb_user_*`, etc.) o si todos sus eventos de la muestra llegan sin `repo` ni `branch`.
- **Marcado visual en Commits Recientes**:
  - Se agregó badge `synthetic/test` por fila cuando el evento luce sintético (patrón de login o shape de evento sin repo/branch).

### Archivos modificados
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
- `gitgov/src/components/control_plane/MetricsGrid.tsx`
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx`

### Validación ejecutada
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerDashboard.tsx src/components/control_plane/MetricsGrid.tsx src/components/control_plane/RecentCommitsTable.tsx` → sin errores
- Validación contractual no destructiva contra server activo:
  - `GET /health` → 200
  - `GET /stats` (Bearer) → 200
  - `GET /logs?limit=5&offset=0` (Bearer) → 200

## Actualización Reciente (2026-02-28) — Scope Helpers Unificados (logs/signals/aliases)

### Correcciones aplicadas
- **Helper de scope unificado** en backend:
  - Se añadieron `OrgScopeError`, `org_scope_status`, `check_org_scope_match` y `resolve_and_check_org_scope`.
  - Se eliminó duplicación de lógica de scope en handlers.
- **`GET /signals` corregido para org-scoped keys**:
  - Ahora resuelve y aplica `org_id` efectivo (incluye caso admin org-scoped sin `org_name` explícito).
  - Evita exposición cross-org por omisión de filtro.
- **`GET /logs` ahora usa el helper común**:
  - Misma semántica de 403/404/500 según scope y resolución de org.
  - Preferencia por `org_id` (UUID) para evitar lookup redundante por `org_name`.
- **`POST /identities/aliases` refactorizado**:
  - Reutiliza helper de scope con regla `org_name` obligatorio para admin global.
  - Mantiene respuestas contractuales: 400/403/404.
- **DB signals filtrado por UUID**:
  - `get_noncompliance_signals` pasó de `org_name` a `org_id`, con condición SQL `ns.org_id = $n::uuid`.

### Archivos principales
- `gitgov/gitgov-server/src/handlers.rs`
- `gitgov/gitgov-server/src/db.rs`

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `52 passed; 0 failed`
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/RecentCommitsTable.tsx src/components/control_plane/MetricsGrid.tsx src/components/control_plane/ServerDashboard.tsx` → sin errores
- `cd gitgov/gitgov-server && cargo clippy` → warnings preexistentes (sin errores de compilación)

## Actualización Reciente (2026-02-28) — Hardening de GDPR / Heartbeat / Identity Aliases

### Correcciones críticas aplicadas
- **Heartbeat corregido**: `heartbeat` ya no se deserializa como `attempt_push`.
  - Se añadió `ClientEventType::Heartbeat` en backend para preservar el tipo real.
- **Identity aliasing funcional en `/logs`**:
  - `get_combined_events` ahora proyecta `user_login` canónico vía `identity_aliases`.
  - Filtrar por `user_login=<canonical>` incluye eventos de aliases del mismo org.
- **Scope enforcement en aliases (multi-tenant)**:
  - `POST /identities/aliases` ahora valida org explícitamente:
    - key org-scoped no puede crear alias para otra org (`403`),
    - `org_name` inexistente devuelve `404`,
    - admin global debe enviar `org_name` (sin filas globales implícitas).
- **Scope enforcement en GDPR export/erase**:
  - `GET /users/{login}/export` y `POST /users/{login}/erase` ahora aplican `auth_user.org_id` cuando la key es org-scoped.
  - Si el usuario no existe en el scope visible, responden `404`.
- **Append-only respetado en GDPR/TTL**:
  - Se eliminó la lógica que intentaba `UPDATE/DELETE` sobre `client_events`/`github_events`.
  - `erase_user_data` ahora registra la solicitud y retorna conteos scoped.
  - El job TTL ahora limpia `client_sessions` antiguos (no eventos de auditoría append-only).
- **Compatibilidad de señales/stats preservada**:
  - Webhook push mantiene `event_type="push"` (y `forced` en payload), evitando romper SQL existente de métricas/detección.

### Archivos principales
- `gitgov/gitgov-server/src/models.rs`
- `gitgov/gitgov-server/src/db.rs`
- `gitgov/gitgov-server/src/handlers.rs`
- `gitgov/gitgov-server/src/main.rs`

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `38 passed; 0 failed`
- `cd gitgov/src-tauri && cargo check` → OK
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov/gitgov-server/tests && smoke_contract.sh` → `17 passed; 0 failed`
- Verificación empírica adicional:
  - heartbeat visible como `event_type=heartbeat` (sin contaminar `attempt_push`)
  - alias canónico agrega eventos de alias en `/logs`
  - bloqueo de cross-org en `POST /identities/aliases`
  - `GET /users/{login}/export` con key scoped fuera de org → `404`

## Actualización Reciente (2026-02-28) — Auditoría por Día (commits/pushes) en Dashboard

### Qué se implementó
- Endpoint backend nuevo: `GET /stats/daily?days=N` (admin-only, con scope por `org_id` de la API key).
- Serie diaria en UTC (append-safe) de `commit` y `successful_push` desde `client_events`, con `generate_series` para devolver días sin actividad en `0`.
- Cableado end-to-end en Desktop/Tauri/Frontend:
  - comando Tauri `cmd_server_get_daily_activity`,
  - estado `dailyActivity` en `useControlPlaneStore`,
  - refresh del dashboard ahora carga los últimos `14` días,
  - widget visual `Actividad diaria (UTC)` con barras `commits` vs `pushes`.
- Publicación de ruta en server router:
  - `GET /stats/daily` con el mismo rate-limit admin que `/stats`.

### Archivos
- `gitgov/gitgov-server/src/models.rs`
  - `DailyActivityPoint`, `DailyActivityQuery`
- `gitgov/gitgov-server/src/db.rs`
  - `get_daily_activity(org_id, days)`
- `gitgov/gitgov-server/src/handlers.rs`
  - `get_daily_activity` (admin-only, clamp `days` 1..90)
- `gitgov/gitgov-server/src/main.rs`
  - ruta `GET /stats/daily`
- `gitgov/src-tauri/src/control_plane/server.rs`
  - `DailyActivityPoint`, `DailyActivityFilter`, `get_daily_activity()`
- `gitgov/src-tauri/src/commands/server_commands.rs`
  - `cmd_server_get_daily_activity`
- `gitgov/src-tauri/src/lib.rs`
  - registro de comando en `generate_handler!`
- `gitgov/src/store/useControlPlaneStore.ts`
  - estado `dailyActivity`, acción `loadDailyActivity()`, refresh integrado
- `gitgov/src/components/control_plane/DailyActivityWidget.tsx`
  - widget nuevo de actividad diaria
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
  - integración del widget en el layout principal

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `38 passed; 0 failed`
- `cd gitgov/src-tauri && cargo check` → OK
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerDashboard.tsx src/components/control_plane/DailyActivityWidget.tsx` → 0 errores

### Checklist empírico (Golden Path)
- `POST /events` con `Authorization: Bearer` → aceptado (`accepted` con UUID nuevo, `errors=[]`)
- `GET /stats` con Bearer → 200 y shape válido
- `GET /logs?limit=5&offset=0` con Bearer → 200 y `events`
- `GET /stats/daily?days=14` con Bearer → 200 y 14 puntos (`YYYY-MM-DD`)
- `gitgov/gitgov-server/tests/smoke_contract.sh` → `17 passed, 0 failed`

## Actualización Reciente (2026-02-27) — Badge de Aprobaciones en Dashboard + Cierre Golden Path

### Qué se implementó
- Se cableó `GET /pr-merges` end-to-end en Desktop/Tauri/Frontend para mostrar evidencia de aprobaciones de PR por commit.
- `Commits Recientes` ahora muestra:
  - **columna `Aprob.`** con badge visual (`>=2` en verde, `<2` en rojo),
  - badge `PR #<n>` en el detalle del commit cuando existe correlación.
- Correlación UI: se asocia por `commit_sha` del commit local contra `head_sha` de `pr-merges` (match exacto y prefix match corto/largo).

### Archivos
- `gitgov/src-tauri/src/control_plane/server.rs`
  - `PrMergeEvidenceFilter`, `PrMergeEvidenceEntry`
  - `get_pr_merges()`
- `gitgov/src-tauri/src/commands/server_commands.rs`
  - `cmd_server_get_pr_merges`
- `gitgov/src-tauri/src/lib.rs`
  - registro de `cmd_server_get_pr_merges` en `generate_handler!`
- `gitgov/src/store/useControlPlaneStore.ts`
  - estado `prMergeEvidence`
  - acción `loadPrMergeEvidence()`
  - `refreshDashboardData()` incluye carga de PR merges
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx`
  - columna `Aprob.`
  - badge `PR #`
  - regla visual de cumplimiento mínimo `2` aprobaciones

### Cierre operativo (checklist empírico)
- Se detectó y corrigió conflicto local de puertos antes de validar:
  - `127.0.0.1:3000` estaba ocupado por `node` (web dev) y `/health` devolvía `404`.
  - Se levantó `gitgov-server` en `127.0.0.1:3000` para evitar split-brain durante la validación.
- Se aplicó migración `supabase_schema_v7.sql` en DB activa para habilitar tablas de PR evidence:
  - `pull_request_merges`
  - `admin_audit_log`

### Smoke/Golden Path
- `tests/smoke_contract.sh` corregido (header Bearer en Sección A):
  - antes fallaba por no enviar Authorization correctamente en Bash/Windows,
  - ahora usa `AUTH_HEADER=\"Authorization: Bearer ...\"`.
- Resultado actual:
  - `Results: 17 passed, 0 failed`
  - `Exit: 0`

### Validación
- `cargo check` (`gitgov/src-tauri`) ✅
- `npm run typecheck` (`gitgov`) ✅
- `npm run build` (`gitgov`) ✅
- `cargo check` (`gitgov/gitgov-server`) ✅
- `tests/smoke_contract.sh` ✅ (17/17)

## Actualización Reciente (2026-02-27) — Revisión de Org Scoping (Claude)

### Hallazgos y correcciones
- **Bug crítico corregido en `POST /orgs`:**
  - `create_org` estaba usando `upsert_org(0, ...)`.
  - `upsert_org` hace `ON CONFLICT (github_id)`, por lo que múltiples orgs manuales colisionaban en el mismo `github_id=0`.
  - **Fix:** nuevo método `upsert_org_by_login()` en DB y `create_org` actualizado para usar conflicto por `login`.
- **Hardening de aislamiento multi-tenant en `/logs`:**
  - Se añadió validación para impedir que una API key org-scoped consulte `org_name` fuera de su scope.
  - Si no se envía org explícita, se aplica auto-scope por `auth_user.org_id` (como estaba planeado).
- **Hardening en creación de API keys:**
  - Admin org-scoped ya no puede crear claves para otra org.
  - Si omite `org_name`, la clave se crea por defecto en su propia org.

### Validación
- `cargo check` ✅
- `cargo test` ✅ (38/38)

## Actualización Reciente (2026-02-27) — PR Approvals Evidence (4-eyes)

### Qué se implementó
- Captura de aprobadores de PR al procesar webhook `pull_request` mergeado.
- Enriquecimiento del payload guardado en `pull_request_merges` con:
  - `gitgov.approvers` (array de logins aprobadores finales)
  - `gitgov.approvals_count` (conteo final)
- Nuevo endpoint admin para evidencia:
  - `GET /pr-merges` con filtros `org_name`, `repo_full_name`, `merged_by`, `limit`, `offset`.

### Archivos
- `gitgov/gitgov-server/src/handlers.rs`
  - `extract_final_approvers()`
  - `fetch_pr_approvers()` (GitHub API `/pulls/{number}/reviews`)
  - integración en `process_pull_request_event()`
  - handler `list_pr_merges()`
- `gitgov/gitgov-server/src/db.rs`
  - `list_pr_merge_evidence()`
- `gitgov/gitgov-server/src/models.rs`
  - `PrMergeEvidenceEntry`, `PrMergeEvidenceResponse`, `PrMergeEvidenceQuery`
- `gitgov/gitgov-server/src/main.rs`
  - ruta `GET /pr-merges`
  - carga opcional de env `GITHUB_PERSONAL_ACCESS_TOKEN`

### Notas de comportamiento
- Si `GITHUB_PERSONAL_ACCESS_TOKEN` no está configurado o GitHub API falla, el merge se guarda igual (non-fatal) pero con `approvers=[]`.
- Regla aplicada: por cada reviewer se usa su **último** estado de review; solo `APPROVED` cuenta como aprobación final.

### Validación
- `cargo check` ✅
- `cargo test` ✅ (38/38)

## Actualización Reciente (2026-02-27) — Re-auditoría de Enterprise Gaps

### Verificación de implementación (Claude)
- Se validó en código la implementación de:
  - tabla `pull_request_merges` (append-only)
  - tabla `admin_audit_log` (append-only)
  - ingestión de webhook `pull_request` para merges
  - endpoint `GET /admin-audit-log` (admin)
  - audit trail en `confirm_signal`, `export_events`, `revoke_api_key`
- Validación local:
  - `cargo check` ✅
  - `cargo test` ✅ (36/36)

### Corrección aplicada en esta re-auditoría
- **Gap cerrado:** faltaba auditar `policy_override` (estaba en propuesta, no en código).
- **Fix aplicado:** `override_policy` ahora escribe entrada append-only en `admin_audit_log`:
  - `action: "policy_override"`
  - `target_type: "repo"`
  - `target_id: repo.id`
  - `metadata: { repo_name, checksum }`
- Patrón non-fatal preservado: si el insert de auditoría falla, se emite `warn!` y la operación principal continúa.

### Riesgo pendiente (compliance)
- La captura actual de PR guarda quién **mergeó** (`merged_by_login`), pero **no** quiénes aprobaron el PR (review approvals).
- Para cubrir "4-eyes principle" completo (SOC2/ISO), falta correlación de aprobaciones (`pull_request_review`/GitHub API) y persistencia dedicada.

## Actualización Reciente (2026-02-27) — Enterprise Gaps v1

### Resumen ejecutivo
Cuatro gaps enterprise implementados end-to-end (backend + Tauri + frontend):

| Gap | Implementación | Estado |
|-----|----------------|--------|
| Sin revocación de API keys | `GET/POST /api-keys`, `POST /api-keys/{id}/revoke`, `GET /me`, `ApiKeyManagerWidget` | ✅ |
| Export compliance-grade | `get_events_for_export` (hasta 50k registros, sin límite 100), `GET /exports`, `ExportPanel` | ✅ |
| Sin notificaciones salientes | `notifications.rs`, `reqwest` fire-and-forget en `blocked_push` y `confirm_signal` | ✅ |
| Instalación enterprise | `tauri.conf.json` code signing, `build-signed.yml` CI, `docs/ENTERPRISE_DEPLOY.md` | ✅ |

### Fase 1 — API Key Revocation + UI de Gestión
- **`gitgov-server/src/models.rs`**: `ApiKeyInfo`, `MeResponse`, `RevokeApiKeyResponse` structs
- **`gitgov-server/src/db.rs`**: `list_api_keys()`, `revoke_api_key()` — soft-delete con `is_active = FALSE`
- **`gitgov-server/src/handlers.rs`**: handlers `get_me`, `list_api_keys`, `revoke_api_key`
- **`gitgov-server/src/main.rs`**: rutas `/me`, `/api-keys` (GET+POST), `/api-keys/{id}/revoke`
- **`src-tauri/src/control_plane/server.rs`**: structs espejo + `get_me()`, `list_api_keys()`, `revoke_api_key()`
- **`src-tauri/src/commands/server_commands.rs`**: `cmd_server_get_me`, `cmd_server_list_api_keys`, `cmd_server_revoke_api_key`
- **`src/store/useControlPlaneStore.ts`**: `userRole`, `apiKeys`, `loadMe()`, `loadApiKeys()`, `revokeApiKey()`
- **`src/components/control_plane/ApiKeyManagerWidget.tsx`**: tabla con revocación two-click, visible solo si `isAdmin`

### Fase 2 — Notificaciones Salientes por Webhook
- **`gitgov-server/Cargo.toml`**: `reqwest = "0.12"` con `rustls-tls`
- **`gitgov-server/src/notifications.rs`**: `send_alert()`, `format_blocked_push_alert()`, `format_signal_confirmed_alert()`
- **`AppState`**: `http_client: reqwest::Client`, `alert_webhook_url: Option<String>` (de `GITGOV_ALERT_WEBHOOK_URL`)
- Triggers: `tokio::spawn` fire-and-forget en `ingest_client_events` (BlockedPush) y `confirm_signal`
- Compatible con Slack, Teams, Discord, PagerDuty (payload Slack Incoming Webhooks)

### Fase 3 — Export Compliance-Grade
- **`gitgov-server/src/db.rs`**: `get_events_for_export()` (hasta 50,000 registros), `list_export_logs()`
- **`gitgov-server/src/handlers.rs`**: `export_events` ahora aplica `org_name` filter; `list_exports` handler
- **`gitgov-server/src/main.rs`**: ruta `GET /exports`
- **`src-tauri`**: `cmd_server_export`, `cmd_server_list_exports` + structs `ExportResponse`, `ExportLogEntry`
- **`src/components/control_plane/ExportPanel.tsx`**: date range picker + blob download + historial de exports

### Fase 4 — Firma de Código + Instalación Enterprise
- **`src-tauri/tauri.conf.json`**: `bundle.windows` con `digestAlgorithm: "sha256"`, `timestampUrl: Digicert`
- **`.github/workflows/build-signed.yml`**: CI para builds firmados en tags `v*` (Windows MSI+NSIS, macOS DMG)
- **`docs/ENTERPRISE_DEPLOY.md`**: Guía completa IT — NSIS silent, MSI GPO, Intune, env vars, SHA256, firewall

### Validación
- `cargo test`: 36/36 tests OK ✅
- `tsc -b`: 0 errores TypeScript ✅
- ESLint: 0 errores en código nuevo (18 errores pre-existentes en archivos no modificados) ✅
- Golden Path preservado: `validate_api_key` en `auth.rs` ya filtra `is_active = TRUE` → revocación inmediata ✅

---

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
