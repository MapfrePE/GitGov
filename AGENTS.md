# Guía para Agentes de IA - GitGov

> Este archivo contiene instrucciones críticas para agentes de IA que trabajen en este proyecto. Léelo completo antes de hacer cambios.

---

## Estado Actual del Proyecto (2026-02-22)

### ✅ Funcional

| Componente | Estado | Notas |
|------------|--------|-------|
| Desktop App | ✅ | Inicia, muestra dashboard, commits |
| Control Plane Server | ✅ | Corre en localhost:3000 |
| Autenticación | ✅ | GitHub OAuth + API Keys |
| Outbox | ✅ | Envía eventos con backoff |
| Dashboard Control Plane | ✅ | Muestra estadísticas y eventos |
| Pipeline E2E | ✅ | Desktop → Server → PostgreSQL → Dashboard |

### ⚠️ Pendiente

| Componente | Prioridad | Descripción |
|------------|-----------|-------------|
| Webhooks GitHub | Alta | Recibir eventos de GitHub |
| Correlation Engine | Alta | Correlacionar client_events con github_events |
| Drift Detection | Media | Detectar desviaciones de política |
| Tests automatizados | Media | Expandir cobertura |

---

## Comandos de Build

```bash
# Desktop App (Tauri)
cd gitgov
npm install
npm run tauri dev      # Desarrollo
cargo build --manifest-path src-tauri/Cargo.toml  # Solo Rust

# Control Plane Server
cd gitgov/gitgov-server
cargo build
cargo run

# Tests
cd gitgov/gitgov-server/tests
./e2e_flow_test.sh
./stress_test.sh
```

---

## Linting y Typecheck

**SIEMPRE ejecutar antes de commit:**

```bash
# Backend Rust
cd gitgov/gitgov-server && cargo clippy -- -D warnings
cd gitgov/src-tauri && cargo clippy -- -D warnings

# Frontend TypeScript
cd gitgov && npm run lint
cd gitgov && npm run typecheck
```

---

## Arquitectura de Autenticación

### Desktop → GitHub OAuth

```
1. Usuario hace login
2. Desktop llama a GitHub Device Flow
3. Usuario ingresa código en github.com/login/device
4. GitHub retorna token
5. Desktop guarda token en keyring (NO en archivo)
```

### Desktop → Control Plane

```
1. Desktop lee API key de .env o config
2. Envía header: Authorization: Bearer {api_key}
3. Server calcula SHA256(api_key)
4. Server busca en tabla api_keys por key_hash
5. Si encuentra → autenticado
```

**⚠️ IMPORTANTE:** El servidor SOLO acepta `Authorization: Bearer`, NO `X-API-Key`.

---

## Flujo de Eventos

### Secuencia de un Push

```
Usuario hace push
    │
    ▼
cmd_push() [git_commands.rs:169]
    │
    ├─► Crear OutboxEvent "attempt_push"
    │       outbox.add(event) → JSONL
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

Los eventos se deduplican por `event_uuid`:

```sql
-- En client_events table
event_uuid TEXT UNIQUE NOT NULL
```

Si el servidor recibe el mismo `event_uuid` dos veces, el segundo se rechaza con error de duplicado.

---

## Estructuras de Datos Críticas

### ServerStats (GET /stats)

```typescript
interface ServerStats {
  github_events: {
    total: number
    today: number
    pushes_today: number
    by_type: Record<string, number>  // ⚠️ Puede ser {} si tabla vacía
  }
  client_events: {
    total: number
    today: number
    blocked_today: number
    by_type: Record<string, number>
    by_status: Record<string, number>
  }
  violations: {
    total: number
    unresolved: number
    critical: number
  }
  active_devs_week: number
  active_repos: number
}
```

### CombinedEvent (GET /logs)

```typescript
interface CombinedEvent {
  id: string
  source: 'github' | 'client'
  event_type: string
  created_at: number  // Unix timestamp ms
  user_login?: string
  repo_name?: string
  branch?: string
  status?: string
  details: Record<string, unknown>
}
```

---

## Errores Comunes y Soluciones

### 1. Panic: "invalid type: null, expected a map"

**Causa:** PostgreSQL `json_object_agg()` devuelve NULL cuando no hay filas.

**Solución:** Usar `COALESCE` en SQL:
```sql
COALESCE(json_object_agg(...), '{}'::json)
```

Y en Rust:
```rust
#[serde(default)]
pub by_type: HashMap<String, i64>,
```

### 2. 401 Unauthorized en /events

**Causa:** Header incorrecto.

**Mal:**
```rust
request.header("X-API-Key", key)
```

**Bien:**
```rust
request.header("Authorization", format!("Bearer {}", key))
```

### 3. Serialization error: decoding response body

**Causa:** Structs no coinciden entre cliente y servidor.

**Solución:** Verificar que `ServerStats` y `CombinedEvent` sean idénticos en ambos lados.

### 4. Outbox no envía eventos

**Verificar:**
1. `server_url` configurado en .env
2. `api_key` configurado
3. Background worker iniciado
4. Conexión de red

---

## Archivos Críticos

| Archivo | Propósito | Modificar con cuidado |
|---------|-----------|----------------------|
| `gitgov/src-tauri/src/outbox/outbox.rs` | Cola de eventos offline | Auth headers, retry logic |
| `gitgov/src-tauri/src/commands/git_commands.rs` | Operaciones Git | Event logging |
| `gitgov/gitgov-server/src/auth.rs` | Middleware auth | Token validation |
| `gitgov/gitgov-server/src/handlers.rs` | API handlers | Response structures |
| `gitgov/gitgov-server/src/models.rs` | Data structures | Serde attributes |
| `gitgov/gitgov-server/supabase_schema.sql` | DB schema | COALESCE in aggregates |

---

## Variables de Entorno

### Desktop (.env)

```env
VITE_SERVER_URL=http://localhost:3000
VITE_API_KEY=57f1ed59-371d-46ef-9fdf-508f59bc4963
```

### Server (.env)

```env
DATABASE_URL=postgresql://...
GITGOV_JWT_SECRET=...
GITGOV_SERVER_ADDR=0.0.0.0:3000
GITGOV_API_KEY=57f1ed59-371d-46ef-9fdf-508f59bc4963
GITHUB_WEBHOOK_SECRET=...
SUPABASE_URL=...
SUPABASE_ANON_KEY=...
```

---

## Convenciones de Código

### Rust

- **Errores:** Usar `thiserror` para custom errors
- **Logging:** Usar `tracing` con niveles info/debug/warn/error
- **Serde:** Siempre `#[serde(default)]` en Option y HashMap
- **Async:** Usar `tokio` runtime

### TypeScript

- **Estado:** Zustand stores en `src/store/`
- **Tipos:** Interfaces en `src/lib/types.ts`
- **Componentes:** Functional components con hooks
- **Estilos:** Tailwind classes, no CSS custom

---

## SQL para Debugging

```sql
-- Ver eventos del cliente
SELECT * FROM client_events ORDER BY created_at DESC LIMIT 20;

-- Ver estadísticas
SELECT * FROM get_audit_stats();

-- Ver eventos combinados
SELECT * FROM get_combined_events(100, 0);

-- Ver API keys activas
SELECT client_id, role, last_used FROM api_keys WHERE is_active = true;

-- Ver jobs pendientes
SELECT * FROM jobs WHERE status IN ('pending', 'running');
```

---

## Contacto y Recursos

- Documentación principal: `docs/PROGRESS.md`
- Plan maestro: `docs/GITGOV_PLAN_CLAUDE_CODE.md`
- Roadmap: `docs/GITGOV_ROADMAP_COMERCIAL_v2.md`
- Server README: `gitgov/gitgov-server/README.md`
