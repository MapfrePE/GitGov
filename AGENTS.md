# Guía para Agentes de IA - GitGov

> Este archivo proporciona contexto esencial para agentes de IA. Léelo completo antes de hacer cambios.
> Nota de alcance: el checklist estricto está redactado principalmente para sesiones con Claude Code.

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

GitGov es un sistema de gobernanza de Git distribuido que audita y controla operaciones de Git en organizaciones.

**Tres componentes:**
1. **Desktop App (Tauri)** - Aplicación de escritorio para desarrolladores
2. **Control Plane Server (Axum)** - Servidor central que recopila eventos
3. **GitHub Integration** - OAuth + Webhooks

---

## Estado del Proyecto (2026-02-22)

### Funcional

| Componente | Estado |
|------------|--------|
| Desktop App | Inicia, muestra dashboard, commits |
| Control Plane Server | Corre en localhost:3000 |
| Autenticación | GitHub OAuth + API Keys |
| Outbox | Envía eventos con backoff |
| Dashboard | Muestra estadísticas y eventos |
| Pipeline E2E | Desktop → Server → PostgreSQL → Dashboard |    

### Golden Path (NO ROMPER)

Antes de tocar auth, outbox, dashboard o endpoints del server, asumir que este flujo es crítico y debe mantenerse:

1. Desktop detecta archivos cambiados
2. Usuario puede hacer commit desde la app
3. Usuario puede hacer push desde la app
4. Control Plane recibe eventos (`stage_files`, `commit`, `attempt_push`, `successful_push`)
5. Dashboard muestra logs/commits sin errores `401`

**Regla para agentes:** cualquier cambio en auth/token/API key/frontend dashboard/server handlers debe validar explícitamente este flujo (o dejar documentado por qué no pudo validarlo).

**Checklist operativa:** `docs/GOLDEN_PATH_CHECKLIST.md`

**Advertencia local (muy importante):** en desarrollo local usar **una sola URL canónica** para el Control Plane (`http://127.0.0.1:3000`). No mezclar `localhost` y `127.0.0.1` si Docker/WSL también están levantados, porque puedes terminar enviando eventos del Desktop a un server y viendo el Dashboard en otro (split-brain local).

**Regla operativa (anti split-brain):**
- `127.0.0.1:3000` = server local (`cargo run`) para Golden Path / demo principal
- Docker `gitgov-server` = **`127.0.0.1:3001`** por defecto (NO usar `3000`)
- Si se levanta Docker para pruebas, mantener el Desktop apuntando a `127.0.0.1:3000` salvo que se esté probando Docker explícitamente

### Pendiente

| Componente | Prioridad |
|------------|-----------|
| Webhooks GitHub | Alta |
| Correlation Engine | Alta |
| Drift Detection | Media |
| Tests automatizados | Media |

---

## Comandos Esenciales

**Desarrollo:**
- Desktop: `cd gitgov && npm run tauri dev`
- Server: `cd gitgov/gitgov-server && cargo run`
- Tests: `cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh`

**Linting (EJECUTAR ANTES DE COMMIT):**
- Server Rust: `cd gitgov/gitgov-server && cargo clippy -- -D warnings`
- Desktop Rust: `cd gitgov/src-tauri && cargo clippy -- -D warnings`
- Frontend TS: `cd gitgov && npm run lint && npm run typecheck`

> Si hay deuda histórica de lint en el repo, la regla de aceptación es: **0 errores nuevos en archivos tocados** (ejecutar ESLint sobre esos archivos), además de `npm run typecheck`.

---

## Arquitectura de Autenticación

### Desktop → Control Plane

1. Desktop lee API key de .env o configuración
2. Envía header: `Authorization: Bearer {api_key}`
3. Server calcula SHA256 del token
4. Busca en tabla api_keys por key_hash
5. Si encuentra → autenticado

**CRÍTICO:** El servidor SOLO acepta `Authorization: Bearer`, NO `X-API-Key`.

### Desktop → GitHub OAuth

1. Desktop llama a GitHub Device Flow
2. Usuario ingresa código en github.com/login/device
3. GitHub retorna token
4. Desktop guarda token en keyring (NUNCA en archivo)

---

## Flujo de Eventos

### Secuencia de un Push

```
Usuario hace push
    │
    ▼
cmd_push() en git_commands.rs
    │
    ├─► Crear OutboxEvent "attempt_push"
    │
    ├─► Validar rama protegida
    │       Si protegida → "blocked_push"
    │
    ├─► Ejecutar push_to_remote()
    │       Éxito → "successful_push"
    │       Error → "push_failed"
    │
    └─► trigger_flush()
            Background worker envía a /events
```

### Deduplicación

Los eventos se deduplican por `event_uuid` único. Si el servidor recibe el mismo UUID dos veces, el segundo se rechaza.

---

## Estructuras de Datos

### ServerStats (GET /stats)

Estructura anidada con tres secciones principales:

**github_events:** total, today, pushes_today, by_type (puede ser {} si no hay datos)

**client_events:** total, today, blocked_today, by_type, by_status (pueden ser {})

**violations:** total, unresolved, critical

**Plus:** active_devs_week, active_repos

### CombinedEvent (GET /logs)

Campos: id, source ("github" | "client"), event_type, created_at (timestamp ms), user_login?, repo_name?, branch?, status?, details

---

## Errores Comunes

### Panic: "invalid type: null, expected a map"

**Causa:** PostgreSQL json_object_agg() devuelve NULL cuando no hay filas.

**Solución:** Usar COALESCE en SQL y #[serde(default)] en Rust HashMaps.

### 401 Unauthorized

**Causa:** Header incorrecto.

**Incorrecto:** `X-API-Key: {key}`
**Correcto:** `Authorization: Bearer {key}`

### Serialization error

**Causa:** Structs no coinciden entre cliente y servidor.

**Solución:** ServerStats y CombinedEvent deben ser idénticos en ambos lados.

### Outbox no envía

**Verificar:** server_url en .env, api_key en .env, background worker iniciado, conexión de red.

---

## Archivos Críticos

| Archivo | Propósito | Precaución |
|---------|-----------|------------|
| outbox/outbox.rs | Cola de eventos offline | Auth headers, retry logic |
| commands/git_commands.rs | Operaciones Git | Event logging |
| auth.rs | Middleware auth | Token validation |
| handlers.rs | API handlers | Response structures |
| models.rs | Data structures | Serde attributes |
| supabase_schema.sql | DB schema | COALESCE in aggregates |

---

## Variables de Entorno

**Desktop (.env):**
- VITE_SERVER_URL=http://127.0.0.1:3000
- VITE_API_KEY=(tu api key)

**Server (.env):**
- DATABASE_URL=postgresql://...
- GITGOV_JWT_SECRET=...
- GITGOV_SERVER_ADDR=0.0.0.0:3000
- GITGOV_API_KEY=(tu api key)
- GITHUB_WEBHOOK_SECRET=...

---

## Convenciones de Código

**Rust:**
- Errores: thiserror
- Logging: tracing (info/debug/warn/error)
- Serde: #[serde(default)] en Option y HashMap
- Async: tokio runtime

**TypeScript:**
- Estado: Zustand stores en src/store/
- Tipos: Interfaces en src/lib/types.ts
- Componentes: Functional components con hooks
- Estilos: Tailwind classes

---

## Endpoints del Servidor

| Endpoint | Auth | Propósito |
|----------|------|-----------|
| /health | None | Health check |
| /events | Bearer | Ingesta de eventos del cliente |
| /webhooks/github | HMAC | Webhooks de GitHub |
| /stats | Bearer (admin) | Estadísticas |
| /logs | Bearer | Eventos combinados |
| /dashboard | Bearer (admin) | Datos del dashboard |
| /jobs/metrics | Bearer (admin) | Métricas del job queue |

---

## Tipos de Eventos

| Evento | Origen | Cuándo |
|--------|--------|--------|
| attempt_push | Desktop | Antes de cada push |
| successful_push | Desktop | Push exitoso |
| blocked_push | Desktop | Push a rama protegida |
| push_failed | Desktop | Error en push |
| commit | Desktop | Commit creado |
| stage_files | Desktop | Archivos staged |
| push | GitHub | Webhook push |

---

## Seguridad

1. Tokens en keyring - Nunca en archivos
2. API keys hasheadas - SHA256 antes de DB
3. HTTPS obligatorio en producción
4. Append-only - Eventos no se modifican
5. Deduplicación - event_uuid único
6. No exponer secretos en chat/logs/commits

---

## Reglas para Agentes (Obligatorias)

1. **Leer antes de modificar** — No proponer cambios a código que no hayas leído
2. **Golden Path primero** — Validar que el flujo base sigue funcionando
3. **Linting antes de commit** — `cargo clippy` + `npm run typecheck` + `0 errores nuevos` en ESLint de archivos tocados
4. **No romper structs compartidas** — `ServerStats`, `CombinedEvent` deben coincidir en frontend y backend
5. **Append-only** — No intentar UPDATE/DELETE en tablas de auditoría
6. **COALESCE siempre** — En cualquier SQL con agregaciones
7. **Bearer, no X-API-Key** — Para autenticación
8. **Documentar cambios** — Actualizar `docs/PROGRESS.md` con cambios significativos
9. **No inventar** — Si no se pudo verificar, responder `NO VERIFICADO` y listar exactamente qué falta para verificar
10. **Anti split-brain local** — Usar canónico `127.0.0.1:3000` para server local; Docker server en `127.0.0.1:3001`

---

## Modo Implementación — Checklist obligatorio

Antes de escribir cualquier línea de código:

**1. Archivos leídos (listar todos antes de empezar):**
- [ ] Archivo a modificar — leído con herramienta de lectura en esta sesión
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

Tras cualquier cambio en archivos críticos (auth/handlers/main/models/outbox/control-plane store), ejecutar:

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
# Esperar: JSON con ServerStats (no {"error":"..."})

# 4. Validar contrato /logs
curl "http://127.0.0.1:3000/logs?limit=5&offset=0" \
  -H "Authorization: Bearer {api_key}"
# Esperar: {"events":[...]} (sin error de deserialización)

# 5. Smoke contractual live (recomendado)
cd gitgov/gitgov-server && make smoke

# 6. E2E Golden Path (recomendado)
cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh
```

---

## Recursos

- Arquitectura: docs/ARCHITECTURE.md
- Troubleshooting: docs/TROUBLESHOOTING.md
- Progreso: docs/PROGRESS.md
- Inicio rápido: docs/QUICKSTART.md
