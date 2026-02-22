# GitGov - Registro de Progreso

## Bug Fixes Críticos (2026-02-22 - Control Plane Sync)

### Resumen

Se resolvieron múltiples errores de serialización y autenticación que impedían la comunicación correcta entre el cliente Tauri y el servidor backend.

| Issue | Estado | Descripción |
|-------|--------|-------------|
| Panic en get_stats() | ✅ | NULL en `by_type`/`by_status` causaba crash |
| Serialización ServerStats | ✅ | Estructura anidada vs plana incompatible |
| Serialización CombinedEvent | ✅ | Cliente esperaba `AuditLogEntry`, servidor enviaba `CombinedEvent` |
| Outbox 401 Unauthorized | ✅ | Header `X-API-Key` vs `Authorization: Bearer` |

---

### Bug 1 - Panic en `get_stats()` ✅

**Problema:**
```
thread 'tokio-runtime-worker' panicked at src\db.rs:502:77:
called `Result::unwrap()` on an `Err` value: ColumnDecode { 
    index: "\"stats\"", 
    source: Error("invalid type: null, expected a map", line: 1, column: 82) 
}
```

**Causa raíz:**
- PostgreSQL `json_object_agg()` devuelve `NULL` cuando no hay filas
- Rust esperaba `HashMap<String, i64>` pero recibía `null`
- Los structs `GitHubEventStats` y `ClientEventStats` no tenían `#[serde(default)]`

**Solución aplicada:**

1. **Rust (models.rs):** Agregado `#[serde(default)]` a campos HashMap:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubEventStats {
    pub total: i64,
    pub today: i64,
    pub pushes_today: i64,
    #[serde(default)]  // <-- Agregado
    pub by_type: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientEventStats {
    pub total: i64,
    pub today: i64,
    pub blocked_today: i64,
    #[serde(default)]  // <-- Agregado
    pub by_type: HashMap<String, i64>,
    #[serde(default)]  // <-- Agregado
    pub by_status: HashMap<String, i64>,
}
```

2. **SQL (supabase_schema.sql):** Agregado `COALESCE` para devolver `{}` en lugar de NULL:
```sql
'by_type', COALESCE(
    (SELECT json_object_agg(event_type, cnt) FROM (...) t), 
    '{}'::json
),
'by_status', COALESCE(
    (SELECT json_object_agg(status, cnt) FROM (...) t), 
    '{}'::json
),
```

**SQL a ejecutar en Supabase:**
```sql
CREATE OR REPLACE FUNCTION get_audit_stats(p_org_id UUID DEFAULT NULL)
RETURNS JSON AS $$
DECLARE
    result JSON;
BEGIN
    SELECT json_build_object(
        'github_events', (
            SELECT json_build_object(
                'total', (SELECT COUNT(*) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id)),
                'today', (SELECT COUNT(*) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= DATE_TRUNC('day', NOW())),
                'pushes_today', (SELECT COUNT(*) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND event_type = 'push' AND created_at >= DATE_TRUNC('day', NOW())),
                'by_type', COALESCE((SELECT json_object_agg(event_type, cnt) FROM (SELECT event_type, COUNT(*) as cnt FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) GROUP BY event_type) t), '{}'::json)
            )
        ),
        'client_events', (
            SELECT json_build_object(
                'total', (SELECT COUNT(*) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id)),
                'today', (SELECT COUNT(*) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= DATE_TRUNC('day', NOW())),
                'blocked_today', (SELECT COUNT(*) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND status = 'blocked' AND created_at >= DATE_TRUNC('day', NOW())),
                'by_type', COALESCE((SELECT json_object_agg(event_type, cnt) FROM (SELECT event_type, COUNT(*) as cnt FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) GROUP BY event_type) t), '{}'::json),
                'by_status', COALESCE((SELECT json_object_agg(status, cnt) FROM (SELECT status, COUNT(*) as cnt FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) GROUP BY status) t), '{}'::json)
            )
        ),
        'violations', (
            SELECT json_build_object(
                'total', (SELECT COUNT(*) FROM violations WHERE (p_org_id IS NULL OR org_id = p_org_id)),
                'unresolved', (SELECT COUNT(*) FROM violations WHERE (p_org_id IS NULL OR org_id = p_org_id) AND NOT resolved),
                'critical', (SELECT COUNT(*) FROM violations WHERE (p_org_id IS NULL OR org_id = p_org_id) AND severity = 'critical' AND NOT resolved)
            )
        ),
        'active_devs_week', (SELECT COUNT(DISTINCT user_login) FROM client_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= NOW() - INTERVAL '7 days'),
        'active_repos', (SELECT COUNT(DISTINCT repo_id) FROM github_events WHERE (p_org_id IS NULL OR org_id = p_org_id) AND created_at >= NOW() - INTERVAL '7 days')
    ) INTO result;
    RETURN result;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
```

---

### Bug 2 - Serialización ServerStats ✅

**Problema:**
```
Serialization error: error decoding response body
```

**Causa raíz:**
El cliente Tauri esperaba estructura plana:
```rust
struct ServerStats {
    pushes_today: i64,
    blocked_today: i64,
    total_events: i64,
    events_by_repo: HashMap<String, i64>,
    // ...
}
```

Pero el servidor enviaba estructura anidada:
```rust
struct AuditStats {
    github_events: GitHubEventStats,
    client_events: ClientEventStats,
    violations: ViolationStats,
    // ...
}
```

**Solución aplicada:**

Sincronizadas las estructuras en:

| Archivo | Cambio |
|---------|--------|
| `src-tauri/src/control_plane/server.rs` | `ServerStats` actualizado para coincidir con `AuditStats` del servidor |
| `src/store/useControlPlaneStore.ts` | TypeScript interface actualizada |
| `src/components/control_plane/ServerDashboard.tsx` | Frontend actualizado para usar campos anidados |

**Nueva estructura unificada:**
```rust
pub struct ServerStats {
    pub github_events: GitHubEventStats,
    pub client_events: ClientEventStats,
    pub violations: ViolationStats,
    pub active_devs_week: i64,
    pub active_repos: i64,
}
```

---

### Bug 3 - Serialización CombinedEvent ✅

**Problema:**
```
Serialization error: error decoding response body
```
(En endpoint `/logs`)

**Causa raíz:**
El cliente esperaba `{ logs: Vec<AuditLogEntry> }` pero el servidor enviaba `{ events: Vec<CombinedEvent> }`.

**Solución aplicada:**

| Archivo | Cambio |
|---------|--------|
| `src-tauri/src/control_plane/server.rs` | Agregado struct `CombinedEvent` |
| `src-tauri/src/commands/server_commands.rs` | `cmd_server_get_logs` retorna `Vec<CombinedEvent>` |
| `src/lib/types.ts` | Agregado interface `CombinedEvent` |
| `src/store/useControlPlaneStore.ts` | `serverLogs` usa `CombinedEvent[]` |
| `src/components/control_plane/ServerDashboard.tsx` | Actualizado para usar campos de `CombinedEvent` |

**Estructura unificada:**
```rust
pub struct CombinedEvent {
    pub id: String,
    pub source: String,        // "github" o "client"
    pub event_type: String,
    pub created_at: i64,
    pub user_login: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub details: serde_json::Value,
}
```

---

### Bug 4 - Outbox 401 Unauthorized ✅

**Problema:**
```
WARN Outbox flush failed: status 401 Unauthorized
```

**Causa raíz:**
Inconsistencia en el header de autorización:
- `send_batch()` usaba: `X-API-Key: {api_key}`
- `start_background_flush()` usaba: `X-API-Key: {api_key}`
- Servidor esperaba: `Authorization: Bearer {api_key}`

**Flujo de autenticación del servidor:**
```rust
// auth.rs
let token = auth_header.strip_prefix("Bearer ")?;
let key_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));
db.validate_api_key(&key_hash).await
```

**Solución aplicada:**

| Archivo | Línea | Cambio |
|---------|-------|--------|
| `src-tauri/src/outbox/outbox.rs` | 439 | `X-API-Key` → `Authorization: Bearer` |
| `src-tauri/src/outbox/outbox.rs` | 550 | `X-API-Key` → `Authorization: Bearer` |

**Antes:**
```rust
request = request.header("X-API-Key", key);
```

**Después:**
```rust
request = request.header("Authorization", format!("Bearer {}", key));
```

---

### Archivos Modificados

| Archivo | Tipo | Descripción |
|---------|------|-------------|
| `gitgov-server/src/models.rs` | MODIFICADO | `#[serde(default)]` en HashMaps |
| `gitgov-server/supabase_schema.sql` | MODIFICADO | `COALESCE` en `get_audit_stats()` |
| `src-tauri/src/control_plane/server.rs` | MODIFICADO | `ServerStats`, `CombinedEvent`, auth header |
| `src-tauri/src/commands/server_commands.rs` | MODIFICADO | Import `CombinedEvent` |
| `src-tauri/src/outbox/outbox.rs` | MODIFICADO | Authorization header unificado |
| `src/store/useControlPlaneStore.ts` | MODIFICADO | Interfaces TypeScript |
| `src/lib/types.ts` | MODIFICADO | Agregado `CombinedEvent` |
| `src/components/control_plane/ServerDashboard.tsx` | MODIFICADO | UI para nueva estructura |

---

### Build Status

```
✅ Desktop (Tauri): cargo build - 10 warnings
✅ Server (Axum): cargo build - 22 warnings  
```

---

### Lecciones Aprendidas

1. **SQL NULL handling:** Siempre usar `COALESCE` con `json_object_agg()` cuando las tablas pueden estar vacías
2. **Rust serde:** Usar `#[serde(default)]` en campos opcionales dentro de structs con `Default`
3. **API contracts:** Mantener structs sincronizados entre servidor y cliente
4. **Auth headers:** Unificar formato de headers en todo el código

---

## Production Hardening (2026-02-21 - Parte 5 - FINAL)

### Resumen de Hardening Completo

Se implementaron todas las correcciones de production-grade identificadas en la auditoría:

| Issue | Estado | Descripción |
|-------|--------|-------------|
| Job Queue Hardening | ✅ | Atomic claim, dedupe, backoff, dead-letter |
| Cursor Incremental | ✅ | `ingested_at` en vez de `created_at` |
| Append-Only Triggers | ✅ | Verificados en todas las tablas de auditoría |
| Job Metrics | ✅ | Endpoint `/jobs/metrics` para observabilidad |
| Bootstrap Security | ✅ | Flag `--print-bootstrap-key` + TTY check |
| Stress Tests | ✅ | Suite de pruebas de carga |

---

### Prioridad 1 - Job Queue Production-Grade ✅

**Problemas resueltos:**
1. Race condition en claim → `FOR UPDATE SKIP LOCKED`
2. Explosión de jobs → Dedupe con índice único parcial
3. Reintentos infinitos → Backoff exponencial + dead-letter
4. Reset peligroso → Safe reset con FOR UPDATE

**Archivos modificados:**

| Archivo | Cambios |
|---------|---------|
| `gitgov-server/src/db.rs` | Job queue hardening: backoff, dead-letter, metrics |
| `gitgov-server/src/main.rs` | Worker mejorado con structured logging |
| `gitgov-server/src/handlers.rs` | Endpoints `/jobs/metrics`, `/jobs/dead`, `/jobs/:id/retry` |
| `gitgov-server/supabase_schema_v2.sql` | NUEVO - Migration con `ingested_at`, job hardening |

**Características del Job Queue:**

| Feature | Implementación |
|---------|---------------|
| Atomic Claim | `FOR UPDATE SKIP LOCKED` en subquery |
| Dedupe | `UNIQUE INDEX WHERE status IN ('pending', 'running')` |
| Backoff | `30s * 2^attempts`, cap 1 hora |
| Dead-Letter | `status='dead'` después de 10 intentos |
| Metrics | `get_job_metrics()` function |
| Stale Reset | TTL 5 min, safe con SKIP LOCKED |

**Nuevos endpoints:**

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/jobs/metrics` | GET | Métricas del queue (pending, running, dead, etc.) |
| `/jobs/dead` | GET | Lista jobs en dead-letter |
| `/jobs/:id/retry` | POST | Reintenta job muerto |

---

### Prioridad 2 - Cursor Incremental Seguro ✅

**Problema identificado:**
- `created_at` refleja tiempo del evento en GitHub
- Eventos pueden llegar tarde (retries, backlogs)
- Cursor en `created_at` salta eventos tardíos

**Solución:**
- Nuevo campo `ingested_at TIMESTAMPTZ DEFAULT NOW()`
- Cursor usa `(ingested_at, id)` en vez de `(created_at, id)`
- Nunca se modifica después del INSERT

**Schema añadido:**
```sql
ALTER TABLE github_events ADD COLUMN ingested_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE client_events ADD COLUMN ingested_at TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE org_processing_state ADD COLUMN last_ingested_at TIMESTAMPTZ;
```

**Función actualizada:**
```sql
detect_noncompliance_signals() -- Ahora usa ingested_at cursor
```

---

### Prioridad 3 - Append-Only y Violations ✅

**Clarificación importante sobre `violations`:**

La tabla `violations` NO es 100% append-only. Tiene **UPDATE limitado**:

```sql
CREATE TRIGGER violations_limited_update
    BEFORE UPDATE ON violations
    FOR EACH ROW EXECUTE FUNCTION violations_limited_update();
```

**Campos inmutables** (NO se pueden cambiar):
- `id`, `org_id`, `repo_id`, `github_event_id`, `client_event_id`
- `violation_type`, `severity`, `user_login`, `branch`, `commit_sha`
- `details`, `created_at`

**Campos mutables** (workflow de investigación):
- `resolved`, `resolved_at`, `resolved_by`

**Justificación:** Es más simple que crear `violation_decisions` separado.

**Tablas con triggers append-only:**

| Tabla | Trigger | Tipo |
|-------|---------|------|
| `github_events` | BEFORE UPDATE OR DELETE | 100% append-only |
| `client_events` | BEFORE UPDATE OR DELETE | 100% append-only |
| `violations` | Limited UPDATE | Solo campos de resolución |
| `noncompliance_signals` | BEFORE UPDATE OR DELETE | 100% append-only |
| `governance_events` | BEFORE UPDATE OR DELETE | 100% append-only |
| `signal_decisions` | BEFORE UPDATE OR DELETE | 100% append-only |

**Jobs table (NO append-only):**
- Restricción de UPDATE: solo columnas de estado pueden cambiar
- Restricción de DELETE: solo jobs completed/failed/dead
- Columnas inmutables: `id`, `org_id`, `job_type`, `created_at`, `payload`

---

### Prioridad 4 - Idempotencia de Jobs ✅

**Pregunta: ¿Los trabajos son idempotentes si se ejecutan dos veces?**

**Respuesta: PARCIALMENTE.**

1. **Señales**: NO se duplican
   ```sql
   AND NOT EXISTS (
       SELECT 1 FROM noncompliance_signals ns
       WHERE ns.github_event_id = ge.id
   )
   ```

2. **Cursor**: Puede avanzar pero los eventos ya procesados no se reprocesan

3. **Riesgo**: Si el trabajo falla DESPUÉS de crear señales pero ANTES de actualizar cursor, esas señales quedarán procesadas.

**Solución implementada:** Cursor y señales se actualizan en la MISMA transacción dentro de la función PostgreSQL.

---

### Prioridad 5 - Clave de Deduplicación ✅

**Pregunta: ¿Qué es la clave de deduplicación y es importante la carga útil?**

```sql
CREATE UNIQUE INDEX idx_jobs_unique_pending 
    ON jobs(org_id, job_type) 
    WHERE status IN ('pending', 'running');
```

**Respuesta:**
- Clave: `(org_id, job_type)`
- Payload NO importa (intencional)
- Justificación: `detect_signals` escanea TODOS los eventos de una org, no por repo

---

### Prioridad 6 - Atomicidad del Claim ✅

**Pregunta: ¿El claim es atómico en una sola transacción?**

**Respuesta: SÍ.** Una sola sentencia SQL:

```sql
UPDATE jobs SET status = 'running', ...
WHERE id = (
    SELECT id FROM jobs 
    WHERE status = 'pending' AND next_run_at <= NOW()
    ORDER BY priority DESC, created_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
RETURNING ...
```

No hay ventana para race condition.

---

### Prioridad 7 - Endpoints de Jobs Protegidos ✅

**Todos los endpoints de jobs requieren ADMIN:**

```rust
pub async fn get_job_metrics(...) {
    if let Err(_) = require_admin(&auth_user) {
        return (StatusCode::FORBIDDEN, ...);
    }
}
```

Métricas son globales (no filtradas por org), pero NO exponen:
- org_id en lista de dead jobs
- payload completo
- errores con detalles sensibles

---

### Prioridad 8 - Seguridad Bootstrap ✅

**Problema identificado:** `eprintln!()` va a logs en Docker/Kubernetes.

**Solución implementada:**
1. Flag CLI explícita (`--print-bootstrap-key`)
2. Detección de TTY con `atty` crate
3. Key solo se imprime si: flag presente O TTY adjunto

**Archivos modificados:**
- `gitgov-server/Cargo.toml`: Agregados `clap` y `atty`
- `gitgov-server/src/main.rs`: CLI args, check TTY

**Uso:**
```bash
# Interactive (TTY) - key printed to console
cargo run

# Docker (no TTY) - key NOT printed to logs
docker run gitgov-server

# Explicit flag - key always printed
docker run gitgov-server --print-bootstrap-key
```

---

### Prioridad 5 - Stress Tests ✅

**Archivo creado:** `gitgov-server/tests/stress_test.sh`

**Tests incluidos:**
1. Webhook idempotency - mismo delivery_id rechazado
2. Job dedupe - 100 webhooks → 1 job pending/running
3. Stale reset - recovery automático después de TTL
4. Multi-org - múltiples orgs, un job por org
5. High volume - 500 webhooks en paralelo
6. Job metrics - endpoint verificando métricas

**Ejecutar tests:**
```bash
cd gitgov-server/tests
chmod +x stress_test.sh
SERVER_URL=http://localhost:3000 API_KEY=xxx ./stress_test.sh
```

---

### Prioridad 6 - Documentación Actualizada ✅

**README actualizado con:**
- Arquitectura del Job Queue
- Estados y transiciones
- Configuración de TTL, backoff, max_attempts
- SQL queries para debugging
- Troubleshooting (stuck jobs, dead jobs, high pending)
- Monitoreo con `/jobs/metrics`

---

## Build Status Final

```
✅ Desktop (Tauri): cargo build - 10 warnings
✅ Server (Axum): cargo build - 22 warnings  
✅ Clippy: cargo clippy - 35 warnings (style only, no errors)
```

---

## Archivos Modificados/Creados Esta Sesión

| Archivo | Tipo | Descripción |
|---------|------|-------------|
| `gitgov-server/supabase_schema_v2.sql` | NUEVO | Migration con ingested_at, job hardening |
| `gitgov-server/supabase_schema_v3.sql` | NUEVO | Violation decisions, true append-only violations |
| `gitgov-server/src/db.rs` | MODIFICADO | Job queue hardening, violation_decisions methods |
| `gitgov-server/src/main.rs` | MODIFICADO | Worker mejorado, CLI args, bootstrap security, nuevas rutas |
| `gitgov-server/src/handlers.rs` | MODIFICADO | Job management, violation decisions endpoints |
| `gitgov-server/Cargo.toml` | MODIFICADO | Agregados clap, atty crates |
| `gitgov-server/README.md` | MODIFICADO | Documentación job queue, bootstrap security |
| `gitgov-server/tests/stress_test.sh` | NUEVO | Suite de stress tests |
| `gitgov/gitgov.toml` | MODIFICADO | Completado con checklist, rules, drift_detection, audit, severity |

---

## Comandos para Aplicar en Producción

```bash
# 1. Aplicar migrations (en orden)
psql -f gitgov-server/supabase_schema.sql      # Base schema
psql -f gitgov-server/supabase_schema_v2.sql   # Job hardening
psql -f gitgov-server/supabase_schema_v3.sql   # Violation decisions

# 2. Build release
cd gitgov-server && cargo build --release

# 3. Ejecutar stress tests
cd tests && ./stress_test.sh

# 4. Monitorear jobs
curl -H "Authorization: Bearer $API_KEY" \
  http://localhost:3000/jobs/metrics
```

---

## SQL de Debugging

```sql
-- Ver todos los jobs de una org
SELECT id, job_type, status, attempts, last_error, created_at
FROM jobs WHERE org_id = 'xxx' ORDER BY created_at DESC;

-- Jobs stuck en running
SELECT * FROM jobs 
WHERE status = 'running' AND locked_at < NOW() - INTERVAL '10 minutes';

-- Dead letter queue
SELECT id, job_type, attempts, last_error 
FROM jobs WHERE status = 'dead';

-- Reset manual de stale jobs
SELECT reset_stale_jobs_safe(5);

-- Throughput diario
SELECT status, COUNT(*), AVG(duration_ms)
FROM jobs 
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY status;
```

---

## PENDIENTES PARA PRODUCCIÓN (TODO)

| Issue | Severidad | Descripción | Estado |
|-------|-----------|-------------|--------|
| TTL por job type | Media | TTL 5min fijo puede causar duplicados | Posponer - solo detect_signals es rápido |
| Heartbeat/lease renewal | Baja | Jobs largos pueden ser reseteados | Posponer - mismo motivo |
| Violations append-only | Media | Confusión entre docs y código | ✅ RESUELTO v3 |

### Prioridad 9 - Violation Decisions ✅

**Problema:** `violations` permitía UPDATE en campos `resolved/resolved_at/resolved_by`, generando confusión con "append-only".

**Solución implementada:**

1. **Nueva tabla `violation_decisions`** (append-only):
   - `decision_type`: acknowledged, false_positive, resolved, escalated, dismissed, wont_fix
   - `decided_by`, `decided_at`, `notes`, `evidence`
   - Unique constraint: `(violation_id, decision_type)`

2. **Trigger en `violations`**:
   - Bloquea UPDATE a `resolved/resolved_at/resolved_by`
   - Fuerza uso de `violation_decisions`

3. **Función `add_violation_decision()`**:
   - Inserta decisión en `violation_decisions`
   - Si `decision_type='resolved'`, actualiza campos legacy en `violations` (backwards compatibility)

4. **Vista `violation_current_status`**:
   - Muestra estado actual con última decisión
   - Incluye `is_closed` calculado

5. **Endpoints nuevos**:
   - `GET /violations/:id/decisions` - Historial de decisiones (auth)
   - `POST /violations/:id/decisions` - Agregar decisión (admin)

6. **Migración automática**:
   - Resoluciones existentes migradas a `violation_decisions`

**Archivos creados/modificados:**
- `gitgov-server/supabase_schema_v3.sql` - NUEVO
- `gitgov-server/src/db.rs` - Métodos `add_violation_decision`, `get_violation_decisions`
- `gitgov-server/src/handlers.rs` - Endpoints nuevos
- `gitgov-server/src/main.rs` - Rutas nuevas

---

### gitgov.toml Completado ✅

**Secciones agregadas:**
- `[checklist]` - Pre-push validaciones
- `[rules]` - Branch protection rules
- `[drift_detection]` - Verificación de protecciones
- `[audit]` - Retención y export
- `[severity]` - Niveles de severidad (bonus)

**Archivo:** `gitgov/gitgov.toml` (55 → 182 líneas)

---

## VERIFICACIÓN DE CALIDAD (Checklist)

### SQL Exacto Implementado

**Dedupe Index** (supabase_schema_v2.sql:442-444):
```sql
CREATE UNIQUE INDEX idx_jobs_unique_pending 
    ON jobs(org_id, job_type) 
    WHERE status IN ('pending', 'running');
```

**Claim Query** (db.rs:1238-1255):
```sql
UPDATE jobs SET status = 'running', locked_at = NOW(), locked_by = $1, attempts = attempts + 1, started_at = NOW()
WHERE id = (SELECT id FROM jobs WHERE status = 'pending' AND next_run_at <= NOW() ORDER BY priority DESC, created_at ASC LIMIT 1 FOR UPDATE SKIP LOCKED)
RETURNING ...
```

**Trigger Jobs UPDATE restriction** (supabase_schema_v2.sql:377-398):
```sql
CREATE OR REPLACE FUNCTION jobs_allowed_update() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.id != OLD.id OR NEW.org_id IS DISTINCT FROM OLD.org_id OR NEW.job_type != OLD.job_type OR NEW.created_at != OLD.created_at OR NEW.payload IS DISTINCT FROM OLD.payload THEN
        RAISE EXCEPTION 'Cannot modify immutable job columns';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
```

**Trigger Jobs DELETE restriction** (supabase_schema_v2.sql:411-414):
```sql
CREATE OR REPLACE FUNCTION jobs_delete_restriction() RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status NOT IN ('completed', 'failed', 'dead') THEN
        RAISE EXCEPTION 'Cannot delete pending or running jobs';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;
```

### Build Output

```
✅ cargo build: Finished dev [unoptimized + debuginfo] target(s) in 1.42s
   Warnings: 22 (unused variables, dead code)

✅ cargo clippy: Finished dev [unoptimized + debuginfo] target(s) in 57.35s
   Warnings: 35 (style only, no errors)
```

### Stress Tests Output

```bash
# Ejecutar con:
SERVER_URL=http://localhost:3000 API_KEY=xxx ./stress_test.sh

# Tests incluidos:
# 1. Webhook idempotency - mismo delivery_id rechazado
# 2. Job dedupe - 100 webhooks → 1 job pending/running
# 3. Stale reset - recovery automático después de TTL
# 4. Multi-org - múltiples orgs, un job por org
# 5. High volume - 500 webhooks en paralelo
# 6. Job metrics - endpoint verificando métricas
```

---

## Audit Stream Endpoint (2026-02-21 - Parte 4)

### Prioridad 4 - Audit Stream Endpoint ✅

**Archivos creados/modificados:**

| Archivo | Cambios |
|---------|---------|
| `gitgov-server/src/models.rs` | Agregados `GovernanceEvent`, `GitHubAuditLogEntry`, `AuditStreamBatch`, `AuditStreamResponse`, `RELEVANT_AUDIT_ACTIONS` |
| `gitgov-server/src/db.rs` | Agregados `insert_governance_event`, `insert_governance_events_batch`, `get_governance_events` |
| `gitgov-server/src/handlers.rs` | Agregados `ingest_audit_stream`, `get_governance_events` handlers |
| `gitgov-server/src/main.rs` | Agregados endpoints `POST /audit-stream/github` y `GET /governance-events` |

**Endpoints nuevos:**

| Endpoint | Método | Requiere | Descripción |
|----------|--------|----------|-------------|
| `/audit-stream/github` | POST | admin | Ingesta batch de audit logs de GitHub |
| `/governance-events` | GET | auth | Query governance events |

**Eventos auditados relevantes:**
- `protected_branch.*` - Branch protection changes
- `repository_ruleset.*` - Ruleset modifications
- `repo.access`, `repo.permissions_*` - Permission changes
- `team.*_repository` - Team access changes

**Formato de request:**
```json
{
  "org_name": "my-org",
  "entries": [
    {
      "@timestamp": 1739980800000,
      "action": "protected_branch.update",
      "actor": "admin-user",
      "repo": "my-org/my-repo",
      "data": {
        "old": { "required_approving_review_count": 1 },
        "new": { "required_approving_review_count": 2 }
      }
    }
  ]
}
```

---

## Correcciones Críticas (2026-02-21 - Parte 3)

### Prioridad 1 - Outbox Integrado en Commands ✅

**Archivos modificados:**

| Archivo | Cambios |
|---------|---------|
| `src-tauri/src/lib.rs` | Inicializa outbox, configura server_url/api_key, inicia background flush worker |
| `src-tauri/src/commands/git_commands.rs` | `cmd_push`, `cmd_commit`, `cmd_stage_files` ahora escriben eventos al outbox |
| `src-tauri/src/commands/branch_commands.rs` | `cmd_create_branch` ahora escribe eventos al outbox |
| `src-tauri/Cargo.toml` | Agregados crates `tracing` y `tracing-subscriber` |
| `src-tauri/src/models/audit_log.rs` | Agregado `Copy` a `AuditAction` y `AuditStatus` |

**Flujo implementado:**
1. Escribir evento al outbox ANTES de la operación (con `event_uuid`)
2. Ejecutar operación Git
3. Actualizar estado del evento según resultado
4. Trigger flush no bloqueante al servidor

**El outbox NO bloquea la operación principal.**

---

### Prioridad 2 - Middleware de Autenticación ✅

**Archivos creados/modificados:**

| Archivo | Cambios |
|---------|---------|
| `gitgov-server/src/auth.rs` | Nuevo módulo: middleware de autenticación, `require_admin`, `require_same_user_or_admin` |
| `gitgov-server/src/main.rs` | Configuración de rutas con auth middleware, separación de rutas públicas vs protegidas |
| `gitgov-server/src/handlers.rs` | Todos los handlers usan `Extension<AuthUser>`, role-based access control |

**Reglas de autorización implementadas:**

| Endpoint | Requiere | Restricción |
|----------|----------|-------------|
| `GET /logs` | auth | admin ve todo, dev solo sus propios eventos |
| `GET /dashboard` | auth + admin | solo admins |
| `GET /stats` | auth + admin | solo admins |
| `POST /events` | auth | cualquier usuario autenticado |
| `GET /policy/:repo` | auth | cualquier usuario del org |
| `PUT /policy/:repo/override` | auth + admin | SOLO admins (renombrado de save_policy) |
| `POST /api-keys` | auth + admin | solo admins |
| `POST /webhooks/github` | HMAC signature | sin JWT |

---

### Prioridad 3 - Correlación y Confidence Scoring ✅

**Actualizado en supabase_schema.sql:**

Tabla `violations` ahora incluye:
- `confidence_level` TEXT ('high', 'low', 'pending')
- `reason` TEXT ('direct_push_no_client_event', 'missing_telemetry_outbox_pending', etc.)
- `correlated_github_event_id` UUID
- `correlated_client_event_id` UUID
- `resolved_by` TEXT (alias de reviewed_by)
- `resolved_at` TIMESTAMPTZ (alias de reviewed_at)

**Lenguaje de señales (NO binario):**
- `confidence = 'high'` → "Señal de noncompliance — ruta no autorizada detectada"
- `confidence = 'low'` → "Telemetría incompleta — outbox pendiente"
- Solo cuando `resolved_by` tiene valor → puede escalarse a "confirmed_bypass"

**NUNCA mostrar "BYPASS DETECTADO" como estado automático.**

---

### Prioridad 4 - Governance Events ✅

**Nueva tabla en supabase_schema.sql:**

```sql
CREATE TABLE governance_events (
    id UUID PRIMARY KEY,
    org_id UUID,
    repo_id UUID,
    delivery_id TEXT UNIQUE NOT NULL,
    event_type TEXT,          -- branch_protection_changed, ruleset_modified, permission_changed
    actor_login TEXT,
    target TEXT,              -- qué recurso fue afectado
    old_value JSONB,
    new_value JSONB,
    payload JSONB,
    created_at TIMESTAMPTZ
);
```

**Aplica trigger append-only.**

---

### Prioridad 5 - PUT /policy Corregido ✅

**Cambio:**
- Renombrado a `PUT /policy/:repo/override`
- Requiere admin
- Registra warning en logs con `is_override=true`
- La política "source of truth" está en `gitgov.toml` versionado en git

---

### Prioridad 6 - Correcciones de Documentación ✅

**Tailwind:** Verificado v4.2.0 instalado

**Hunk Staging:** MVP solo soporta stage por archivo completo. Hunk staging en V2.0.

**Outbox Status:** 
- ✅ Módulo creado
- ✅ Integrado en commands
- ✅ Background worker iniciado

---

## Build Status

**Desktop (Tauri):** ✅ Compila con warnings menores
**Server:** ✅ Compila con warnings menores

---

## Resumen de Archivos Modificados

### Desktop App (src-tauri/)

1. `src/lib.rs` - Outbox initialization y background worker
2. `src/commands/git_commands.rs` - Outbox integration en push, commit, stage
3. `src/commands/branch_commands.rs` - Outbox integration en create_branch
4. `src/models/audit_log.rs` - Added Copy trait
5. `Cargo.toml` - Added tracing dependencies

### Server (gitgov-server/)

1. `src/auth.rs` - NUEVO - Auth middleware
2. `src/main.rs` - Route configuration con auth + audit stream routes
3. `src/handlers.rs` - Auth user extraction, role checks, audit stream handlers
4. `src/models.rs` - GovernanceEvent, GitHubAuditLogEntry models
5. `src/db.rs` - Governance events CRUD
6. `supabase_schema.sql` - Added governance_events, updated violations

---

## Endpoints Completos

| Endpoint | Método | Auth | Descripción |
|----------|--------|------|-------------|
| `/health` | GET | public | Health check básico |
| `/health/detailed` | GET | public | Health check con DB status |
| `/webhooks/github` | POST | HMAC | GitHub webhook receiver |
| `/events` | POST | auth | Client telemetry batch |
| `/audit-stream/github` | POST | admin | GitHub audit log stream |
| `/governance-events` | GET | auth | Query governance events |
| `/logs` | GET | auth | Combined event log |
| `/stats` | GET | admin | Audit statistics |
| `/dashboard` | GET | admin | Dashboard data |
| `/compliance/:org` | GET | admin | Compliance dashboard |
| `/signals` | GET | auth | Noncompliance signals |
| `/signals/:id` | POST | auth | Update signal status |
| `/signals/detect/:org` | POST | admin | Trigger detection |
| `/policy/:repo` | GET | auth | Get repo policy |
| `/policy/:repo/override` | PUT | admin | Override policy |
| `/policy/:repo/history` | GET | auth | Policy change history |
| `/export` | POST | auth | Export events |
| `/api-keys` | POST | admin | Create API key |

---

## Para Probar

```bash
# Desktop
cd gitgov/src-tauri && cargo build

# Server
cd gitgov-server && cargo build

# Con Supabase configurado:
# 1. Ejecutar supabase_schema.sql en SQL Editor
# 2. Configurar .env
# 3. cargo run
```

---

## HMAC curl Example

```bash
# Generar signature
PAYLOAD='{"ref":"refs/heads/feat/123","pusher":{"name":"dev1"},"repository":{"full_name":"org/repo"},"commits":[],"after":"abc123"}'
SIGNATURE=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "YOUR_SECRET" | sed 's/SHA2-256(stdin)= /sha256=/')

curl -X POST http://localhost:3000/webhooks/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: test-delivery-$(date +%s)" \
  -H "X-Hub-Signature-256: $SIGNATURE" \
  -d "$PAYLOAD"
```
