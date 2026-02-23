# GitGov - Arquitectura del Sistema

## Visión General

GitGov es un sistema de gobernanza de Git distribuido con tres componentes principales:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ARQUITECTURA GITGOV                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐         ┌─────────────────┐         ┌───────────────┐ │
│  │   DESKTOP APP   │         │ CONTROL PLANE   │         │    GITHUB     │ │
│  │    (Tauri)      │         │    SERVER       │         │               │ │
│  │                 │         │    (Axum)       │         │               │ │
│  │  ┌───────────┐  │         │  ┌───────────┐  │         │  ┌─────────┐  │ │
│  │  │  React UI │  │         │  │  Handlers │  │         │  │   API   │  │ │
│  │  │  Zustand  │  │  HTTP   │  │  Auth     │  │ Webhook │  │  OAuth  │  │ │
│  │  │  Tailwind │  │◄───────►│  │  DB       │  │◄───────►│  │  Repos  │  │ │
│  │  └───────────┘  │         │  └───────────┘  │         │  └─────────┘  │ │
│  │  ┌───────────┐  │         │  ┌───────────┐  │         │               │ │
│  │  │  Rust     │  │         │  │ PostgreSQL│  │         │               │ │
│  │  │  git2     │  │         │  │ Supabase  │  │         │               │ │
│  │  │  Outbox   │  │         │  │ Jobs      │  │         │               │ │
│  │  │  SQLite   │  │         │  └───────────┘  │         │               │ │
│  │  └───────────┘  │         │                 │         │               │ │
│  └─────────────────┘         └─────────────────┘         └───────────────┘ │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Componentes

### 1. Desktop App (Tauri)

**Tecnologías:**
- Frontend: React 18 + TypeScript + Tailwind CSS
- Backend: Rust + Tauri v2
- Estado: Zustand
- Git: git2 (libgit2 bindings)
- Auth: keyring (OS credential store)

**Responsabilidades:**

| Componente | Responsabilidad |
|------------|----------------|
| `git_commands.rs` | Operaciones Git (push, commit, stage, diff) |
| `outbox.rs` | Cola offline de eventos con reintentos |
| `auth_commands.rs` | GitHub OAuth Device Flow |
| `branch_commands.rs` | Creación y validación de ramas |
| `audit/db.rs` | SQLite local para auditoría offline |

**Flujo de datos:**

```
Usuario → React UI → Tauri Command → Rust Logic → Git Operations
                                        ↓
                                   Outbox Event
                                        ↓
                              JSONL + Background Flush
                                        ↓
                              POST /events → Server
```

### 2. Control Plane Server (Axum)

**Tecnologías:**
- Framework: Axum (Tokio)
- Database: sqlx + PostgreSQL (Supabase)
- Auth: SHA256 API key hashing
- Jobs: Background worker con backoff

**Responsabilidades:**

| Componente | Responsabilidad |
|------------|----------------|
| `handlers.rs` | HTTP endpoints y lógica de negocio |
| `auth.rs` | Middleware de autenticación |
| `db.rs` | Acceso a base de datos |
| `models.rs` | Estructuras de datos |
| `main.rs` | Configuración del servidor y jobs |

**Endpoints principales:**

| Endpoint | Método | Auth | Propósito |
|----------|--------|------|-----------|
| `/health` | GET | None | Health check |
| `/events` | POST | Bearer | Ingesta de eventos del cliente |
| `/webhooks/github` | POST | HMAC | Webhooks de GitHub |
| `/stats` | GET | Bearer | Estadísticas del dashboard |
| `/logs` | GET | Bearer | Eventos combinados |
| `/dashboard` | GET | Bearer | Datos del dashboard |

### 3. GitHub Integration

**Mecanismos:**

| Mecanismo | Uso |
|-----------|-----|
| OAuth Device Flow | Login de usuarios desde desktop |
| Webhooks | Recepción de eventos push/create |
| API REST | Operaciones en repos |

---

## Modelo de Datos

### Tablas Principales

```sql
-- Eventos de GitHub (webhooks)
CREATE TABLE github_events (
    id UUID PRIMARY KEY,
    org_id UUID,
    repo_id UUID,
    event_type TEXT,           -- push, create
    actor_login TEXT,
    ref_name TEXT,
    after_sha TEXT,
    commits_count INT,
    delivery_id TEXT UNIQUE,   -- Deduplicación
    created_at TIMESTAMPTZ
);

-- Eventos del cliente (desktop)
CREATE TABLE client_events (
    id UUID PRIMARY KEY,
    org_id UUID,
    repo_id UUID,
    event_uuid TEXT UNIQUE,    -- Deduplicación
    event_type TEXT,           -- attempt_push, successful_push, etc.
    user_login TEXT,
    branch TEXT,
    commit_sha TEXT,
    status TEXT,               -- success, blocked, failed
    reason TEXT,
    created_at TIMESTAMPTZ
);

-- API Keys
CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    key_hash TEXT UNIQUE,      -- SHA256 del API key
    client_id TEXT,
    role TEXT,                 -- admin, developer
    org_id UUID,
    is_active BOOLEAN,
    last_used TIMESTAMPTZ
);
```

### Relaciones

```
orgs ──┬── repos ────┬── github_events
       │             └── client_events
       │
       └── members ──── api_keys
```

---

## Flujo de Eventos

### Secuencia: Push desde Desktop

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│ Usuario  │     │ Desktop  │     │ Outbox   │     │ Server   │
└────┬─────┘     └────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │                │
     │ git push       │                │                │
     │───────────────►│                │                │
     │                │                │                │
     │                │ attempt_push   │                │
     │                │───────────────►│                │
     │                │                │                │
     │                │ push_to_remote │                │
     │                │────────────────┼───────────────►│
     │                │                │                │
     │                │ successful_push│                │
     │                │───────────────►│                │
     │                │                │                │
     │                │ trigger_flush  │                │
     │                │───────────────►│                │
     │                │                │                │
     │                │                │ POST /events   │
     │                │                │───────────────►│
     │                │                │                │
     │                │                │                │ INSERT
     │                │                │                │ client_events
     │                │                │                │
     │                │                │ 200 OK         │
     │                │                │◄───────────────│
     │                │                │                │
     │ OK             │                │                │
     │◄───────────────│                │                │
```

### Tipos de Eventos

| Evento | Origen | Cuándo | Campos |
|--------|--------|--------|--------|
| `attempt_push` | Desktop | Antes de cada push | user, branch |
| `successful_push` | Desktop | Push exitoso | user, branch, commit_sha |
| `blocked_push` | Desktop | Push a rama protegida | user, branch, reason |
| `push_failed` | Desktop | Error en push | user, branch, error |
| `commit` | Desktop | Commit creado | user, branch, files |
| `stage_files` | Desktop | Archivos staged | user, files |
| `push` | GitHub | Webhook push | actor, ref, commits |

---

## Autenticación

### Desktop → Control Plane

```
┌─────────────────────────────────────────────────────────────┐
│                   API KEY AUTHENTICATION                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Desktop                                                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ API Key: 57f1ed59-371d-46ef-9fdf-508f59bc4963       │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Header: Authorization: Bearer 57f1ed59-...          │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
└─────────────────────────┼───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                      SERVER                                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 1. Extract token from Authorization header           │   │
│  │    token = "57f1ed59-371d-46ef-9fdf-508f59bc4963"   │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 2. Calculate SHA256 hash                             │   │
│  │    hash = sha256(token)                             │   │
│  │    = "a1b2c3d4..."                                  │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 3. Query database                                    │   │
│  │    SELECT * FROM api_keys                            │   │
│  │    WHERE key_hash = 'a1b2c3d4...'                   │   │
│  │    AND is_active = true                             │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 4. If found → Authenticated                         │   │
│  │    Return: client_id, role, org_id                  │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Desktop → GitHub OAuth

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│ Desktop  │     │ GitHub   │     │ Usuario  │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     │ POST /device/code              │
     │───────────────►│                │
     │                │                │
     │ device_code    │                │
     │ user_code      │                │
     │◄───────────────│                │
     │                │                │
     │ Muestra código │                │
     │────────────────┼───────────────►│
     │                │                │
     │                │   Usuario va a │
     │                │   github.com/  │
     │                │   login/device │
     │                │                │
     │                │ Ingresa código │
     │                │◄───────────────│
     │                │                │
     │                │ Autoriza       │
     │                │───────────────►│
     │                │                │
     │ POST /oauth/access_token        │
     │───────────────►│                │
     │                │                │
     │ access_token   │                │
     │◄───────────────│                │
     │                │                │
     │ Guarda en      │                │
     │ keyring        │                │
     │                │                │
```

---

## Outbox Pattern

### Arquitectura del Outbox

```
┌─────────────────────────────────────────────────────────────┐
│                      OUTBOX ARCHITECTURE                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │   Event     │    │   Memory    │    │   Disk      │     │
│  │   Source    │───►│   Queue     │───►│   JSONL     │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
│                                               │             │
│                                               │ persist()   │
│                                               ▼             │
│                                        ~/.gitgov/           │
│                                        outbox.jsonl         │
│                                                             │
│  Background Worker (cada 60s)                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 1. Leer eventos pendientes (sent=false)              │   │
│  │ 2. Agrupar en batch                                  │   │
│  │ 3. POST /events con Authorization                    │   │
│  │ 4. Si éxito → marcar sent=true                       │   │
│  │ 5. Si error → incrementar attempts                   │   │
│  │ 6. Backoff exponencial: 30s * 2^attempts            │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Estructura del Evento

```rust
pub struct OutboxEvent {
    pub event_uuid: String,      // UUID único para dedup
    pub event_type: String,      // successful_push, commit, etc.
    pub user_login: String,
    pub user_name: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub files: Vec<String>,
    pub status: String,          // success, blocked, failed
    pub reason: Option<String>,
    pub repo_full_name: Option<String>,
    pub org_name: Option<String>,
    pub timestamp: i64,
    pub sent: bool,              // ¿Enviado al servidor?
    pub attempts: u32,           // Intentos de envío
    pub last_attempt: Option<i64>,
}
```

### Retry Logic

```
Intento 1: inmediato
Intento 2: +30 segundos
Intento 3: +60 segundos
Intento 4: +120 segundos
Intento 5: +240 segundos
...
Máximo: 5 intentos
```

---

## Seguridad

### Principios

1. **Tokens en keyring:** Nunca en archivos ni localStorage
2. **API keys hasheadas:** SHA256 antes de guardar en DB
3. **HTTPS obligatorio:** En producción
4. **Append-only:** Eventos no se pueden modificar
5. **Deduplicación:** event_uuid único previene duplicados

### Headers de Autenticación

| Tipo | Header | Uso |
|------|--------|-----|
| API Key | `Authorization: Bearer {key}` | Desktop → Server |
| HMAC | `X-Hub-Signature-256: sha256={sig}` | GitHub → Server |

### Validación de Webhooks

```rust
pub fn verify_github_signature(
    payload: &[u8],
    signature: &str,
    secret: &str,
) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let computed = format!("sha256={}", hex::encode(result.into_bytes()));
    constant_time_eq(computed.as_bytes(), signature.as_bytes())
}
```

---

## Observabilidad

### Logs Estructurados

```rust
// Niveles
tracing::error!("Critical error: {}", err);
tracing::warn!("Outbox flush failed: status {}", status);
tracing::info!("Token saved successfully login={}", login);
tracing::debug!("Headers: {:?}", headers);

// Con campos
tracing::info!(
    server_url = %url,
    has_api_key = key.is_some(),
    "GitGov Server configured from environment"
);
```

### Métricas Disponibles

| Métrica | Endpoint | Descripción |
|---------|----------|-------------|
| Job Queue | `/jobs/metrics` | pending, running, dead |
| Stats | `/stats` | Eventos por tipo |
| Health | `/health/detailed` | DB latency, uptime |

### SQL Queries de Debugging

```sql
-- Eventos recientes
SELECT * FROM client_events 
ORDER BY created_at DESC LIMIT 20;

-- Jobs atascados
SELECT * FROM jobs 
WHERE status = 'running' 
AND locked_at < NOW() - INTERVAL '10 minutes';

-- API keys activas
SELECT client_id, role, last_used 
FROM api_keys 
WHERE is_active = true;
```

---

## Extensibilidad

### Agregar Nuevo Tipo de Evento

1. **Desktop:** Agregar en `outbox.rs`:
```rust
pub fn from_audit_action(action: &AuditAction) -> String {
    match action {
        // ...
        AuditAction::NewAction => "new_action",
    }
}
```

2. **Server:** Agregar en `models.rs`:
```rust
pub enum ClientEventType {
    // ...
    NewAction,
}
```

3. **SQL:** Actualizar `get_combined_events()` si es necesario.

### Agregar Nuevo Endpoint

1. **Handler:** Agregar función en `handlers.rs`
2. **Route:** Agregar en `main.rs`
3. **Auth:** Decidir si requiere admin o solo auth
4. **Test:** Agregar caso en `e2e_flow_test.sh`
