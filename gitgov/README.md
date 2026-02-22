# GitGov

**Git Governance Control Plane** - Aplicación de escritorio para control de flujos Git con roles, nomenclatura de ramas forzada, staging selectivo y auditoría centralizada.

## Posicionamiento

> "GitGov es la capa de gobernanza que convierte las reglas de tu equipo de ingeniería en código versionado, enforcement orquestado, y evidencia inmutable — sin reemplazar GitHub."

**GitGov NO es un cliente de Git.** Es un Control Plane de gobernanza que:
1. **Orquesta** los controles de GitHub (branch protection, rulesets)
2. **Genera evidencia** inmutable para auditorías
3. **Detecta** señales de noncompliance con confianza calibrada

## Trust Model

**Enforcement vive en el Git host, no en el desktop.**

El desktop de GitGov es un guardrail de UX: guía al developer, valida nomenclatura, hace staging selectivo, y registra intentos. Pero no puede garantizar compliance por sí solo — cualquier dev podría hacer `git push` desde la terminal.

El enforcement real se logra configurando branch protection y rulesets en GitHub, que GitGov orquesta y verifica a escala. GitGov detecta cuando un repo se desvía de la política (drift detection) y genera evidencia de cumplimiento o noncompliance.

**Lo que GitGov aporta que GitHub no empaqueta:**
- Correlación entre lo que el dev intentó (client_events) y lo que GitHub ejecutó (github_events via webhooks)
- Detección de rutas no autorizadas con confidence scoring (no acusaciones automáticas)
- Versiones de política trazables (gitgov.toml en git = gestión de cambios auditable)
- Retención de evidencia de gobernanza más allá de los 180 días del audit log de GitHub
- Un solo panel para orgs con múltiples repos sin policy consistente

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────┐
│                        DESKTOP APP (Tauri v2)                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   React UI  │  │  Zustand    │  │   Rust Backend          │ │
│  │  (TypeScript)│  │   Stores   │  │   - Git operations      │ │
│  │             │  │             │  │   - GitHub OAuth        │ │
│  │  Dashboard  │  │ useAuthStore│  │   - Local SQLite audit  │ │
│  │  FileList   │  │ useRepoStore│  │   - Config validation   │ │
│  │  DiffViewer │  │ useAuditStore│ │   - Outbox (offline)    │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ HTTP POST /events (batch)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    CONTROL PLANE SERVER (Rust)                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Axum API  │  │  Handlers   │  │   PostgreSQL (Supabase) │ │
│  │             │  │             │  │                         │ │
│  │ /events     │  │ Webhooks    │  │   - github_events       │ │
│  │ /webhooks   │  │ ClientEvents│  │   - client_events       │ │
│  │ /logs       │  │ Correlation │  │   - noncompliance_signals│ │
│  │ /compliance │  │ Detection   │  │   - violations          │ │
│  │ /export     │  │             │  │   - policy_history      │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ Webhooks (push, create)
                              │
┌─────────────────────────────────────────────────────────────────┐
│                         GITHUB                                  │
│   Repositories → Webhooks → GitGov Control Plane                │
│   Branch Protection + Rulesets = Real Enforcement               │
└─────────────────────────────────────────────────────────────────┘
```

## Event Contracts

### Client Events (desde Desktop)

| event_type | Descripción | Campos obligatorios adicionales |
|-----------|-------------|-------------------------------|
| `attempt_push` | Dev intentó hacer push desde la app | `branch`, `files[]` |
| `successful_push` | Push completado exitosamente | `branch`, `commit_sha`, `files[]` |
| `blocked_push` | Push bloqueado por regla de GitGov | `branch`, `reason` |
| `create_branch` | Rama creada desde la app | `branch`, `from_branch` |
| `blocked_branch` | Creación de rama bloqueada | `branch`, `reason` |
| `commit` | Commit creado | `branch`, `commit_sha`, `files[]` |
| `stage_files` | Archivos agregados al staging | `files[]` |

### Garantías del sistema

- **Entrega:** at-least-once (el outbox reintenta hasta 5 veces con backoff)
- **Deduplicación:** por `event_uuid` UNIQUE en servidor
- **Ordering:** no garantizado entre eventos distintos, garantizado por timestamp dentro del mismo usuario
- **Batch size máximo:** 100 eventos por request
- **Payload máximo por evento:** 64KB

### GitHub Events (webhooks)

| event_type | Descripción | Campos procesados |
|-----------|-------------|-------------------|
| `push` | Push a rama | `ref`, `after`, `commits[]`, `pusher`, `repository` |
| `create` | Creación de branch/tag | `ref`, `ref_type`, `sender`, `repository` |

## Control Plane Data Contracts

### ServerStats (GET /stats)

Estructura anidada devuelta por el servidor:

```typescript
interface ServerStats {
  github_events: {
    total: number
    today: number
    pushes_today: number
    by_type: Record<string, number>
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

Estructura unificada para eventos combinados:

```typescript
interface CombinedEvent {
  id: string
  source: 'github' | 'client'
  event_type: string
  created_at: number  // Unix timestamp (ms)
  user_login?: string
  repo_name?: string
  branch?: string
  status?: string
  details: Record<string, unknown>
}
```

### Authentication Flow

El cliente Tauri autentica con el servidor usando API keys:

1. **Header requerido:** `Authorization: Bearer {api_key}`
2. **Servidor:** Calcula `SHA256(api_key)` y busca en tabla `api_keys`
3. **Importante:** El servidor NO acepta `X-API-Key`, solo `Authorization: Bearer`

```bash
# ❌ WRONG - Returns 401
curl -H "X-API-Key: $API_KEY" http://localhost:3000/stats

# ✅ CORRECT
curl -H "Authorization: Bearer $API_KEY" http://localhost:3000/stats
```

## Features Principales

### ✅ V1.0 - Implementado

| Feature | Descripción |
|---------|-------------|
| **Source of Truth** | Webhooks de GitHub → `github_events` (append-only) |
| **Telemetría** | Desktop outbox → `client_events` (append-only) |
| **Correlación** | client_event ↔ github_event por commit_sha |
| **Bypass Detection** | Señales de noncompliance con confidence scoring |
| **Policy Versioning** | Historial automático de cambios de gitgov.toml |
| **Export** | PDF/Excel/JSON con hash SHA256 |
| **Offline Queue** | Outbox JSONL + backoff exponencial |

### Bypass Detection - NO Binario

El sistema usa **confidence scoring** en lugar de detección binaria:

| Confidence | Condición | Signal Type |
|------------|-----------|-------------|
| **High** | Push sin client_event, outbox vacío | `untrusted_path` |
| **Low** | Push sin client_event, outbox pendiente | `missing_telemetry` |

**Lenguaje orientado a evidencia (no acusaciones):**
- ✓ `noncompliance signal`
- ✓ `untrusted path detected`
- ✓ `missing telemetry`
- ✗ `bypass detected` (muy acusatorio)
- ✗ `violation` (requiere confirmación manual)

### Separación Source of Truth vs Telemetría

| Tabla | Origen | Uso | Append-only |
|-------|--------|-----|-------------|
| `github_events` | Webhooks | Source of truth | ✓ |
| `client_events` | Desktop | Telemetría/intentos | ✓ |
| `noncompliance_signals` | Server | Detección | ✓ |
| `violations` | Manual | Confirmadas | ✓ |

## Componentes

### Desktop App (`gitgov/`)

- **Tauri v2**: Framework de apps de escritorio (Rust + WebView)
- **React 18 + TypeScript**: Frontend con Vite
- **Tailwind v4**: Estilos
- **Zustand**: Estado global
- **git2 (Rust)**: Operaciones Git nativas
- **SQLite**: Auditoría local
- **Outbox**: Cola offline con JSONL

### Control Plane Server (`gitgov-server/`)

- **Axum**: Servidor HTTP en Rust
- **Supabase/PostgreSQL**: Base de datos centralizada
- **Append-only**: Auditoría inmutable
- **RLS**: Row Level Security para multi-tenant
- **Correlation Engine**: Motor de correlación de eventos

## Deployment Requirements

### HTTPS/TLS obligatorio

GitHub rechaza endpoints HTTP para webhooks en producción. Usar certificado válido (Let's Encrypt funciona). En desarrollo local usar ngrok o similar.

### Variables de entorno obligatorias

```env
# Obligatorias
DATABASE_URL=postgresql://...
GITGOV_JWT_SECRET=    # Mínimo 32 caracteres random
GITHUB_WEBHOOK_SECRET=
GITGOV_SERVER_ADDR=0.0.0.0:3000
```

### Observabilidad mínima

El servidor loguea a stdout en formato JSON estructurado:
- `timestamp`
- `level`
- `request_id`
- `endpoint`
- `status_code`
- `duration_ms`

Usar `tracing` con `tracing-subscriber` en formato JSON.

### Backups

- **Supabase managed**: point-in-time recovery automático
- **Self-host**: documentar `pg_dump` periódico

## Fallback de Enforcement

**Si el plan de GitHub no incluye push rulesets:**

Push rulesets están disponibles en repos privados/internos con planes de pago. Si el cliente no los tiene disponibles, GitGov sigue aportando valor mediante:

- Branch protection + PR requerido + status checks + CODEOWNERS
- Policy-as-code versionado con drift detection
- Correlación de evidencia y noncompliance signals
- El enforcement cambia de mecanismo, el valor de gobernanza no desaparece.

## Inicio Rápido

### Desktop App

```bash
cd gitgov
npm install
npm run tauri dev
```

### Control Plane Server

```bash
cd gitgov-server

# 1. Crear .env
cp .env.example .env
# Editar con credenciales de Supabase

# 2. Ejecutar schema en Supabase SQL Editor
# Ver: supabase_schema.sql

# 3. Correr servidor
cargo run
```

## Endpoints del Server

### Health (público)

| Método | Path | Descripción |
|--------|------|-------------|
| GET | `/health` | Health check simple |
| GET | `/health/detailed` | Detallado: latencia DB, uptime, eventos pendientes |

### Webhooks (HMAC auth)

| Método | Path | Descripción |
|--------|------|-------------|
| POST | `/webhooks/github` | GitHub webhook (push, create) |

### Authenticated Endpoints

| Método | Path | Auth | Descripción |
|--------|------|------|-------------|
| POST | `/events` | Bearer | Client events batch |
| GET | `/logs` | Bearer | Query events (dev: own only) |
| GET | `/stats` | Bearer + Admin | Statistics |
| GET | `/dashboard` | Bearer + Admin | Dashboard |
| GET | `/compliance/:org` | Bearer + Admin | Compliance dashboard |
| GET | `/signals` | Bearer | Noncompliance signals |
| POST | `/signals/:id` | Bearer | Update signal status |
| POST | `/signals/detect/:org` | Bearer + Admin | Trigger detection |
| GET | `/policy/:repo` | Bearer | Get policy |
| PUT | `/policy/:repo/override` | Bearer + Admin | Override policy (logged) |
| GET | `/policy/:repo/history` | Bearer | Policy change history |
| POST | `/export` | Bearer | Export events |
| POST | `/api-keys` | Bearer + Admin | Create API key |

## Configuración (`gitgov.toml`)

```toml
[branches]
patterns = ["feat/*", "fix/*", "hotfix/*"]
protected = ["main", "develop", "staging"]

[groups.frontend]
members = ["alice", "bob"]
allowed_branches = ["feat/frontend/*", "fix/*"]
allowed_paths = ["src/frontend/**", "public/**"]

[groups.backend]
members = ["charlie"]
allowed_branches = ["feat/backend/*", "fix/*"]
allowed_paths = ["src/backend/**", "api/**"]

admins = ["admin-user"]
```

## Tecnologías

| Componente | Stack |
|------------|-------|
| Desktop | Tauri v2, React 18, TypeScript, Tailwind v4, Zustand |
| Backend Desktop | Rust, git2, rusqlite, reqwest |
| Server | Rust, Axum, sqlx |
| Database | Supabase (PostgreSQL) |
| Auth | GitHub OAuth Device Flow |
| Git | libgit2 via git2-rs |

## Roadmap Comercial

Ver [GITGOV_ROADMAP_COMERCIAL_v2.md](./GITGOV_ROADMAP_COMERCIAL_v2.md) para detalles.

### V1.0 ✅
- ✅ Correlation engine
- ✅ Bypass detection con confidence scoring
- ✅ Policy versioning
- ✅ Export con hash
- ✅ Noncompliance signals

### V1.1
- [ ] Drift detection
- [ ] Checklist antes del push
- [ ] Integración Jira

### V2.0
- [ ] Multi-provider (GitLab, Bitbucket)
- [ ] Hunk staging

## Pitch Defensible

**La objeción más común:** "GitHub ya tiene branch protection, audit log y rulesets. ¿Para qué necesito GitGov?"

**La respuesta:**

> "GitHub hace el enforcement — lo hace bien. El problema es que con 50 repos y 5 equipos, nadie sabe si todos los repos tienen la policy correcta aplicada hoy. Nadie puede demostrar qué política estaba vigente el día que ocurrió un incidente. Y nadie puede correlacionar 'el dev intentó hacer algo' con 'lo que GitHub realmente ejecutó'. GitGov es el control plane que gestiona eso a escala y genera la evidencia que GitHub no empaqueta para auditorías."

## Documentación

- [Plan Maestro](./GITGOV_PLAN_CLAUDE_CODE.md)
- [Roadmap Comercial](./GITGOV_ROADMAP_COMERCIAL_v2.md)
- [Progreso](./PROGRESS.md)
- [Server README](./gitgov-server/README.md)

## Licencia

MIT
