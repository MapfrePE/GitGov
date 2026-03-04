# GitGov - Registro de Progreso

---

## Actualización (2026-03-04) — Documentación: Golden Path incluye chatbot

### Qué se actualizó (solo docs)
- `docs/GOLDEN_PATH_CHECKLIST.md`
  - Se añadió el chatbot como parte explícita del flujo crítico del Golden Path.
  - Se agregó sección de validación de chatbot para Admin:
    - `POST /chat/ask` operativo sin crash
    - respuestas con datos verificables (sin inventar)
    - comportamiento correcto cuando faltan datos (`insufficient_data`)
    - contraste manual con `GET /admin-audit-log` para logs/acciones ("tags") de admin

### Sin cambios de código
- No se modificó lógica de backend/frontend en esta actualización.

---

## Actualización (2026-03-04) — Hotfix de rendimiento: comandos Tauri no bloquean UI

### Causa raíz identificada
- El cliente de Control Plane en Tauri usa `reqwest::blocking` (`gitgov/src-tauri/src/control_plane/server.rs`), pero varios `#[tauri::command]` seguían en modo síncrono.
- Bajo carga (orgs grandes + consultas concurrentes), esos comandos podían bloquear el runtime de comandos y provocar congelamientos visibles (`No responde`).

### Qué se implementó
- `gitgov/src-tauri/src/commands/server_commands.rs`
  - Se completó migración de comandos `cmd_server_*` restantes a `pub async fn` usando `run_blocking_command(...)` (wrapper sobre `tauri::async_runtime::spawn_blocking`).
  - Comandos migrados en este bloque:
    - `cmd_server_send_event`
    - `cmd_server_create_org`
    - `cmd_server_create_org_user`
    - `cmd_server_list_org_users`
    - `cmd_server_update_org_user_status`
    - `cmd_server_create_api_key_for_org_user`
    - `cmd_server_create_org_invitation`
    - `cmd_server_list_org_invitations`
    - `cmd_server_resend_org_invitation`
    - `cmd_server_revoke_org_invitation`
    - `cmd_server_preview_org_invitation`
    - `cmd_server_accept_org_invitation`
    - `cmd_server_list_api_keys`
    - `cmd_server_revoke_api_key`
    - `cmd_server_export`
    - `cmd_server_list_exports`

### Validación ejecutada
- `cd gitgov/src-tauri && cargo check` → sin errores
- `cd gitgov/src-tauri && cargo test` → `0 passed; 0 failed`
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/layout/Header.tsx src/components/control_plane/ServerDashboard.tsx src/components/control_plane/ServerConfigPanel.tsx` → 0 errores
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`

### Validación Golden Path (runtime local 127.0.0.1:3000)
- `POST /events` con `Authorization: Bearer`:
  - `accepted=1`, `duplicates=0`, `errors=0`
- `GET /stats` con `Authorization: Bearer`:
  - respuesta JSON válida (`github_total=0`, `client_total=334`, `active_repos=0`)
- `GET /logs?limit=5&offset=0` con `Authorization: Bearer`:
  - `events_count=5`

### Impacto
- Se elimina bloqueo sincrónico en capa de comandos Tauri para operaciones de Control Plane.
- Se mantiene comportamiento funcional (mismos endpoints, mismo payload, auth `Bearer`, sin recortar features).

---

## Actualización (2026-03-04) — Hotfix de rendimiento: refresh pesado desacoplado + heartbeat liviano

### Causa raíz identificada
- El auto-refresh del dashboard (`cada 30s`) estaba ejecutando en paralelo consultas pesadas (`jenkins correlations`, `PR merge evidence`, `ticket coverage`) junto con el refresh base.
- El heartbeat de conexión del header también corría cada 30s con estado de carga visible, aumentando renders globales innecesarios.
- En orgs grandes, este patrón produce picos de CPU/red y congelamientos intermitentes de UI.

### Qué se implementó
- `gitgov/src/store/useControlPlaneStore.ts`
  - `refreshDashboardData(...)` ahora separa:
    - **Core refresh** (siempre): `stats`, `daily activity`, `logs`.
    - **Heavy refresh** (throttled): `jenkins correlations`, `pr merges`, `ticket coverage`.
  - Se añadió TTL para refresh pesado: `HEAVY_DASHBOARD_REFRESH_MS = 5 min`.
  - Se añadió `forceHeavy` para refresco manual explícito desde UI.
  - `checkConnection(...)` acepta `{ background?: boolean }`:
    - en background evita “loading churn” innecesario en heartbeat.
  - `refreshForCurrentRole(...)` acepta `{ forceHeavy?: boolean }` y lo propaga.
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
  - El botón manual de refresh ahora fuerza refresh pesado (`forceHeavy: true`).
  - Auto-refresh mantiene solo path normal (rápido).
- `gitgov/src/components/layout/Header.tsx`
  - Heartbeat de conexión cada 30s en modo background (`checkConnection({ background: true })`).
- `gitgov/src/components/control_plane/ServerConfigPanel.tsx`
  - Ajuste de handlers (`onClick`) para nueva firma de `checkConnection`.

### Validación ejecutada
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/layout/Header.tsx src/components/control_plane/ServerDashboard.tsx src/components/control_plane/ServerConfigPanel.tsx` → 0 errores
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`

### Validación Golden Path (runtime local 127.0.0.1:3000)
- `POST /events` con `Authorization: Bearer`:
  - `accepted=1`, `duplicates=0`, `errors=0`
- `GET /stats` con `Authorization: Bearer`:
  - `stats_ok=true`
- `GET /logs?limit=5&offset=0` con `Authorization: Bearer`:
  - `logs_count=5`

### Impacto
- Se reduce carga periódica de frontend/backend sin romper contratos ni auth.
- Escala mejor para orgs con más repos/devs/eventos al evitar consultas pesadas en cada tick.

---

## Actualización (2026-03-03) — Chat con múltiples conversaciones (tabs `+` / `x`)

### Qué se implementó
- `gitgov/src/store/useControlPlaneStore.ts`
  - Nuevo modelo de sesiones de chat: `chatSessions` + `activeChatSessionId`.
  - Persistencia por usuario GitHub con schema v2 por sesiones:
    - `gitgov.chat_messages.v2.<github_login>`
    - payload: `{ version: 2, active_session_id, sessions[] }`
  - Migración compatible desde formato anterior (array simple de mensajes) a sesión única.
  - Nuevas acciones:
    - `createChatSession()`
    - `setActiveChatSession(sessionId)`
    - `closeChatSession(sessionId)`
  - `clearChatMessages()` ahora limpia solo la conversación activa.
  - Límite operativo: hasta 8 conversaciones, 80 mensajes por conversación.
- `gitgov/src/components/control_plane/ConversationalChatPanel.tsx`
  - Barra de tabs en el panel del bot:
    - botón `+` para nueva conversación
    - botón `x` por tab para cerrar
    - cambio de conversación manteniendo historial independiente
  - El título de cada tab se infiere de la primera pregunta del usuario.

### Validación ejecutada
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ConversationalChatPanel.tsx` → 0 errores
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`

### Validación Golden Path (runtime local 127.0.0.1:3000)
- `POST /events` con `Authorization: Bearer`:
  - `accepted=1`, `duplicates=0`, `errors=0`
- `GET /stats` con `Authorization: Bearer`:
  - `stats_ok=true`
- `GET /logs?limit=5&offset=0` con `Authorization: Bearer`:
  - `logs_count=5`

### Impacto
- El chatbot ahora soporta varias conversaciones en el mismo espacio (tipo tabs), con apertura/cierre individual y estado persistente por usuario.
- No se modificó auth (`Authorization: Bearer`) ni contratos compartidos (`ServerStats`, `CombinedEvent`).

---

## Actualización (2026-03-03) — Historial de chatbot aislado por usuario GitHub

### Causa raíz identificada
- Aunque se corrigió la limpieza en `disconnect()`, el historial seguía compartido por una sola key global de `localStorage`.
- En escenarios de cambio de usuario, la nueva sesión podía heredar conversación de otra cuenta.

### Qué se implementó
- `gitgov/src/store/useControlPlaneStore.ts`
  - Se migró el storage a key por usuario: `gitgov.chat_messages.v2.<github_login>`.
  - Se mantiene migración legacy (`gitgov.chat_messages`) solo cuando aún no existen keys scopeadas, evitando fuga de historial a cuentas adicionales.
  - Se añadió `refreshChatMessagesForActiveUser()` para recargar historial activo según usuario autenticado.
- `gitgov/src/components/layout/MainLayout.tsx`
  - Nuevo efecto que recarga el historial al cambiar `user.login`, garantizando aislamiento entre cuentas.

### Validación ejecutada
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/layout/MainLayout.tsx` → 0 errores
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`

### Validación Golden Path (runtime local 127.0.0.1:3000)
- `POST /events` con `Authorization: Bearer`:
  - `accepted=1`, `duplicates=0`, `errors=0`
- `GET /stats` con `Authorization: Bearer`:
  - `stats_ok=true`
- `GET /logs?limit=5&offset=0` con `Authorization: Bearer`:
  - `logs_count=5`

### Impacto
- Cada usuario ve y persiste su propio historial de chat.
- No se alteró auth (`Authorization: Bearer`) ni contratos compartidos (`ServerStats`, `CombinedEvent`).

---

## Actualización (2026-03-03) — Corrección de persistencia del historial del chatbot

### Causa raíz identificada
- El store sí persistía el chat en `localStorage` (`CHAT_MESSAGES_STORAGE_KEY`), pero `disconnect()` lo borraba en cada desconexión.
- `disconnect()` también se ejecuta en flujos automáticos de sesión (por ejemplo, cuando `MainLayout` detecta que la sesión GitHub no está autenticada), no solo en “clear chat”.
- Resultado: al reautenticar/reiniciar, el historial desaparecía aunque el usuario no lo hubiera limpiado manualmente.

### Qué se implementó
- `gitgov/src/store/useControlPlaneStore.ts`
  - `disconnect()` ya no ejecuta `persistChatMessages([])`.
  - `disconnect()` ya no fuerza `chatMessages: []` en el estado.
  - Se mantiene `clearChatMessages()` como única acción explícita para borrar historial.

### Validación ejecutada
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts` → 0 errores
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`

### Validación Golden Path (runtime local 127.0.0.1:3000)
- `POST /events` con `Authorization: Bearer`:
  - `accepted=1`, `duplicates=0`, `errors=0`
- `GET /stats` con `Authorization: Bearer`:
  - `stats_ok=true` (JSON válido con `github_events` y `client_events`)
- `GET /logs?limit=5&offset=0` con `Authorization: Bearer`:
  - `logs_count=5`

### Impacto
- El historial del chatbot vuelve a persistir entre reinicios/reautenticación.
- No se alteró auth (`Authorization: Bearer`) ni contratos compartidos (`ServerStats`, `CombinedEvent`).

---

## Actualización (2026-03-03) — Mitigación de congelamiento “No responde” en Control Plane

### Causa raíz identificada (evidencia técnica)
- El auto-refresh del dashboard de Control Plane ejecutaba cada 30s, para Admin, un bloque de carga que incluía datos de Settings (`org_users`, `org_invitations`, `team_overview`, `team_repos`) aunque esa vista no estuviera abierta.
- El efecto de `ServerDashboard` dependía de `isChatLoading`, así que al cerrar una respuesta del bot se disparaba un refresh completo adicional.
- `/logs` en backend enriquecía eventos cliente con `WHERE id::text = ANY($1::text[])`, patrón que degrada uso de índice sobre UUID y escala mal cuando crece `client_events`.
- La tabla de commits recalculaba estructuras pesadas por render (`buildDashboardRows` y correlaciones), amplificando el costo cuando había varios updates de estado seguidos.

### Qué se implementó
- `gitgov/src/store/useControlPlaneStore.ts`
  - Guardas de concurrencia para evitar corridas solapadas en `checkConnection` y `refreshForCurrentRole`.
  - `refreshForCurrentRole` para Admin se enfocó en dashboard core (se removió refresh periódico de datos de Settings).
  - `refreshDashboardData` dejó de hacer una segunda llamada `/logs` para `activeDevs7d`; ahora deriva esa métrica desde `serverLogs` ya cargados.
  - Se agregó helper `buildActiveDevs7dFromLogs(...)` para cálculo consistente y reutilizable.
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
  - El loop de auto-refresh usa `ref` para `isChatLoading`; ya no reconfigura ni dispara refresh inmediato por cada transición del chat.
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx`
  - Se añadieron `useMemo`/`useCallback` y `React.memo` para evitar recomputación completa en renders no relacionados.
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx` (ajuste inmediato)
  - Se retiró `React.memo` a nivel export para evitar error runtime observado en dev (`TypeError: Component is not a function`) y se mantuvieron optimizaciones internas con `useMemo`/`useCallback`.
- `gitgov/src/components/control_plane/dashboard-helpers.ts`
  - `buildDashboardRows` pasó de enfoque de búsqueda doble a procesamiento lineal con cola por usuario para emparejar `stage_files`→`commit`.
- `gitgov/gitgov-server/src/db.rs`
  - Optimización de enriquecimiento en `get_combined_events`: `id = ANY($1::uuid[])` en lugar de `id::text = ANY($1::text[])`.

### Archivos
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/components/control_plane/ServerDashboard.tsx`
- `gitgov/src/components/control_plane/RecentCommitsTable.tsx`
- `gitgov/src/components/control_plane/dashboard-helpers.ts`
- `gitgov/gitgov-server/src/db.rs`

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerDashboard.tsx src/components/control_plane/RecentCommitsTable.tsx src/components/control_plane/dashboard-helpers.ts` → 0 errores
- `cd gitgov/gitgov-server && npx eslint src/db.rs` → 0 errores (warning esperado: archivo `.rs` ignorado por ESLint config)

### Validación Golden Path (runtime local 127.0.0.1:3000)
- `POST /events` con `Authorization: Bearer` y evento `commit` de prueba:
  - `accepted=1`, `duplicates=0`, `errors=0`
- `GET /stats` con `Authorization: Bearer`:
  - responde JSON válido (`ServerStats`) sin `401`
- `GET /logs?limit=5&offset=0` con `Authorization: Bearer`:
  - responde `events` (conteo observado: 5) sin error de deserialización

### Impacto Golden Path
- No se modificó auth (`Bearer`) ni contrato compartido (`ServerStats`, `CombinedEvent`).
- Se redujo carga de refresh en dashboard para evitar bloqueos de UI sin romper ingestión `/events` ni lectura `/stats` y `/logs`.

---

## Actualización (2026-03-03) — Chat Founder/Admin: cobertura integral del Control Plane + validación live

### Qué se implementó
- Se amplió el query engine conversacional para consultas ejecutivas de Control Plane:
  - `ControlPlaneExecutiveSummary`
  - `OnlineDevelopersNow`
  - `CommitsWithoutTicketWindow`
- Se conectaron estas consultas al handler del chat con respuesta determinística (sin inventar), usando SQL real y `data_refs`.
- Se mantuvo la excepción founder global (`bootstrap-admin`, `Admin`, sin `org_id`) para resolver consultas analíticas sin `org_name` explícito.

### Evidencia técnica (código)
- Detección de nuevas intenciones:
  - `gitgov/gitgov-server/src/handlers/conversational/core.rs:765-770`
  - `gitgov/gitgov-server/src/handlers/conversational/query.rs:221-331`
- Capacidades expuestas al payload de conocimiento:
  - `gitgov/gitgov-server/src/handlers/conversational/engine.rs:100-102`
- Scope y excepción founder:
  - `gitgov/gitgov-server/src/handlers/chat_handler.rs:1-6`
  - `gitgov/gitgov-server/src/handlers/chat_handler.rs:22`
  - `gitgov/gitgov-server/src/handlers/chat_handler.rs:445-448`
- Respuestas determinísticas nuevas:
  - `gitgov/gitgov-server/src/handlers/chat_handler.rs:562-767` (resumen ejecutivo)
  - `gitgov/gitgov-server/src/handlers/chat_handler.rs:769-858` (devs ON, commits sin ticket)
- SQL de métricas nuevas:
  - `gitgov/gitgov-server/src/db.rs:4685` (`chat_query_pushes_no_ticket_count`)
  - `gitgov/gitgov-server/src/db.rs:4715-4739` (`chat_query_commits_without_ticket_count`)
  - `gitgov/gitgov-server/src/db.rs:4743-4764` (`chat_query_online_developers_count`)

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `79 passed; 0 failed`
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov/gitgov-server && npx eslint src/db.rs src/handlers/chat_handler.rs src/handlers/conversational/core.rs src/handlers/conversational/query.rs src/handlers/conversational/engine.rs src/handlers/tests.rs`
  - Resultado: sin errores (warnings de “File ignored” por configuración ESLint no aplicable a `.rs`)

### Validación live (runtime real en 127.0.0.1:3000)
- `POST /chat/ask` — `"cuantos devs hay on ahora en control plane?"`
  - `status=ok`, respuesta con conteo real (`Developers ON detectados: 1`)
- `POST /chat/ask` — `"cuantos commits sin ticket hubo esta semana?"`
  - `status=ok`, respuesta con conteo real (`36 en 168 horas`)
- `POST /chat/ask` — `"dame todo lo que hay en el control plane, resumen ejecutivo"`
  - `status=ok`, respuesta con bloque ejecutivo y scope `founder/global`
- `POST /chat/ask` — `"cual fue el ultimo commit del usuario mapfrepe?"`
  - `status=ok`, devuelve SHA `54cdab4...`, rama `main`, mensaje `feat: XD`, hora Lima+UTC coherente

### Validación de sincronización temporal (/events → /logs → chat)
- Se inyectó evento `commit` manual con timestamp fijo `2026-03-01T11:35:43Z` (ms: `1772364943000`) vía `POST /events`.
- `GET /logs?event_type=commit` devolvió el mismo evento con:
  - `details.event_uuid` exacto
  - `created_at = 1772364943000` (match exacto con timestamp enviado)
- `POST /chat/ask` para `sync_probe_user` devolvió:
  - `Fecha del evento: 2026-03-01 06:35:43 (America/Lima) | 2026-03-01 11:35:43 UTC`

### NO VERIFICADO
- Restricción runtime para **admin no-founder sin org_name** en este entorno live con una key alterna no founder.
  - Bloqueador: solo hubo key founder/admin disponible para pruebas live.
  - Cobertura parcial existente: test unitario de excepción founder en `gitgov/gitgov-server/src/handlers/tests.rs:269-290`.

---

## Actualización (2026-03-03) — Login founder/admin estable + cancelación en Device Flow

### Problema corregido
- El Control Plane podía quedar en vista `Developer` por fallback erróneo cuando `/me` fallaba.
- La `GITGOV_API_KEY` podía quedar inválida (`401`) si existía inactiva/revocada: al iniciar el server se intentaba insertar y fallaba por `duplicate key`.
- El login GitHub Device Flow no tenía acción de cancelación durante espera/polling.

### Qué se implementó
- **Backend startup (`main.rs` + `db.rs`)**
  - Nuevo `ensure_admin_api_key(...)` que hace upsert por `key_hash` y fuerza:
    - `role = Admin`
    - `org_id = NULL`
    - `is_active = TRUE`
    - `client_id = bootstrap-admin`
  - `GITGOV_API_KEY` ahora se asegura como key founder/admin activa en cada arranque (ya no se rompe por `duplicate key` en keys inactivas).
- **Frontend Control Plane (`useControlPlaneStore.ts`)**
  - `loadMe()` deja de degradar a `Developer` cuando falla auth.
  - Si `/me` falla, solo usa fallback de compatibilidad a `/stats` para servers antiguos; si no, deja rol en `null` con error explícito de API key inválida.
  - `checkConnection()` ahora valida contexto de rol antes de marcar conexión `connected`.
  - Si falla auth, intenta auto-recuperar con `VITE_API_KEY` (si es distinta de la key actual) y reintenta `/me`.
- **Login UX (`useAuthStore.ts`, `LoginScreen.tsx`)**
  - Se agregó `cancelAuth()` para abortar Device Flow.
  - Se limpia correctamente el timer de polling para evitar estados colgados.
  - Botón **Cancelar** visible en estados `waiting_device` y `polling`.
- **Claridad de identidad Founder/Admin en UI (`ServerConfigPanel.tsx`, `SettingsPage.tsx`, `useControlPlaneStore.ts`)**
  - Se añadió identidad visible de Control Plane (`role` + `client_id`) para eliminar ambigüedad entre login GitHub y rol API key.
  - Se añadió acción explícita **`Usar Founder/Admin (.env)`** / **`Forzar Founder/Admin (.env)`** para aplicar `VITE_API_KEY` y reconectar automáticamente.
  - Se mostró advertencia cuando la sesión está en rol no-admin aunque el usuario espere acceso founder/admin.
- **Separación explícita GitHub vs Control Plane identity (`LoginScreen.tsx`, `useControlPlaneStore.ts`)**
  - Se añadió login alternativo desde pantalla de autenticación: **Entrar con API key** (sin depender de Device Flow).
  - Se implementó validación fuerte de identidad cruzada:
    - `Device Flow` autentica GitHub usuario.
    - `/me` autentica API key y devuelve `client_id/role`.
    - Si `client_id` de API key no coincide con `login` de GitHub, se bloquea el rol Control Plane y se muestra error de identidad (sin fallback silencioso).
  - Excepción founder controlada: `bootstrap-admin` solo se permite para login founder configurado por `VITE_FOUNDER_GITHUB_LOGIN` (o `VITE_FOUNDER_LOGIN`).
- **Ajuste de UX por flujo correcto (2026-03-03, iteración)**
  - Se removió el formulario de API key de la pantalla inicial de login.
  - Nuevo paso intermedio **post-Device Flow**: `ControlPlaneAuthScreen` (Paso 2 de 2), donde se valida API key de Control Plane después de autenticar GitHub.
  - Este flujo evita mezclar “iniciar sesión GitHub” con “autenticación de rol/scope Control Plane” en una sola pantalla.
  - Se volvió **obligatorio** el paso de validación de Control Plane si no hay rol/sesión CP válida.
  - Se añadió selector de perfil en Paso 2 (`Founder`, `Admin Org`, `Developer`) con copy específico para cada caso.
  - Reforzado según QA:
    - El botón de avance ahora exige verificación previa de identidad con `/me`.
    - Se muestra explícitamente el resultado exacto de `/me` antes de continuar (`client_id`, `role`, `org_id`).
    - Si perfil es `Admin Org`, se exige `org_name` activo y se muestra junto al resultado.
    - Se añadió confirmación final previa al repo selector: **"Sesión CP validada como X en Y"** con botón explícito de continuar.
    - Se cerró fuga de estado: al perder sesión GitHub, se resetea el gate de confirmación para evitar confirmaciones stale.
  - Log hygiene Tauri dev:
    - Se ajustó el logger de `src-tauri` para usar `EnvFilter` con default `info` y librerías de red en `warn`.
    - Objetivo: suprimir ruido de desarrollo tipo `client connection error ... ConnectionReset (10054)` durante HMR de Vite sin ocultar errores reales de aplicación.

### Archivos
- `gitgov/gitgov-server/src/db.rs`
- `gitgov/gitgov-server/src/main.rs`
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/store/useAuthStore.ts`
- `gitgov/src/components/auth/LoginScreen.tsx`

### Validación ejecutada
- `cd gitgov && npm run typecheck` -> sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/store/useAuthStore.ts src/components/auth/LoginScreen.tsx` -> 0 errores
- `cd gitgov/gitgov-server && cargo test` -> `76 passed; 0 failed`
- `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores

---

## Actualización (2026-03-03) — Excepción founder para chat global sin org_name

### Qué se implementó
- En `POST /chat/ask`, se añadió excepción de scope **solo** para la key founder global:
  - `client_id = bootstrap-admin`
  - `role = Admin`
  - `org_id = None`
- Con esa combinación, el chat ya no devuelve `This query needs an organization scope...` y permite consultas analíticas sin `org_name`.
- La excepción **no aplica** a otros admins globales ni a keys scopeadas por org.

### Archivos
- `gitgov/gitgov-server/src/handlers/chat_handler.rs`
- `gitgov/gitgov-server/src/handlers/tests.rs`

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` -> `76 passed; 0 failed`
- `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores

---

## Actualización (2026-03-03) — Bloqueo anti-AWS en desarrollo

### Qué se implementó
- Se forzó el Control Plane URL en modo `dev` a `http://127.0.0.1:3000` desde el store, ignorando:
  - input manual de URL,
  - `VITE_SERVER_URL`,
  - URL persistida en localStorage.
- Se bloqueó el input de URL en el panel de conexión cuando `import.meta.env.DEV === true` y se muestra aviso visual de que la URL está fija en local.
- Se ajustó `.env` de frontend para desarrollo local por defecto:
  - `VITE_SERVER_URL=http://127.0.0.1:3000`

### Archivos
- `gitgov/src/store/useControlPlaneStore.ts`
- `gitgov/src/components/control_plane/ServerConfigPanel.tsx`
- `gitgov/.env`

### Validación ejecutada
- `cd gitgov && npm run typecheck` -> sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerConfigPanel.tsx` -> 0 errores
- `cd gitgov/gitgov-server && cargo test` -> `75 passed; 0 failed`

---

## Actualización (2026-03-03) — Sincronización real de hora de eventos en chat/dashboard

### Problema corregido
- El Control Plane recibía `timestamp` desde Desktop en `POST /events`, pero los `INSERT` en `client_events` no persistían ese valor en `created_at`.
- Resultado: el dashboard (`/logs`) y el chat de "último commit" ordenaban por hora de ingesta DB (NOW) en lugar de hora real del evento, generando desfases de día/hora cuando el outbox enviaba con retraso.

### Qué se implementó
- **`gitgov/gitgov-server/src/db.rs`**
  - `insert_client_event(...)` y `insert_client_events_batch_tx(...)` ahora insertan `created_at = to_timestamp(event.created_at / 1000.0)`.
  - `chat_query_user_last_commit(...)` se enriqueció con:
    - `user_name`
    - `event_uuid`
    - `repo_full_name` (join con `repos`)
    - `commit_message` (desde `metadata`)
  - `get_combined_events(...)` ahora incluye `event_uuid` en `details` para eventos `client`, facilitando reconciliación exacta entre ingesta y `/logs`.
  - Orden determinístico reforzado: `ORDER BY c.created_at DESC, c.id DESC`.
- **`gitgov/gitgov-server/src/handlers/chat_handler.rs`**
  - Respuesta de "último commit" ahora incluye, cuando existe: usuario (login + nombre), repo y mensaje de commit, además de hora en America/Lima y UTC.

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `76 passed; 0 failed`
- `cd gitgov && npx tsc -b` → sin errores
- `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ConversationalChatPanel.tsx` → 0 errores
- Validación live local (`cargo run` + API key local):
  - `POST /events` con `timestamp=1710000000000` y `user_login=tsfix...` → aceptado.
  - `GET /logs?event_type=commit&user_login=tsfix...` → `created_at=1710000000000` (match exacto; antes quedaba en hora de ingesta).
  - En esa misma lectura de `/logs`, `details.event_uuid` coincide con el `event_uuid` enviado en `/events`.
  - `POST /chat/ask` (`"cual fue el ultimo commit del usuario mapfrepe?"`) → respuesta `ok` con SHA + mensaje + fecha Lima/UTC coherente con logs.

### Impacto Golden Path
- Cambia el path de ingesta `/events` (persistencia de `created_at`) y el flujo conversacional `/chat/ask` para "último commit".
- No cambia auth (`Authorization: Bearer`) ni contrato de `ServerStats`/`CombinedEvent`.

---

## Actualización (2026-03-03) — Corrección de fechas del chat y “último commit” determinístico

### Problema corregido
- El chat podía devolver fechas incoherentes en respuestas de “último commit” al depender de salida no determinística del LLM.
- Preguntas de reclamo de fechas (ej. “¿cómo es posible 04 si hoy es 03?”) estaban mal clasificadas como consulta de hora actual o ayuda genérica.

### Qué se implementó
- **`gitgov/gitgov-server/src/db.rs`**:
  - Nueva query `chat_query_user_last_commit(user_login, org_id)` (alias-aware) para obtener el commit más reciente con `commit_sha`, `branch` y `timestamp` en ms.
- **`gitgov/gitgov-server/src/handlers/chat_handler.rs`**:
  - Nueva ruta determinística `ChatQuery::UserLastCommit`:
    - responde con SHA, rama y fecha exacta en **America/Lima** y **UTC**.
  - Nueva ruta `ChatQuery::DateMismatchClarification` para explicar desfases de fecha por zona horaria/LLM y evitar respuestas irrelevantes.
- **`gitgov/gitgov-server/src/handlers/conversational/query.rs`**:
  - Detección explícita de consultas de “último commit”.
  - Detección explícita de reclamos de inconsistencia de fecha.
  - Se redujo sobre-clasificación:
    - “hora/fecha actual” ya no se activa por cualquier aparición de `hoy/date/time`.
    - `GuidedHelp` ya no se activa por cualquier `como/cómo`.
- **`gitgov/gitgov-server/src/handlers/conversational/core.rs`**:
  - Se añadió `ChatQuery::DateMismatchClarification`.
- **`gitgov/gitgov-server/src/handlers/tests.rs`**:
  - Nuevos tests para:
    - `UserLastCommit`
    - `DateMismatchClarification`
  - Actualización del test de accuracy para incluir ambas clases.

### Validación ejecutada
- `cd gitgov/gitgov-server && cargo test` → `75 passed; 0 failed`
- `cd gitgov && npm run typecheck` → OK

### Impacto Golden Path
- No se modificaron `/events`, `/stats`, `/logs`, auth Bearer ni outbox.
- Cambio acotado al flujo conversacional (`/chat/ask`) y query engine de chat.

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

## 2026-03-01 - Cierre de sesión (deploy real EC2 + validación end-to-end chatbot)

- Deploy manual completado en EC2 del backend actualizado:
  - build release en host Linux (`cargo build --release`)
  - instalación del binario en `/opt/gitgov/bin/gitgov-server`
  - reinicio de servicio `gitgov-server` por systemd
- Problemas resueltos durante despliegue:
  - `404 /chat/ask` por binary viejo en runtime (se corrigió tras redeploy)
  - error Gemini por modelo deprecado (`gemini-2.0-flash`) -> migrado a modelo configurable (`GEMINI_MODEL`) y uso de modelo vigente
  - cuota/billing Gemini habilitada en proyecto correcto para eliminar `429`.
- Validación funcional live en EC2 (`http://127.0.0.1:3000/chat/ask`) con Bearer admin:
  - consulta analítica real: "¿Cuántos commits ha hecho el usuario MapfrePE?" -> `status: ok`, respuesta numérica real, `data_refs: ["client_events"]`
  - saludo: `status: ok` con respuesta conversacional
  - fecha/hora: `status: ok` con hora Lima y UTC
  - guía paso a paso Jira: `status: ok` con instrucciones accionables
  - capacidad de Control Plane: `status: ok` explicando alcance real del bot
- Estado final de sesión:
  - chatbot operativo en producción EC2
  - consultas SQL + guía conversacional funcionando
  - contexto de producto ampliado en backend sin romper tests

## 2026-03-01 - Fix parser de commits en chatbot (evitar falsos usuarios)

- Problema detectado:
  - preguntas como `cuantos commits hay ... de esta sesion` podían interpretar `esta` como si fuera `user_login`, devolviendo conteos incorrectos o respuestas inconsistentes.
  - follow-ups tipo `y del usuario X` / `todo el historial` podían caer en respuesta KB en vez de respuesta analítica clara.
- Cambios backend (`gitgov-server/src/handlers.rs`):
  - nueva función `extract_user_login(...)` con extracción más estricta:
    - prioriza marcadores explícitos (`usuario X`, `del usuario X`, `user X`)
    - fallback solo en contexto de commits (`commits de X`, `commit by X`)
  - `detect_query(...)` ahora usa `trim()` al inicio para robustez en mensajes con espacios iniciales.
  - se mantiene respuesta determinística para intents SQL (`UserCommitsCount`, `UserCommitsRange`, etc.) sin depender de LLM.
  - nuevo intent `NeedUserForCommitHistory` para responder `insufficient_data` útil cuando piden historial sin especificar usuario.
- Resultado esperado:
  - preguntas de conteo por usuario responden con datos reales de `client_events`.
  - disminuyen respuestas genéricas de documentación en consultas analíticas.
  - cobertura de regresión agregada con tests del parser de intención (`detect_query_*`).
- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `54 passed; 0 failed`
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-02 - Chatbot v3 (NLP + contexto dinámico + TODO runtime + aprendizaje)

- Se refactorizó `handlers.rs` para evolucionar el chat desde un flujo estático hacia un módulo conversacional más avanzado:
  - **NLP/Intent Engine**:
    - Nuevos tipos `NlpIntent`, `NlpEntities`, `NlpAnalysis`.
    - Detección de idioma (`detect_language`), extracción de entidades (`user_login`, `repo`, `branch`) y detección de acciones TODO.
    - Intents soportados: greeting, farewell, gratitude, ask_datetime, ask_capabilities, guided_help, query_analytics, todo_add, todo_list, todo_complete, feedback_positive/negative, unknown.
  - **Gestión de contexto dinámica**:
    - Nuevo runtime en memoria `ConversationalRuntime` dentro de `AppState`.
    - Estado por sesión (`ConversationState`) con historial de turnos, slots semánticos, TODOs, aprendizaje y snapshot de proyecto.
    - Clave de sesión por `client_id + org_scope`.
  - **Base de conocimiento estructurada + estado vivo**:
    - Se conserva KB existente y se envuelve en payload conversacional avanzado.
    - Nuevo `refresh_project_snapshot_if_stale(...)` para adjuntar estado live (`stats` + `job_metrics`) al contexto conversacional.
  - **Respuesta contextual con prioridad**:
    - Motor de decisión prioriza: SQL determinístico, TODO runtime, ayuda guiada, luego LLM.
    - Respuestas determinísticas para consultas críticas (pushes sin ticket, bloqueados del mes, commits por usuario/rango).
  - **Integración TODO**:
    - Crear/listar/completar tareas desde chat.
    - Sugerencias proactivas automáticas a TODO según snapshot (bloqueos, violaciones, dead jobs).
  - **Personalidad y consistencia**:
    - Mensajes consistentes para saludo, despedida, feedback positivo/negativo y errores de LLM.
  - **Aprendizaje básico por uso**:
    - Contadores por intent, métricas de éxito/insuficiencia y feedback positivo/negativo por sesión.
    - Preferencia de idioma mantenida en slots.

- Cambios de wiring:
  - `main.rs`: `AppState` ahora inicializa `conversational_runtime`.
  - `handlers.rs`: nueva función `finalize_chat_response(...)` para persistir historial/aprendizaje en todas las salidas del handler.

- Tests añadidos (backend):
  - detección de idioma español.
  - detección NLP de TODO + entidad de usuario.
  - ciclo de vida TODO (add/list/complete).
  - generación de TODOs proactivos desde snapshot.
  - se mantienen tests de parser de commits previos.

- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `58 passed; 0 failed`
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-02 - Chatbot v3.1 (contexto conversacional más profundo + fallback robusto)

- Mejoras aplicadas en `gitgov/gitgov-server/src/handlers.rs`:
  - **Nuevas consultas determinísticas**:
    - `SessionCommitsCount`: responde commits en la sesión conversacional actual, con o sin usuario.
    - `TotalCommitsCount`: responde total de commits del Control Plane dentro del scope de la API key.
  - **Seguimiento conversacional mejorado**:
    - Se agregó `session_started_ms` en `ConversationState`.
    - Se inicializa estado de sesión al primer turno y se usa para resolver preguntas tipo “esta sesión”.
    - Follow-up “en todo el historial” ahora reutiliza `last_user_login` cuando existe contexto previo.
  - **Fallback inteligente sin LLM**:
    - Nuevo ranking reusable de KB (`rank_project_knowledge`).
    - Si falla Gemini (cuota/timeout/error), el bot ya no cae a mensaje genérico: responde con contexto local estructurado y accionable.
  - **Payload de IA más rico**:
    - Se añadió `project_state_summary` con KPIs clave (blocked pushes, commits, devs/repos activos, violaciones, dead jobs).
    - Ajuste de estilo por aprendizaje (`high_precision` vs `balanced`) según feedback acumulado de la sesión.
  - **Cobertura de capacidades declaradas**:
    - KB ahora anuncia explícitamente `session_commits_count` y `total_commits_count` dentro del bloque `query_engine`.

- Cambios en DB (`gitgov/gitgov-server/src/db.rs`):
  - Nuevo método `chat_query_commits_count(start_ms, end_ms, org_id)` para contar commits por ventana temporal y scope de organización.

- Testing ampliado:
  - Nuevos tests para:
    - detección de queries de sesión con/sin usuario,
    - total commits,
    - follow-up corto “en todo el historial”,
    - fallback de conocimiento,
    - benchmark de clasificación del query engine con umbral mínimo `>= 90%`.

- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `63 passed; 0 failed`
  - `cd gitgov/gitgov-server && cargo clippy -- -D warnings` -> sin errores
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-02 - Chat UX fix (persistencia + respuestas de producto)

- Se mejoró la experiencia del chat en dos frentes:
  - gitgov/src/store/useControlPlaneStore.ts:
    - Persistencia local de historial de chat (chatMessages) usando localStorage.
    - Restauración automática del historial al iniciar la app.
    - Persistencia en cada mensaje (usuario, asistente y error) con límite de 100 mensajes.
    - Limpieza consistente del historial persistido en clearChatMessages y disconnect.
  - gitgov/gitgov-server/src/handlers.rs:
    - Nuevas respuestas guiadas determinísticas para preguntas de producto frecuentes: pricing/gratis, descarga desktop, warning de firma en Windows/SmartScreen, disponibilidad por sistema operativo y contacto de soporte.
    - Enrutamiento de intención para estas preguntas hacia GuidedHelp incluso cuando el usuario no usa palabras como "ayuda" o "configurar".

- Objetivo del cambio:
  - Evitar respuestas genéricas/repetitivas para preguntas comerciales y de distribución.
  - Mantener contexto de conversación del lado frontend tras reiniciar la app.
## 2026-03-03 - Chat hardening (menos respuestas genéricas + KB web)

- Se reforzó el comportamiento del asistente en gitgov/gitgov-server/src/handlers.rs para producción:
  - Se añadió una nueva base WEB_FAQ_KNOWLEDGE_BASE con conocimiento de gitgov-web (FAQ/docs públicos) para temas de producto: plataformas soportadas, open source, privacidad del chat, offline/outbox, actualizaciones, integraciones, self-host, pricing/contacto, SmartScreen.
  - ank_project_knowledge(...) ahora rankea de dos fuentes (project_docs_kb + web_docs_faq) y adjunta source por snippet.
  - Se agregó uild_grounded_knowledge_answer(...): respuesta directa basada en KB cuando hay match confiable y la pregunta no requiere datos analíticos en vivo.
  - Se agregó should_override_llm_answer_with_kb(...): si el LLM responde genérico o insufficient_data pero existe KB relevante, se reemplaza por respuesta grounded.
  - Se integró el grounding en dos puntos del flujo:
    - antes de llamar al LLM (respuesta local inmediata para preguntas de producto/documentación),
    - después de la respuesta del LLM (override anti-respuesta-genérica).
  - uild_guided_help_answer(...) ahora intenta grounding KB antes de caer al mensaje genérico "opciones frecuentes".
  - Se amplió el enrutamiento de detect_query(...) para preguntas de producto comunes (open source, código fuente, plataformas, integraciones, soporte, pricing, etc.).
  - uild_project_knowledge_payload(...) ahora incluye snippets con source y mezcla semilla de ambas bases para contexto de modelo más robusto.

- Tests backend agregados:
  - grounded_knowledge_answer_uses_web_faq_for_platform_questions
  - insufficient_llm_answer_is_overridden_when_kb_has_confident_match

- Validación ejecutada:
  - cd gitgov/gitgov-server && cargo test -> 65 passed; 0 failed
  - cd gitgov && npm run typecheck -> sin errores
## 2026-03-03 - UI performance hardening para chat (eliminar micro-freezes)

- Se aplicó optimización de render en frontend para evitar bloqueos breves al responder el chat:
  - Se eliminaron suscripciones globales (useControlPlaneStore()) en vistas/paneles críticos y se reemplazaron por selectores granulares por campo.
  - Archivos optimizados: ControlPlanePage, Header, ServerDashboard, ConversationalChatPanel, RecentCommitsTable, TicketCoverageWidget, ServerConfigPanel, MaintenanceOverlay, DeveloperAccessPanel, ExportPanel, TeamManagementPanel, AdminOnboardingPanel, ApiKeyManagerWidget, SettingsPage, App.
  - Efecto esperado: updates de chatMessages/isChatLoading ya no fuerzan re-render completo del dashboard y widgets pesados.

- Se redujo costo de persistencia de chat (useControlPlaneStore.ts):
  - Persistencia desacoplada del mismo turno de render (escritura diferida con setTimeout(0)).
  - Límite de historial persistido: 80 mensajes.
  - Recorte de payload para storage en mensajes/respuestas extensas (tope ~4000 chars por campo) y data_refs truncado.
  - Limpieza de storage optimizada cuando el historial queda vacío.

- Ajuste de UX técnico:
  - scrollIntoView del chat pasó de smooth a uto para reducir costo de layout/paint en actualizaciones frecuentes.

- Validación ejecutada:
  - cd gitgov && npm run typecheck -> sin errores
  - cd gitgov && npx eslint <archivos tocados> -> 0 errores
- Ajuste de colaboración: se removió del AGENTS.md la sección obligatoria de formato de respuesta "Modo Auditor" para evitar respuestas rígidas y mejorar UX en iteraciones de producto.
## 2026-03-03 - Chat stability + seguridad + consultas determinísticas por usuario

- Se corrigió una causa estructural del freeze de UI en chat desktop:
  - gitgov/src-tauri/src/commands/server_commands.rs:
    - cmd_server_chat_ask migrado a comando async con 	auri::async_runtime::spawn_blocking.
    - cmd_server_create_feature_request también migrado a spawn_blocking para evitar bloqueo del hilo UI.
  - Resultado esperado: durante llamadas de red/LLM, la ventana ya no debería entrar en estado "No responde" por bloqueo directo del comando síncrono.

- Se reforzó seguridad del chatbot para evitar exposición de secretos:
  - gitgov/gitgov-server/src/handlers.rs:
    - Nueva detección is_secret_exfiltration_request(...) para rechazar solicitudes de extracción de API keys/tokens.
    - Sanitización central de respuestas sanitize_chat_answer_text(...) para redacción de patrones sensibles (UUID/tokens Bearer) antes de persistir/devolver.
    - Prompt del sistema endurecido: prohibición explícita de revelar/reconstruir secretos.

- Se corrigieron respuestas incoherentes de follow-up y se ampliaron consultas determinísticas por usuario:
  - Nuevas rutas de consulta en handlers.rs:
    - UserAccessProfile (rol/estado + existencia de key activa, sin exponer valores)
    - UserBlockedPushesMonth
    - UserPushesNoTicketWeek
    - UserScopeClarification para evitar asumir incorrectamente "commits" ante follow-ups ambiguos tipo "y del usuario X?".
  - Se añadieron heurísticas de continuidad conversacional usando last_user_login para preguntas de perfil/blocked/sin-ticket sin repetir usuario.

- Se corrigió mismatch entre UI (alias canónico) y consultas chat en DB:
  - gitgov/gitgov-server/src/db.rs:
    - chat_query_user_commits_count y chat_query_user_commits_range ahora son alias-aware vía identity_aliases.
    - Nuevos métodos alias-aware:
      - chat_query_user_blocked_pushes_month
      - chat_query_user_pushes_no_ticket_week
      - chat_query_user_access_profile
  - Resultado esperado: preguntas sobre usuarios visibles en dashboard/logs canónicos (ej. MapfrePE) ahora resuelven mejor en chat.

- Validación ejecutada:
  - cd gitgov/gitgov-server && cargo test -> 70 passed; 0 failed
  - cd gitgov/src-tauri && cargo check -> sin errores
  - cd gitgov && npm run typecheck -> sin errores
  - cd gitgov && npx eslint <archivos frontend tocados> -> 0 errores
## 2026-03-03 - Benchmark de capacidad del chat (p50/p95/p99 + throughput)

- Se añadió benchmark reproducible específico para chat (no solo webhooks/jobs):
  - Nuevo script: `gitgov/gitgov-server/tests/chat_capacity_test.py`
  - Métricas: distribución HTTP/status del chat, p50/p95/p99, throughput (RPS), errores de red/timeouts.
  - Escenarios incluidos: `deterministic`, `mixed`, `llm_forced`.
  - Soporta `--out-json` para artefactos en CI o comparación histórica.

- Se añadió target de Makefile:
  - `make chat-bench` con variables:
    - `SERVER_URL` (default `http://127.0.0.1:3000`)
    - `API_KEY`
    - `CHAT_REQUESTS`
    - `CHAT_CONCURRENCY`
    - `CHAT_SCENARIO`

- Corridas ejecutadas (local):
  1. `python tests/chat_capacity_test.py --server-url http://127.0.0.1:3000 --requests 60 --concurrency 6 --scenario deterministic --out-json tests/artifacts/chat_capacity_deterministic.json`
     - throughput: **3.09 rps**
     - latency: **p50 488 ms / p95 5757 ms / p99 6484 ms**
     - HTTP: `200=60/60`
  2. `python tests/chat_capacity_test.py --server-url http://127.0.0.1:3000 --requests 60 --concurrency 6 --scenario mixed --out-json tests/artifacts/chat_capacity_mixed.json`
     - throughput: **2.35 rps**
     - latency: **p50 2682 ms / p95 5856 ms / p99 6182 ms**
     - HTTP: `200=59/60`, `network_error=1`
  3. `python tests/chat_capacity_test.py --server-url http://127.0.0.1:3000 --requests 30 --concurrency 4 --scenario llm_forced --out-json tests/artifacts/chat_capacity_llm_forced.json`
     - throughput: **0.76 rps**
     - latency: **p50 5827 ms / p95 6744 ms / p99 6870 ms**
     - HTTP: `200=29/30`, `network_error=1`

- Artefactos generados:
  - `gitgov/gitgov-server/tests/artifacts/chat_capacity_deterministic.json`
  - `gitgov/gitgov-server/tests/artifacts/chat_capacity_mixed.json`
  - `gitgov/gitgov-server/tests/artifacts/chat_capacity_llm_forced.json`

## 2026-03-03 - Control de capacidad activo para /chat/ask

- Se implementó control de capacidad en backend para chat (además del benchmark):
  - `gitgov/gitgov-server/src/main.rs`:
    - Nuevo rate limit dedicado para chat: `GITGOV_RATE_LIMIT_CHAT_PER_MIN` (default 40/min), aplicado a `POST /chat/ask`.
    - Nuevas variables de runtime para LLM:
      - `GITGOV_CHAT_LLM_MAX_CONCURRENCY` (default 4)
      - `GITGOV_CHAT_LLM_QUEUE_TIMEOUT_MS` (default 500 ms)
      - `GITGOV_CHAT_LLM_TIMEOUT_MS` (default 9000 ms)
  - `gitgov/gitgov-server/src/handlers.rs`:
    - Cola/concurrencia con semáforo (`chat_llm_semaphore`) para evitar saturación de llamadas simultáneas al proveedor LLM.
    - Timeout de cola: si no se obtiene slot a tiempo, responde rápido con estado ocupado (429) en lugar de bloquear.
    - Timeout de llamada LLM: si excede el umbral, devuelve fallback/timeout controlado (504) manteniendo contexto conversacional.

- Objetivo del cambio:
  - Evitar acumulación de requests lentos en chat.
  - Proteger estabilidad para múltiples usuarios/teams y reducir la sensación de freeze.
  - Hacer degradación controlada bajo carga (busy/timeout) en vez de bloqueo.

- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> 70 passed; 0 failed
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-03 - Refactor estructural de handlers.rs a layout multiarchivo

- Se particionó `gitgov/gitgov-server/src/handlers.rs` en un layout mantenible de 16 archivos bajo `gitgov/gitgov-server/src/handlers/`, manteniendo el módulo público `handlers` intacto para no romper imports externos.
- Estrategia aplicada:
  - `handlers.rs` quedó como orquestador mínimo con `include!(...)` en orden estable.
  - El contenido existente se movió en bloques funcionales a:
    - `prelude_health.rs`
    - `integrations.rs`
    - `compliance_signals.rs`
    - `violations_policy_export.rs`
    - `github_webhook.rs`
    - `client_ingest_dashboard.rs`
    - `policy_admin.rs`
    - `org_core.rs`
    - `org_users_api_keys.rs`
    - `audit_stream_governance.rs`
    - `jobs_merges_admin_audit.rs`
    - `gdpr_clients_identities_scope.rs`
    - `conversational_runtime.rs`
    - `chat_handler.rs`
    - `feature_requests.rs`
    - `tests.rs`
- Objetivo:
  - Reducir riesgo de mantenimiento en un archivo monolítico.
  - Mejorar navegabilidad por dominio sin cambiar contratos ni rutas HTTP.

- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> 70 passed; 0 failed
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-03 - Split adicional de conversational_runtime.rs (3 archivos)

- Se particionó `gitgov/gitgov-server/src/handlers/conversational_runtime.rs` en 3 unidades para mejorar mantenibilidad:
  - `gitgov/gitgov-server/src/handlers/conversational/core.rs`
  - `gitgov/gitgov-server/src/handlers/conversational/query.rs`
  - `gitgov/gitgov-server/src/handlers/conversational/engine.rs`
- `conversational_runtime.rs` quedó como orquestador con `include!(...)`, manteniendo comportamiento y API interna sin cambios.

- Validación ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> 70 passed; 0 failed
  - `cd gitgov && npm run typecheck` -> sin errores

## 2026-03-03 - Hardening integral de chatbot (coherencia + seguridad + scope)

- Se reforzó la lógica de intención y extracción de usuario para reducir respuestas incoherentes:
  - `gitgov/gitgov-server/src/handlers/conversational/query.rs`
    - `extract_user_login(...)` ahora soporta más variantes reales (sin marcador explícito de "usuario") con filtros anti-falsos positivos.
    - Mejora de clasificación para frases en inglés tipo `how many commits did <user> ...`.
    - Se evitó capturar tokens ambiguos de contexto (`esta`, `this`, `sesion`, etc.) como si fueran login.

- Se endureció seguridad del chat frente a exfiltración de secretos:
  - `query.rs`:
    - `is_secret_exfiltration_request(...)` ahora cubre más patrones (`key`, `password`, `jwt`, `hash`, `credenciales`).
    - `sanitize_chat_answer_text(...)` amplió redacción para UUID, Bearer, JWT, tokens tipo `ghp_...`, `sk-...` y pares `key=value`.
  - `chat_handler.rs`:
    - Normalización de respuestas LLM (`normalize_llm_response`) para forzar estado válido, evitar respuestas genéricas vacías y mantener política de no exposición.
    - Sanitización de pregunta antes de persistir historial runtime y antes de enviarla al LLM.

- Se mejoró coherencia por organización (multiorg) y explicación cuando faltan datos:
  - `chat_handler.rs`:
    - Para keys globales sin org seleccionada, consultas analíticas ahora exigen scope de org explícito (evita ambigüedad entre organizaciones).
    - En consultas por usuario con resultado cero, se distingue mejor entre “sin actividad” vs “usuario fuera del scope activo”.
    - Mensajes de `insufficient_data` más explícitos sobre qué falta (usuario/org/rango).

- Se mejoró robustez del cliente Desktop/Tauri:
  - `gitgov/src-tauri/src/control_plane/server.rs`:
    - `chat_ask(...)` ahora intenta parsear `ChatAskResponse` incluso cuando el backend devuelve HTTP != 2xx (evita perder mensajes útiles en 429/504/500).
  - `gitgov/src/store/useControlPlaneStore.ts` + `gitgov/src/components/control_plane/ConversationalChatPanel.tsx`:
    - `chatAsk` ahora envía por defecto `org_name` usando la org seleccionada en UI (`selectedOrgName`) para evitar consultas sin scope en multiorg.

- Capacidad/runtime:
  - `gitgov/gitgov-server/src/handlers/conversational/core.rs`:
    - Se añadió pruning del runtime conversacional in-memory por TTL de inactividad y máximo de sesiones para evitar crecimiento sin límite.

- Corrección de KB interna:
  - `core.rs` actualizó la referencia de revocación de API key a endpoint real `POST /api-keys/{id}/revoke`.

- Tests y validación:
  - Nuevos tests de regresión en `gitgov/gitgov-server/src/handlers/tests.rs` para:
    - detección de usuario sin marcador explícito,
    - detección en phrasing inglés `did <user>`,
    - detección/sanitización ampliada de secretos.
  - Comandos ejecutados:
    - `cd gitgov/gitgov-server && cargo test` -> 72 passed; 0 failed
    - `cd gitgov/src-tauri && cargo check` -> sin errores
    - `cd gitgov && npm run typecheck` -> sin errores
    - `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ConversationalChatPanel.tsx` -> 0 errores

## 2026-03-03 - Chat UX y observabilidad de historial (ventana de logs + anti-freeze)

- Se corrigio la inconsistencia del historial en Dashboard (tabla de commits) aumentando la ventana operativa de logs para que no quede en muestras demasiado cortas:
  - `gitgov/src/store/useControlPlaneStore.ts`
    - `loadLogs` default paso a `limit=500`.
    - `refreshForCurrentRole` ahora usa `refreshDashboardData({ logLimit: 500 })` para admin.
    - Vista developer ahora carga `loadLogs(500, 0)`.
  - `gitgov/src/components/control_plane/ServerDashboard.tsx`
    - Nuevo `DASHBOARD_LOG_LIMIT = 500`.
    - Auto-refresh/refresh manual usan 500.
    - Se pausa el refresh periodico mientras `isChatLoading` para reducir micro-freezes durante respuesta del chatbot.
  - `gitgov/src/components/control_plane/TicketCoverageWidget.tsx`
    - Tras correlacion de Jira, se recarga logs con 500 para mantener coherencia de tabla.

- Claridad UX ya aplicada en tabla:
  - `gitgov/src/components/control_plane/RecentCommitsTable.tsx` muestra explicito que es "ventana reciente (hasta 500 eventos)" y pagination "en esta ventana".

- Validacion ejecutada:
  - `cd gitgov/gitgov-server && cargo test` -> `73 passed; 0 failed`
  - `cd gitgov/src-tauri && cargo check` -> OK
  - `cd gitgov && npm run typecheck` -> OK
  - `cd gitgov && npx eslint src/store/useControlPlaneStore.ts src/components/control_plane/ServerDashboard.tsx src/components/control_plane/TicketCoverageWidget.tsx src/components/control_plane/RecentCommitsTable.tsx src/components/control_plane/ConversationalChatPanel.tsx` -> `0 errores`

- Stress test real de chat (capacidad):
  - Comando:
    - `python tests/chat_capacity_test.py --requests 60 --concurrency 8 --scenario mixed --server-url http://127.0.0.1:3000 --out-json tests/artifacts/chat_capacity_latest.json`
  - Resultado:
    - throughput: `2.80 rps`
    - latencia: `p50 2669.9 ms`, `p95 6094.8 ms`, `p99 6711.8 ms`, `max 7394.3 ms`
    - HTTP: `200 = 60/60`
    - status chat: `ok=39`, `insufficient_data=17`, `error=4`

- Verificacion live de Golden Path contractual:
  - `POST /events` con Bearer -> `accepted=1, duplicates=0, errors=0`
  - `GET /stats` con Bearer -> respuesta valida con `client_events` y `github_events`
  - `GET /logs?limit=5&offset=0` con Bearer -> `5` eventos

## 2026-03-03 - Hotfix LLM JSON roto sin crash + degradacion sin badge ERROR

- Se atendio el problema reportado en logs (`Failed to parse LLM JSON: EOF...`) y los panics historicos por cortes inseguros de string UTF-8:
  - El parser de respuesta de LLM ya opera con recorte seguro por caracteres en `conversational/engine.rs` (`safe_prefix`) para evitar `byte index is not a char boundary`.
  - `chat_handler.rs` ahora degrada respuestas de fallo de LLM a respuesta util `status=ok` usando contexto local (`llm_degraded_answer(...)`) en vez de devolver `status=error` al usuario final.
  - `normalize_llm_response(...)` convierte respuestas `status=error` emitidas por el modelo a `insufficient_data` con mensaje accionable.

- Validacion de carga tras reinicio real del backend local (sin binario stale):
  - `python tests/chat_capacity_test.py --requests 20 --concurrency 4 --scenario llm_forced --server-url http://127.0.0.1:3000 --out-json tests/artifacts/chat_capacity_llm_forced_after_restart.json`
  - Resultado: `HTTP 200=20/20`, `chat status ok=20` (sin `error`), `p95=7981.4ms`, `p99=8777.1ms`.

- Validacion funcional adicional de historial:
  - `GET /logs?limit=50` devuelve ventana corta (ej. solo 5 commits en ventana).
  - `GET /logs?limit=500&event_type=commit&user_login=MapfrePE` devuelve 53 commits (47 feb + 6 mar), confirmando que no hay perdida de historial; era ventana de visualizacion.

## 2026-03-03 - Simplificacion de Paso 2 (Device obligatorio + login CP normal)

- Se simplifico la UX de autenticacion de Control Plane en desktop:
  - `gitgov/src/components/auth/ControlPlaneAuthScreen.tsx`
    - Se eliminó el flujo confuso de doble verificación (`Verificar identidad (/me)` + `Validar identidad y continuar`).
    - Ahora el Paso 2 usa un único flujo y un único botón:
      - `Usuario GitHub`
      - `URL Control Plane`
      - `API key GitGov`
      - `org_name` (solo cuando aplique por scope Admin Org)
      - Botón único `Entrar al Control Plane`
    - Se mantiene `Device Flow` como paso obligatorio previo (GitHub).
  - `gitgov/src/components/layout/MainLayout.tsx`
    - Se ajustó el gate para evitar estados inconsistentes del paso intermedio anterior.
    - Para `Admin` scopeado por org (`userOrgId != null`), ahora se exige `selectedOrgName` para continuar (evita ambigüedad multiorg).

- Validacion ejecutada:
  - `cd gitgov && npm run typecheck` -> OK
  - `cd gitgov && npx eslint src/components/auth/ControlPlaneAuthScreen.tsx src/components/layout/MainLayout.tsx` -> 0 errores

## 2026-03-03 - Hardening de inicio de sesión (Device siempre al arrancar)

- Se forzó el comportamiento de producto para desktop:
  - `gitgov/src/store/useAuthStore.ts`
    - `checkExistingSession()` ahora arranca en `authStep=idle` por defecto y exige pasar por GitHub Device Flow en cada reinicio de la app.
    - Se agregó flag `VITE_REQUIRE_DEVICE_FLOW_ON_START` (default `true`); solo si se define explícitamente `false` vuelve el comportamiento legacy de reutilizar sesión.

- Ajuste de compatibilidad de identidad CP:
  - `gitgov/src/store/useControlPlaneStore.ts`
    - `bootstrap-admin` ya no bloquea por falta de `VITE_FOUNDER_GITHUB_LOGIN`; si esa variable no existe, permite login founder con la key.
    - Para `Developer` sí se mantiene validación estricta `client_id == github_login`.
    - Para `Admin/Architect/PM` se permite identidad no 1:1 (caso org-admin/service users).

- Validación ejecutada:
  - `cd gitgov && npm run typecheck` -> OK
  - `cd gitgov && npx eslint src/store/useAuthStore.ts src/store/useControlPlaneStore.ts src/components/auth/ControlPlaneAuthScreen.tsx src/components/layout/MainLayout.tsx` -> 0 errores

## 2026-03-03 - Flujo login/logout corregido + limpieza de UI no deseada

- Se corrigió el flujo para que **Paso 2 sea exclusivamente posterior a Device Flow** y no se salte por estado viejo:
  - `gitgov/src/components/layout/MainLayout.tsx`
    - El gate de `ControlPlaneAuthScreen` ya no depende de que exista `serverConfig`; exige revalidación CP cuando no hay conexión/rol.
    - Cuando no hay sesión GitHub autenticada, se limpia estado de Control Plane para evitar arrastre entre usuarios.

- Se eliminó la acción no solicitada `Forzar/Usar Founder/Admin (.env)`:
  - `gitgov/src/pages/SettingsPage.tsx`
  - `gitgov/src/components/control_plane/ServerConfigPanel.tsx`

- Se reforzó cierre/cambio de usuario para limpiar también sesión de Control Plane:
  - `gitgov/src/pages/SettingsPage.tsx`
  - `gitgov/src/components/layout/Sidebar.tsx`
  - `gitgov/src/components/auth/PinUnlockScreen.tsx`
  - `gitgov/src/components/auth/ControlPlaneAuthScreen.tsx`

- Corrección de precisión temporal en chat (desfase de 1 segundo):
  - `gitgov/gitgov-server/src/db.rs`
    - `event_ts` en queries de commits cambió a `(EXTRACT(EPOCH FROM c.created_at) * 1000)::bigint` para conservar milisegundos y evitar redondeo previo.

- Validación ejecutada:
  - `cd gitgov && npm run typecheck` -> OK
  - `cd gitgov && npx eslint src/components/layout/MainLayout.tsx src/pages/SettingsPage.tsx src/components/control_plane/ServerConfigPanel.tsx src/components/layout/Sidebar.tsx src/components/auth/ControlPlaneAuthScreen.tsx src/components/auth/PinUnlockScreen.tsx src/store/useAuthStore.ts src/store/useControlPlaneStore.ts` -> 0 errores
  - `cd gitgov/gitgov-server && cargo test --quiet` -> `76 passed; 0 failed`

## 2026-03-03 - UX Device Flow: feedback visual de conexión garantizado

- Se ajustó el flujo de login GitHub para evitar percepción de salto instantáneo:
  - `gitgov/src/store/useAuthStore.ts`
    - `pollAuth()` ahora asegura una ventana visual mínima en estado `polling` (`MIN_POLLING_VISUAL_MS = 900`) incluso si GitHub responde muy rápido.
  - `gitgov/src/components/auth/LoginScreen.tsx`
    - Mensaje de `polling` actualizado a `Conectando con GitHub...` + `Validando autorización del Device Flow`.

- Objetivo:
  - Mantener confirmación visual explícita de que la app sí está validando contra GitHub antes de pasar a Paso 2.

- Validación ejecutada:
  - `cd gitgov && npm run typecheck` -> OK
  - `cd gitgov && npx eslint src/store/useAuthStore.ts src/components/auth/LoginScreen.tsx` -> 0 errores
