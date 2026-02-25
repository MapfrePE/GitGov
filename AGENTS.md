||||||||||as# Guía para Agentes de IA - GitGov

> Este archivo proporciona contexto esencial para agentes de IA. Léelo completo antes de hacer cambios.

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

---

## Recursos

- Arquitectura: docs/ARCHITECTURE.md
- Troubleshooting: docs/TROUBLESHOOTING.md
- Progreso: docs/PROGRESS.md
- Inicio rápido: docs/QUICKSTART.md
