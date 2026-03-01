# GitGov - Registro de Progreso

---

## Actualización (2026-03-01) — Zona Horaria Configurable en Audit Trail

### Motivación
Audit trail con horas incorrectas es inválido legalmente. Los timestamps UTC almacenados en PostgreSQL se mostraban sin conversión de zona horaria en toda la UI.

### Qué se implementó
- **`gitgov/src/lib/timezone.ts`** (nuevo): utilidad de zona horaria con `formatTs()`, `formatTimeOnly()`, `formatDateOnly()`, lista `TIMEZONES` (12 zonas IANA: América Latina, España, UK, EE.UU.), `detectBrowserTimezone()`, clave localStorage `gitgov:displayTimezone`.
- **`useControlPlaneStore`**: estado `displayTimezone` (string IANA, default = browser timezone o localStorage), acción `setDisplayTimezone(tz)` persiste a localStorage.
- **`SettingsPage.tsx`**: nueva sección "Zona Horaria del Audit Trail" con `<select>` de zonas, botón "Auto-detectar del sistema", muestra zona activa. También corrige `toLocaleString()` del updater.
- **`ServerDashboard.tsx`**: badge "Timezone: UTC" → "TZ: {displayTimezone}" dinámico. Timestamps de activeDevs7d actualizados con `formatTs()`.
- **`RecentCommitsTable.tsx`**: timestamp de la columna Hora usa `formatTs()`.
- **`TeamManagementPanel.tsx`**: elimina `formatDate()` local, usa `formatTs(…, displayTimezone)`.
- **`AdminOnboardingPanel.tsx`**: elimina `formatDate()` local, usa `formatTs(…, displayTimezone)` para `expires_at` de invitaciones.
- **`ApiKeyManagerWidget.tsx`**: elimina `formatTimestamp()` local, usa `formatTs(…, displayTimezone)`.
- **`ExportPanel.tsx`**: elimina `formatTimestamp()` local, usa `formatTs(…, displayTimezone)` en historial de exports.
- **`AuditLogRow.tsx`**: mantiene `formatDistanceToNow` para eventos recientes (<24h), reemplaza `format(timestamp, 'dd/MM/yyyy HH:mm')` con `formatTs(…, displayTimezone)` para eventos históricos.
- **`ConversationalChatPanel.tsx`**: reemplaza `toLocaleTimeString('es-PE', …)` con `formatTimeOnly(…, displayTimezone)`.

### Comportamiento
- Zona horaria elegida se persiste en localStorage bajo `gitgov:displayTimezone`.
- Al primer arranque, auto-detecta el timezone del sistema operativo.
- La zona se puede cambiar desde Settings → "Zona Horaria del Audit Trail".
- **No hay cambios en el servidor ni en PostgreSQL**: los datos siguen almacenándose en UTC. Solo cambia la capa de display.

### Validación
- `tsc -b --noEmit`: 0 errores.
- `eslint <archivos tocados>`: 0 errores nuevos.
- Golden Path no tocado (no se modificaron auth, outbox, handlers, models, routes).

### Archivos modificados/creados
- `gitgov/src/lib/timezone.ts` (**NUEVO**)
- `gitgov/src/store/useControlPlaneStore.ts` (+import timezone, +displayTimezone state, +setDisplayTimezone action)
- `gitgov/src/pages/SettingsPage.tsx` (+sección Zona Horaria, +formatTs para updater timestamps)
- `gitgov/src/components/control_plane/ServerDashboard.tsx` (+displayTimezone, UTC badge dinámico)
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx` (+formatTs para columna Hora)
- `gitgov/src/components/control_plane/TeamManagementPanel.tsx` (formatDate → formatTs)
- `gitgov/src/components/control_plane/AdminOnboardingPanel.tsx` (formatDate → formatTs)
- `gitgov/src/components/control_plane/ApiKeyManagerWidget.tsx` (formatTimestamp → formatTs)
- `gitgov/src/components/control_plane/ExportPanel.tsx` (formatTimestamp → formatTs)
- `gitgov/src/components/control_plane/ConversationalChatPanel.tsx` (toLocaleTimeString → formatTimeOnly)
- `gitgov/src/components/audit/AuditLogRow.tsx` (date-fns format → formatTs para histórico)

---

## Actualización (2026-03-01) — Dashboard Conversacional MVP (chat de gobernanza)

### Qué se implementó
- **Backend (Axum/Rust):**
  - `POST /chat/ask` (admin, Bearer): pregunta en lenguaje natural → query engine → LLM Anthropic → respuesta JSON estructurada.
  - `POST /feature-requests` (Bearer): registra capacidades solicitadas por usuarios vía chat.
  - Query engine soporta 3 consultas SQL reales: (1) pushes a main esta semana sin ticket Jira, (2) pushes bloqueados este mes, (3) commits de {usuario} entre fechas.
  - LLM: Anthropic Messages API (`claude-haiku-4-5-20251001`) con system prompt estricto; activado con `ANTHROPIC_API_KEY`.
  - Webhook opcional de notificación (`FEATURE_REQUEST_WEBHOOK_URL`).
  - `AppState` extiende con `llm_api_key` y `feature_request_webhook_url`.
- **Migración SQL:**
  - `supabase_schema_v11.sql`: tabla `feature_requests` (append-only, org_id, requested_by, question, missing_capability, status, metadata, created_at).
- **Tauri Bridge:**
  - Structs: `ChatAskRequest`, `ChatAskResponse`, `FeatureRequestInput`, `FeatureRequestCreated` en `control_plane/server.rs`.
  - Nuevos métodos HTTP en `ControlPlaneClient`: `chat_ask()`, `create_feature_request()`.
  - Nuevos comandos: `cmd_server_chat_ask`, `cmd_server_create_feature_request`.
- **Desktop Frontend:**
  - Nuevo componente `ConversationalChatPanel.tsx`: terminal estética, suggestion chips, status badges, botón "Reportar necesidad".
  - Store: `chatMessages`, `isChatLoading`, acciones `chatAsk()`, `reportFeature()`, `clearChatMessages()`.
  - Integrado en `ServerDashboard` (solo admins conectados).

### Golden Path
- No modifica rutas `/events`, `/logs`, `/stats`, `/dashboard`.
- No modifica `auth_middleware`, `outbox`, ni structs compartidas existentes.
- `cargo test`: 52 passed; 0 failed.
- `cargo clippy -- -D warnings`: 0 errores nuevos.
- `tsc -b --noEmit`: 0 errores.
- `eslint <archivos tocados>`: 0 errores nuevos.

### Archivos modificados/creados
- `gitgov/gitgov-server/supabase/supabase_schema_v11.sql` (**NUEVO**)
- `gitgov/gitgov-server/src/models.rs` (+4 structs)
- `gitgov/gitgov-server/src/db.rs` (+4 funciones: chat_query_pushes_no_ticket, chat_query_blocked_pushes_month, chat_query_user_commits_range, create_feature_request)
- `gitgov/gitgov-server/src/handlers.rs` (+2 campos AppState, +handlers chat_ask + create_feature_request_handler)
- `gitgov/gitgov-server/src/main.rs` (+2 env vars, +2 routes)
- `gitgov/src-tauri/src/control_plane/server.rs` (+4 structs, +2 métodos HTTP)
- `gitgov/src-tauri/src/commands/server_commands.rs` (+2 Tauri commands)
- `gitgov/src-tauri/src/lib.rs` (+2 commands registrados)
- `gitgov/src/store/useControlPlaneStore.ts` (+interfaces, +state, +actions)
- `gitgov/src/components/control_plane/ConversationalChatPanel.tsx` (**NUEVO**)
- `gitgov/src/components/control_plane/ServerDashboard.tsx` (+import + render ChatPanel)

---

## Actualización Reciente (2026-03-01) — Panel de gestión de equipo (admin: developers + repos)

### Qué se implementó
- Backend: nuevas vistas agregadas para gestión de equipo por organización:
  - `GET /team/overview` (admin): lista developers de `org_users` con métricas por ventana (`days`) y resumen de repos activos por developer.
  - `GET /team/repos` (admin): vista invertida por repositorio con developers activos y métricas de actividad.
- Scope/Auth:
  - ambos endpoints exigen Bearer auth y rol admin.
  - respetan scope por `org_id` y `org_name` (global admin requiere `org_name`).
- Tauri bridge:
  - nuevos métodos y comandos para consumir `/team/overview` y `/team/repos`.
- Desktop UI:
  - nuevo componente `TeamManagementPanel` en Control Plane (solo admin), con:
    - filtros `org`, `days`, `status`
    - tab Developers (rol/estado, actividad, repos activos, last seen)
    - tab Repos (developers activos, eventos, commits/pushes/blocked, last seen)
  - integrado en `ServerDashboard` junto a onboarding admin.

### Archivos
- `gitgov/gitgov-server/src/models.rs`
- `gitgov/gitgov-server/src/db.rs`
- `gitgov/gitgov-server/src/handlers.rs`
- `gitgov/gitgov-server/src/main.rs`
- `gitgov/src-tauri/src/control_plane/server.rs`
- `gitgov/src-tauri/src/commands/server_commands.rs`
- `gitgov/src-tauri/src/lib.rs`
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/components/control_plane/TeamManagementPanel.tsx` (nuevo)
- `gitgov/src/components/control_plane/ServerDashboard.tsx`

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`
- `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> OK
- `cd gitgov/src-tauri && cargo clippy -- -D warnings` -> OK
- `cd gitgov && npm run typecheck` -> OK
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerDashboard.tsx src/components/control_plane/TeamManagementPanel.tsx` -> `0 errores`

### Validación empírica (E2E endpoints nuevos)
- Se ejecutó smoke funcional en server local temporal `127.0.0.1:3001`:
  - seed de org + org_users + eventos
  - `/team/overview` devolvió developers esperados
  - `/team/repos` devolvió repos esperados cuando los eventos incluyen `metadata.repo_name` o repo resuelto

### Nota operativa
- En esta sesión, `127.0.0.1:3000` seguía ocupado por un proceso `node`; la validación live se realizó en `127.0.0.1:3001` para evitar split-brain operativo durante la prueba.

### Fix adicional aplicado (misma fecha)
- Se corrigió la causa raíz de repos faltantes en la vista de equipo:
  - Desktop ahora adjunta `repo_full_name`/`org_name` inferidos desde `origin` en eventos clave (`stage_files`, `commit`, `attempt_push`, `blocked_push`, `successful_push`, `push_failed`).
  - Server ingesta `/events` ahora intenta resolver repo desde `repo_full_name` o `metadata.repo_name`; si no existe en `repos`, lo upsertea automáticamente por `full_name` dentro del `org_id` del evento.
- Validación empírica del fix:
  - Seed con eventos que solo traen `repo_full_name` (sin repo preexistente en DB) y ventana `30d`.
  - Resultado esperado/obtenido: `/team/overview` muestra developers con `repos_active_count` correcto y `/team/repos` lista repos activos.

---

## Actualización Reciente (2026-03-01) — Onboarding admin completo (org -> invitaciones -> vistas por rol)

### Qué se implementó
- **Backend onboarding de organizaciones e invitaciones:**
  - Nuevo schema `gitgov/gitgov-server/supabase_schema_v10.sql` con tabla `org_invitations` (token hasheado, expiración, estados `pending/accepted/revoked`, auditoría de aceptación/revocación).
  - Nuevos endpoints admin:
    - `POST/GET /org-invitations`
    - `POST /org-invitations/{id}/resend`
    - `POST /org-invitations/{id}/revoke`
  - Nuevos endpoints públicos para flujo de invitación:
    - `GET /org-invitations/preview/{token}`
    - `POST /org-invitations/accept`
  - Aceptación de invitación transaccional en DB: activa/provisiona `org_users`, emite API key scopeada (`Authorization: Bearer`) y marca invitación como `accepted`.
  - Registro en `admin_audit_log` para crear/reenviar/revocar/aceptar invitaciones.

- **Cliente Tauri (Control Plane) extendido:**
  - Nuevas operaciones para orgs, org users e invitaciones en:
    - `gitgov/src-tauri/src/control_plane/server.rs`
    - `gitgov/src-tauri/src/commands/server_commands.rs`
    - `gitgov/src-tauri/src/lib.rs` (registro de comandos)

- **Desktop UI (Control Plane) con onboarding y vistas por rol:**
  - Nuevo panel admin `AdminOnboardingPanel`:
    - crear org
    - provisionar miembros directos
    - invitar developers
    - listar miembros/invitaciones
    - emitir API key por usuario
  - Nuevo panel developer `DeveloperAccessPanel`:
    - validar token de invitación
    - aceptar invitación y recibir API key
  - `ServerDashboard` ahora aplica refresh por rol:
    - Admin: dashboard completo + onboarding + gestión
    - Developer: vista acotada + aceptación de invitación + commits recientes
  - `useControlPlaneStore` ampliado con estado/acciones de onboarding (orgs, users, invitations, accept/preview, refresh por rol).

### Archivos clave
- `gitgov/gitgov-server/supabase_schema_v10.sql` (nuevo)
- `gitgov/gitgov-server/src/models.rs`
- `gitgov/gitgov-server/src/db.rs`
- `gitgov/gitgov-server/src/handlers.rs`
- `gitgov/gitgov-server/src/main.rs`
- `gitgov/src-tauri/src/control_plane/server.rs`
- `gitgov/src-tauri/src/commands/server_commands.rs`
- `gitgov/src-tauri/src/lib.rs`
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
- `gitgov/src/components/control_plane/AdminOnboardingPanel.tsx` (nuevo)
- `gitgov/src/components/control_plane/DeveloperAccessPanel.tsx` (nuevo)

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`
- `cd gitgov && npm run typecheck` -> OK
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerDashboard.tsx src/components/control_plane/AdminOnboardingPanel.tsx src/components/control_plane/DeveloperAccessPanel.tsx` -> `0 errores`

### Nota de validación
- No se ejecutó smoke live contra server/DB real (`make smoke`, `e2e_flow_test.sh`) en esta pasada; pendiente para validar empíricamente el Golden Path completo en entorno levantado.

---

## Actualización Reciente (2026-02-28) — Aclaración de alcance en AGENTS.md

### Qué se hizo
- Se agregó una nota explícita en `AGENTS.md` para aclarar que el bloque de "Modo Auditor" y su checklist estricto están orientados principalmente a sesiones con Claude Code.

### Archivos
- `AGENTS.md`
- `docs/PROGRESS.md`

---

## Actualización Reciente (2026-02-28) — Tema Desktop a neutro oscuro + logo sidebar más grande

### Qué se hizo
- Se eliminó la dominante azul del theme base de Desktop y se movió a paleta:
  - superficies: gris/negro
  - acento (`brand`): naranja, alineado al logo
- Se actualizaron fondos y estados focus para inputs/cards/glass a valores neutros oscuros.
- Se agrandó el logo del sidebar y se amplió el ancho de la barra para mejorar presencia visual:
  - sidebar `w-14` → `w-16`
  - logo `w-8 h-8` → `w-10 h-10`
  - iconos de navegación y logout también escalados.

### Archivos
- `gitgov/src/styles/globals.css`
- `gitgov/src/components/layout/Sidebar.tsx`
- `gitgov/src/components/layout/MainLayout.tsx`

### Validación
- `cd gitgov && npm run typecheck` → OK
- `cd gitgov && eslint src/components/layout/Sidebar.tsx src/components/layout/MainLayout.tsx src/styles/globals.css`
  - `globals.css` reportado como ignorado por config ESLint (sin errores de TS/React)

---

## Actualización Reciente (2026-02-28) — Desktop icon actualizado a logo.png

### Qué se hizo
- Se regeneraron los íconos de Tauri usando `gitgov/public/logo.png` como fuente.
- Comando ejecutado:
  - `cd gitgov && npx tauri icon public/logo.png`
- Se forzó además el icono de ventana en runtime para `tauri dev`:
  - `gitgov/src-tauri/src/lib.rs` ahora asigna `window.set_icon(...)` con `icon.png` embebido.
  - Se añadió `image` crate (`png`) para decodificar el icono embebido a RGBA.

### Archivos impactados
- `gitgov/src-tauri/icons/*` (png/ico/icns/appx/android/ios)
- Incluye los íconos usados por Windows bundle:
  - `gitgov/src-tauri/icons/32x32.png`
  - `gitgov/src-tauri/icons/128x128.png`
  - `gitgov/src-tauri/icons/128x128@2x.png`
  - `gitgov/src-tauri/icons/icon.ico`

### Validación
- `cd gitgov/src-tauri && cargo build` → OK

### Nota
- En Windows, la barra puede mostrar el ícono anterior por caché hasta reiniciar la app o desanclar/anclar de nuevo.

---

## Actualización Reciente (2026-02-28) — Fix preloader first-paint (web)

### Problema
- En el primer paint se veía el fondo del hero antes del intro de zorro, rompiendo la narrativa visual del preloader.

### Causa raíz
- `Preloader` cargaba `FoxIntro` con `dynamic(..., { ssr: false })`, por lo que el intro no se renderizaba en SSR y aparecía tarde tras hidratación.

### Cambios aplicados
- `gitgov-web/components/layout/Preloader.tsx`
  - Se eliminó `dynamic(..., { ssr: false })`.
  - `FoxIntro` ahora se importa de forma directa para que exista desde el primer render.
- `gitgov-web/components/marketing/FoxIntro.tsx`
  - Imágenes del intro con `loading="eager"`, `fetchPriority="high"` y `decoding="sync"`.
- `gitgov-web/app/layout.tsx`
  - Preload explícito en `<head>` para `/fox.png` y `/fox1.png`.

### Validación ejecutada
- `cd gitgov-web && pnpm run lint` → OK (warnings preexistentes por `<img>` en Header/Footer/FoxIntro)
- `cd gitgov-web && pnpm run build` → OK

### Resultado esperado
- El preloader se pinta desde el inicio y evita mostrar primero el fondo del hero.

---

## Actualización Reciente (2026-02-28) — Hotfix CI estricto (dead_code + unused variables)

### Qué se corrigió
- `gitgov-server` volvía a fallar con `cargo clippy -- -D warnings` por tipos públicos sin referencia en runtime (`dead_code`).
- `src-tauri` fallaba por variables `bak_path` no usadas en Linux (`unused-variables`) aunque eran necesarias en bloque `#[cfg(windows)]`.

### Cambios aplicados
- `gitgov/gitgov-server/src/handlers.rs`
  - `get_job_metrics` ahora devuelve tipos fuertemente tipados:
    - éxito: `JobMetricsResponse`
    - error: `ErrorResponse`
  - Con esto ambos structs quedan usados en código real.
- `gitgov/gitgov-server/src/models.rs`
  - Se añadió `touch_contract_types()` que referencia explícitamente tipos públicos/legacy que siguen siendo parte del contrato compartido.
- `gitgov/gitgov-server/src/main.rs`
  - Se llama `models::touch_contract_types()` al inicio del arranque para mantener esos tipos enlazados bajo clippy estricto.
- `gitgov/src-tauri/src/outbox/queue.rs`
  - `bak_path` se movió dentro de bloques `#[cfg(windows)]` en ambos paths de persistencia atómica.
  - Resultado: Linux deja de reportarlo como variable no usada, Windows mantiene la lógica de rollback/backup.

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo clippy -- -D warnings` → OK
- `cd gitgov/gitgov-server && cargo test` → `52 passed; 0 failed`
- `cd gitgov/src-tauri && cargo clippy -- -D warnings` → OK
- `cd gitgov && npm run typecheck` → OK
- `cd gitgov && npm run lint` → OK
- `cd gitgov-web && pnpm run lint` → OK
- `cd gitgov-web && pnpm run build` → OK

### Impacto en Golden Path
- No se modificó auth (`Authorization: Bearer`), ingestión `/events`, ni contratos `ServerStats`/`CombinedEvent`.
- No hay cambios de comportamiento en commit/push/outbox; solo correcciones de tipado/compilación estricta.

---

## Actualización Reciente (2026-02-28) — Guardrails de identidad git (3 capas)

### Qué se implementó
Prevención de mismatch de identidad git en tres capas para evitar errores de autor que rompen CI/Vercel:

**Capa 1 — Onboarding (`scripts/setup-dev.ps1`):**
- Script PowerShell idempotente para configurar `user.name`, `user.email` y `core.hooksPath` en modo `--local` (solo este repo, no global).
- Muestra valores actuales, acepta valores por parámetro o interactivo, valida formato de email.
- Advierte si el valor difiere del git config global (comportamiento intencional y esperado).

**Capa 2 — Terminal (`.githooks/pre-commit`):**
- Hook sh activado por `core.hooksPath = .githooks`.
- Valida que `user.name` y `user.email` estén definidos y con formato válido antes de cada commit CLI.
- Si falla: aborta el commit y muestra comandos exactos de remediación y referencia al script de setup.

**Capa 3 — Desktop App (`CommitPanel.tsx`):**
- Nuevo comando Tauri `cmd_get_git_identity` (Rust, `git_commands.rs`) que lee `user.name/email` del repo via git2.
- Registrado en `lib.rs` `invoke_handler`.
- `CommitPanel` llama el comando al cambiar `repoPath` y detecta mismatch: identidad incompleta o email que no contiene el login del usuario autenticado.
- Banner de warning no bloqueante visible con instrucciones de remediación (`git config --local` + referencia al script).

### Archivos modificados/creados
- `scripts/setup-dev.ps1` — nuevo, script de onboarding
- `.githooks/pre-commit` — nuevo, hook de validación
- `gitgov/src-tauri/src/commands/git_commands.rs` — añadido `cmd_get_git_identity` (antes de `cmd_push`)
- `gitgov/src-tauri/src/lib.rs` — registrado `cmd_get_git_identity` en invoke_handler
- `gitgov/src/components/commit/CommitPanel.tsx` — añadido `GitIdentity` interface, `useEffect` de detección, banner warning
- `docs/QUICKSTART.md` — nueva sección "Setup de identidad git por repo" (paso 1)
- `docs/PROGRESS.md` — esta entrada

### Impacto en Golden Path
- NO modifica auth headers, `/events`, contratos `ServerStats`/`CombinedEvent` ni lógica de push.
- `cmd_get_git_identity` es read-only sobre el git config. No afecta commits ni push.
- El warning en Desktop es no bloqueante: commit/push siguen funcionando igual.
- Golden Path intacto: `stage_files → commit → attempt_push → successful_push → dashboard` sin cambios.

### Validación ejecutada
- Ver sección de validaciones al final de esta entrada.

---

## Actualización Reciente (2026-02-28) — Remediación de pipeline CI (sin desactivar reglas)

### Qué se corrigió
- Se corrigieron errores reales de lint/clippy en frontend, server y desktop para volver a estado verde en checks estrictos.
- Frontend:
  - Correcciones de accesibilidad (`label` + `htmlFor`/`id`) en formularios.
  - Refactor de router para cumplir `react-refresh/only-export-components` sin desactivar regla.
  - Separación de `Bar` a componente dedicado y helpers en módulo utilitario.
  - Ajustes menores de `useRepoStore` para el nuevo payload de creación de rama.
- Server (`gitgov-server`):
  - Limpieza de variables no usadas y campo muerto de `AppState`.
  - Refactor de firmas con exceso de argumentos en DB:
    - `get_noncompliance_signals` ahora recibe `NoncomplianceSignalsQuery`.
    - `upsert_org_user` ahora recibe `UpsertOrgUserInput`.
  - Actualización de handlers llamadores sin cambiar contrato HTTP externo.
- Desktop (`src-tauri`):
  - Refactors automáticos de Clippy + fixes manuales (`unnecessary_unwrap`, deprecations, doc comments, ramas duplicadas en outbox).
  - Refactor de `cmd_create_branch` para reducir argumentos (`BranchActorInput`) y actualización del caller frontend.
  - Reorganización de módulo outbox (`outbox.rs` → `queue.rs`) para corregir `module_inception`.

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo clippy -- -D warnings` → OK
- `cd gitgov/src-tauri && cargo clippy -- -D warnings` → OK
- `cd gitgov && npm run lint` → OK
- `cd gitgov && npx tsc -b` → OK
- `cd gitgov/gitgov-server && cargo test` → `52 passed; 0 failed`

### Nota
- Esta remediación se hizo **sin desactivar reglas de calidad** (`clippy`/`eslint`) para pasar CI.

---

## Actualización Reciente (2026-02-28) — Provisioning de usuarios por organización (admin)

### Qué se implementó
- Se agregó migración `gitgov/gitgov-server/supabase_schema_v9.sql` para tabla `org_users`:
  - Campos de negocio: `org_id`, `login`, `display_name`, `email`, `role`, `status`.
  - Restricciones: `role` en (`Admin|Architect|Developer|PM`), `status` en (`active|disabled`), `UNIQUE (org_id, login)`.
  - Auditoría de cambios por timestamps (`created_at`, `updated_at`) y trigger de actualización.
- Se añadieron modelos backend en `src/models.rs`:
  - `OrgUser`, `CreateOrgUserRequest/Response`, `OrgUsersQuery/Response`, `UpdateOrgUserStatusRequest`.
- Se añadieron funciones de acceso a datos en `src/db.rs`:
  - `upsert_org_user`, `list_org_users`, `get_org_user_by_id`, `update_org_user_status`.
- Se añadieron handlers y validaciones en `src/handlers.rs`:
  - `create_org_user`, `list_org_users`, `update_org_user_status`, `create_api_key_for_org_user`.
  - Validación estricta de `role` y `status`.
  - Scope por organización reutilizando helper de autorización.
  - Registro de acciones en `admin_audit_log`.
- Se registraron nuevas rutas en `src/main.rs`:
  - `GET/POST /org-users`
  - `PATCH /org-users/{id}/status`
  - `POST /org-users/{id}/api-key`

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `52 passed; 0 failed`.
- `cd gitgov/gitgov-server && cargo clippy` → sin errores de compilación; warnings preexistentes.
- `cd gitgov && npx tsc -b` → sin errores.

### Nota operativa
- Esta entrega deja el backend listo para que un admin gestione usuarios de su org y emita API keys por usuario.
- No cambia contrato del Golden Path de ingest (`/events`) ni el flujo Desktop commit/push.
- Validación live local (server nuevo): `POST/GET /org-users`, `PATCH /org-users/{id}/status`, `POST /org-users/{id}/api-key` ejecutadas con éxito (incluyendo `409` esperado cuando el usuario está `disabled`).
- Estado producción (`http://3.143.150.199`): `/health` y `/stats` responden `200`, pero `/org-users` aún responde `404` hasta desplegar este backend en EC2.

---

## Actualización Reciente (2026-02-28) — Auditoría de preguntas + alineación de claims SSO

### Qué se implementó
- Se creó `questions.md` en raíz con auditoría técnica de 18 preguntas de negocio/integraciones, cada una con evidencia `archivo:línea`.
- Se ajustó copy de pricing en `gitgov-web/lib/i18n/translations.ts` para evitar sobrepromesa de SSO:
  - Starter/Team: `Compliance reports`
  - Enterprise: `Compliance reports (SSO roadmap)`
- Login UX/seguridad (MVP):
  - Nueva pantalla de desbloqueo por PIN local opcional (`PinUnlockScreen`).
  - Configuración de PIN local (activar/actualizar/desactivar/bloquear ahora) en Settings.
  - Acción explícita de "Cambiar usuario" en Settings y Sidebar.
  - Control server opcional `GITGOV_STRICT_ACTOR_MATCH` para rechazar eventos cuyo `user_login` no coincida con `client_id` autenticado.

### Impacto
- Comercial: reduce riesgo de vender capacidades no implementadas.
- Técnico: Golden Path intacto; cambios aditivos en UX de sesión y enforcement opcional por env.

---

## Actualización Reciente (2026-02-28) — CI preparado para 3 plataformas

### Qué se implementó
- Workflow `.github/workflows/build-signed.yml` actualizado para builds de **Windows + macOS + Linux**:
  - Windows: corrige comando de build a `npx tauri build` (antes usaba `npm run tauri build`).
  - macOS: corrige comando a `npx tauri build --target universal-apple-darwin` y agrega `.sha256` para DMG.
  - Linux: nuevo job `build-linux` en `ubuntu-latest` con bundles `AppImage` + `deb` y generación de `.sha256`.
- Script local `scripts/build_signed_windows.ps1` corregido para usar `npx tauri build`.
- `docs/ENTERPRISE_DEPLOY.md` actualizado con panorama multiplataforma (artefactos por OS y prerequisitos).

### Estado
- **Listo en código** para pipeline de 3 plataformas.
- Pendiente comercial: certificado Authenticode (Windows) y notarización Apple (macOS) para distribución enterprise sin warnings.

---

## Actualización Reciente (2026-02-28) — Copy comercial neutral en página de descarga

### Qué se ajustó
- Se reemplazó copy alarmista por copy neutral en `gitgov-web`:
  - Banner de descarga: de “sin firma temporal / ejecutar de todas formas” a mensaje oficial y neutral.
  - Paso de instalación en Windows: ahora instrucción genérica de verificación en pantalla (sin CTA agresiva).
  - Etiqueta `Checksum` renombrada a `Integridad (SHA256)`.
  - Bloque de hash marcado como verificación opcional.

---

## Riesgos Abiertos (Feb 2026)

| # | Riesgo | Estado | Plan de cierre | Owner |
|---|--------|--------|----------------|-------|
| R-1 | **SmartScreen (Windows Defender)** — El instalador sin firma Authenticode activa advertencia SmartScreen en Windows. Usuarios necesitan clicar "Más información" → "Ejecutar de todas formas". | **Abierto** | Adquirir certificado OV/EV Authenticode. Configurar CI con secrets `WINDOWS_CERTIFICATE*`. Trigger: primer cliente pago. | Equipo producto |
| R-2 | **Falta firma Authenticode** — Los instaladores `.exe` y `.msi` actuales no están firmados digitalmente. EDR enterprise puede bloquearlos. | **Abierto** | Ver R-1. El proceso de firma con `scripts/build_signed_windows.ps1` está documentado y listo para activarse. | Equipo infra |
| R-3 | **JWT_SECRET hardcodeado en producción** — Si `GITGOV_JWT_SECRET` no se sobreescribe con un secreto fuerte, cualquiera puede forjar tokens. | **Mitigado localmente** — pendiente verificar en EC2 | Confirmar que la instancia EC2 tiene `GITGOV_JWT_SECRET` configurado con `openssl rand -hex 32`. | DevOps |
| R-4 | **Checksum `pending-build` en web** — Si `NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM` no se actualiza en Vercel en cada release, la página muestra `sha256:pending-build`. | **Proceso documentado** | Seguir `docs/RELEASE_CHECKLIST.md` paso 4 en cada release. Automatizar en CI futuro. | Release manager |
| R-5 | **HTTPS en Control Plane (EC2)** — El server en EC2 sirve en HTTP. Credenciales en tránsito sin cifrar. | **Abierto** | Configurar dominio + Let's Encrypt + reverse proxy (nginx/caddy). | DevOps |

---

## Actualización Reciente (2026-02-28) — Download page: checksum, SHA256 copy, hash verify, MSI, API

### Qué se implementó

**gitgov-web — página /download y configuración de release:**

- `lib/config/site.ts`: dos nuevas variables de entorno:
  - `NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM` — checksum real del instalador; fallback a `sha256:pending-build`
  - `NEXT_PUBLIC_DESKTOP_DOWNLOAD_MSI_URL` — URL opcional para segundo botón `.msi`
- `lib/release.ts` (nuevo): función `getReleaseMetadata()` unificada; usada por la página y la API
- `app/api/release-metadata/route.ts` (nuevo): endpoint GET read-only que devuelve `{ version, downloadUrl, checksum, msiUrl, available }`
- `app/(marketing)/download/page.tsx`: refactorizado para llamar `getReleaseMetadata()` y pasar `release` a `DownloadClient`
- `components/download/DownloadCard.tsx`:
  - Botón "Copiar SHA256" (icono clipboard) junto al checksum con feedback "Copiado" durante 2 s
  - Nuevo componente `HashVerifyBlock`: muestra comando `Get-FileHash` con el nombre real del archivo y el hash esperado
  - Prop `msiUrl?: string | null`: renderiza botón secundario `.msi` si está definida
- `components/download/DownloadClient.tsx`:
  - Banner neutral "Instalador sin firma Authenticode (temporal)" sobre las tarjetas
  - Incluye `HashVerifyBlock` debajo de `ReleaseInfo`
  - Prop cambiada a `release: ReleaseMetadata`
- `components/download/index.ts`: exporta `HashVerifyBlock`
- `lib/i18n/translations.ts`: 9 nuevas claves EN/ES (`copyChecksum`, `copiedChecksum`, `buttonMsi`, `unsignedBanner`, `verifyHash.*`)

**Scripts y docs:**

- `scripts/generate_sha256.ps1` (nuevo): recibe `-InstallerPath`, escribe `.sha256` al lado, imprime hash y acción siguiente (actualizar Vercel)
- `docs/ENTERPRISE_DEPLOY.md`: nueva subsección "Generating a .sha256 file" en §7 con documentación del script
- `docs/RELEASE_CHECKLIST.md` (nuevo): checklist completo (build → hash → upload → Vercel env → smoke)
- `gitgov-web/tests/e2e/download-url.mjs` (nuevo): smoke test Node.js sin dependencias externas; verifica shape de `/api/release-metadata` y URL externa cuando `NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL` está definida

### Validación ejecutada

- `npm run typecheck` → sin errores
- `npm run lint` → `✔ No ESLint warnings or errors`

---

## Actualización Reciente (2026-02-28) — Updater Desktop apuntando a GitHub Releases

### Qué se implementó
- El endpoint OTA del plugin updater en Tauri se cambió a GitHub Releases:
  - `https://github.com/MapfrePE/GitGov/releases/latest/download/latest.json`
- El fallback manual del updater ahora usa `https://github.com/MapfrePE/GitGov/releases/latest`.
- `getDesktopUpdateFallbackUrl()` se endureció para no concatenar `/stable` cuando la URL base ya es un destino directo (`/releases/latest`, `.exe` o `.json`).
- Se recompiló Desktop local (`npx tauri build`) y se regeneró firma updater + `latest.json` (timestamp actualizado).
- Pendiente operativo manual: subir al release `v0.1.0` los archivos actualizados:
  - `gitgov/src-tauri/target/release/bundle/nsis/GitGov_0.1.0_x64-setup.exe.sig`
  - `release/desktop/stable/latest.json`

## Actualización Reciente (2026-02-28) — Fix de descarga en Web Deploy (URL externa)

### Qué se implementó
- `gitgov-web` ahora soporta descarga del Desktop por URL externa configurable:
  - Nueva configuración: `NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL`.
  - Si está definida, `siteConfig.downloadPath` usa esa URL en lugar de `/downloads/...`.
- `app/(marketing)/download/page.tsx` ya no bloquea el botón cuando el instalador se hospeda fuera de `public/`:
  - En modo URL externa (`http/https`), marca `available: true` sin hacer `fs.stat` local.
  - Mantiene el comportamiento anterior para artefactos locales en `public/downloads`.

### Pendiente explícito (comercial)
- **Code signing Authenticode OV/EV**: diferido hasta primer cliente pago por restricción de presupuesto.
- Estado actual: descarga funcional vía GitHub Releases, con posible advertencia de SmartScreen en Windows.
- Acción futura: adquirir certificado de code signing, configurar secretos CI (`WINDOWS_CERTIFICATE*`) y publicar instaladores firmados.

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

---

## 2026-03-01 - Conversational Chat hardening

- Se corrigió scope de organización en la consulta de chat para commits por usuario (`chat_query_user_commits_range`) agregando filtro `org_id` opcional y propagándolo desde `chat_ask`.
- Impacto: evita mezcla de commits entre organizaciones cuando la API key está scopeada por org.
- Se migró proveedor LLM de chat desde Anthropic a Gemini API (`GEMINI_API_KEY`) en backend (`main.rs` + `handlers.rs`) usando `generateContent` con salida JSON.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`

## 2026-03-01 - Timezone UI review hardening

- Revisión de implementación de zona horaria configurable en frontend.
- Fix 1: `formatTs/formatTimeOnly/formatDateOnly` ahora acepta timestamp `0` correctamente (`epochMs == null` en vez de `!epochMs`).
- Fix 2: persistencia de timezone robusta (`readStoredTimezone`/`persistTimezone`) con guardas de `window/localStorage` y validación IANA para evitar fallos en entornos restringidos.
- Store actualizado para usar helpers centralizados de timezone (`useControlPlaneStore`).
- Validación ejecutada:
  - `cd gitgov && npm run typecheck` -> sin errores
  - `cd gitgov && npx eslint src/lib/timezone.ts src/store/useControlPlaneStore.ts` -> sin errores

## 2026-03-01 - Retención configurable (compliance)

- Se agregó política explícita de retención de auditoría con mínimo legal de 5 años:
  - nuevo env `AUDIT_RETENTION_DAYS` (se clamp a mínimo `1825` días).
  - log de arranque con política efectiva cargada.
- Se separó retención de sesiones efímeras del concepto de retención de auditoría:
  - nuevo env `CLIENT_SESSION_RETENTION_DAYS`.
  - compatibilidad hacia atrás: `DATA_RETENTION_DAYS` se mantiene como fallback.
- No se agregó borrado de tablas de auditoría (append-only intacto).
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`

## 2026-03-01 - Fallback de rol admin para servidores legacy

- Se corrigió un bloqueo de UX en Control Plane cuando el backend no expone `GET /me` (retorna `404`), situación que forzaba erróneamente la vista Developer.
- Nuevo fallback en frontend (`loadMe`):
  - intenta `cmd_server_get_me`;
  - si falla, intenta `cmd_server_get_stats`;
  - si `stats` responde, asigna rol `Admin`; si no, `Developer`.
- Objetivo: compatibilidad con servidores legacy sin perder acceso al onboarding/panel admin cuando la API key sí es admin.
- Validación ejecutada:
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-01 - Saneo de datos de prueba + hardening anti-contaminación

- Saneo operativo en base de datos del entorno activo:
  - se eliminaron eventos sintéticos de `client_events` (patrones `dev_team_`, `e2e_`, `alias_`, `user_*`, `test_*`, `golden_*`, `smoke`, `manual-check`, `victim_`, etc.).
  - respaldo CSV previo en raíz del workspace: `test_data_backup_20260301_032754.client_events.csv` y `test_data_backup_20260301_032754.github_events.csv`.
- Hardening backend:
  - nuevo flag env `GITGOV_REJECT_SYNTHETIC_LOGINS` (default `false`) para rechazar ingesta `/events` con `user_login` sintético.
  - cambios en `handlers.rs` y wiring en `main.rs`.
- Hardening métrica:
  - nueva migración `gitgov/gitgov-server/supabase/supabase_schema_v12.sql` para excluir logins sintéticos de `active_devs_week` en `get_audit_stats`.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`

## 2026-03-01 - Chatbot ampliado a modo conocimiento del proyecto

- El endpoint `/chat/ask` ahora tiene fallback de **modo conocimiento** cuando la pregunta no cae en una query SQL analítica.
- Se actualizó el system prompt para permitir respuestas sobre:
  - integraciones (GitHub/Jira/Jenkins/GitHub Actions),
  - configuración operativa,
  - troubleshooting,
  - FAQ del proyecto.
- Se agregó base de conocimiento interna (`PROJECT_KNOWLEDGE_BASE`) con snippets operativos y selección por keywords.
- Las 3 queries SQL originales se mantienen intactas.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`

## 2026-03-01 - Settings: mover onboarding admin y gestión de equipo

- Se movieron los paneles de administración desde `Control Plane > Dashboard` hacia `Settings`:
  - `AdminOnboardingPanel`
  - `TeamManagementPanel`
  - `ApiKeyManagerWidget`
- `ExportPanel` **no** se movió (se mantiene fuera de Settings) según requerimiento.
- `ServerDashboard` quedó enfocado en métricas, actividad y chatbot.
- `SettingsPage` ahora muestra un bloque "Administración de Organización" solo para rol admin del Control Plane.
- Si no hay conexión activa al Control Plane, Settings muestra CTA para abrir `/control-plane` y conectar.
- Ajuste de layout en Settings para soportar tablas/paneles amplios (`max-w-6xl`).
- Validación ejecutada:
  - `cd gitgov && npm run typecheck` -> sin errores
  - `cd gitgov && npx eslint src/pages/SettingsPage.tsx src/components/control_plane/ServerDashboard.tsx` -> sin errores
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`

## 2026-03-01 - Chatbot Gemini: modelo configurable

- Se eliminó hardcode de modelo Gemini en backend.
- Nuevo env `GEMINI_MODEL` con default `gemini-2.5-flash`.
- Motivo: `gemini-2.0-flash` devuelve `404 NOT_FOUND` para proyectos nuevos.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores

## 2026-03-01 - Chatbot: conteo de commits por usuario (Control Plane real)

- Se corrigió el query engine del chatbot para soportar preguntas de conteo tipo:
  - "¿Cuántos commits ha hecho el usuario X?"
  - "How many commits did user X make this week/month?"
- Cambios backend:
  - Nuevo `ChatQuery::UserCommitsCount` en `handlers.rs`.
  - Parser de intención mejorado para extraer login con patrones `usuario X`, `el usuario X`, `commits de X`, `commits by X`.
  - Soporte de ventana temporal en conteo:
    - `esta semana` / `this week`
    - `este mes` / `this month`
    - rango explícito `entre <fecha> y <fecha>`
    - default de conteo sin ventana -> all-time.
  - Nueva query DB `chat_query_user_commits_count(...)` en `db.rs` sobre `client_events` con scope por `org_id`.
- Mejora de conocimiento contextual:
  - Se añadió snippet "Control Plane datos" para evitar respuestas ambiguas de capacidad.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores

## 2026-03-01 - Chatbot: ampliacion fuerte de contexto de proyecto

- Se amplió significativamente `PROJECT_KNOWLEDGE_BASE` del chatbot (backend) para cubrir más contexto operativo y funcional:
  - arquitectura general
  - endpoints clave de Control Plane
  - auth/scope/roles
  - onboarding admin y gestión de equipo
  - API keys
  - GitHub/Jenkins/Jira/GitHub Actions
  - OAuth Device Flow
  - outbox/reintentos
  - Golden Path y eventos
  - branch protection, signals/violations
  - deploy EC2 + CI/CD Jenkins
  - rate limits
  - timezone/retención/compliance
  - higiene de datos sintéticos
  - troubleshooting de chatbot (404/401/429/modelo Gemini)
  - feature requests desde chat
- Mejora en payload de conocimiento (`build_project_knowledge_payload`):
  - mayor cobertura de snippets seleccionados (fallback y top-ranked)
  - scoring incluye coincidencia por título + keywords
  - se añade bloque `capabilities` (query engine, integraciones, auth/scope, limits)
- Se mantiene regla de no inventar: si no hay datos suficientes o capacidad no implementada, sigue devolviendo `insufficient_data` / `feature_not_available`.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores

## 2026-03-01 - Chatbot v2: contexto ampliado + respuestas directas (saludos/fecha/hora/guía)

- Se llevó el chatbot a un nivel más completo en backend:
  - Base de conocimiento ampliada con más contexto de producto (Settings admin, roles/scope, PR/merges, docs/FAQ, onboarding, integraciones, troubleshooting, compliance, deploy).
  - Nuevas intenciones conversacionales directas (sin depender del LLM para todo):
    - `Greeting`
    - `CurrentDateTime`
    - `CapabilityOverview` (capacidad real del Control Plane)
    - `GuidedHelp` (respuestas paso a paso según tema: GitHub/Jenkins/Jira/Settings)
  - Se añadió metadata runtime al payload de conocimiento (`now_utc_iso`, `now_lima_iso`, `weekday_lima_es`, `timezone_hint`) para respuestas de día/hora.
- Objetivo: evitar respuestas pobres tipo `insufficient_data` en saludos o preguntas generales y mejorar guidance accionable.
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `52 passed; 0 failed`
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores
