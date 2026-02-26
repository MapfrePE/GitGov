# GitGov - Guía de Inicio Rápido

## Requisitos Previos

- Node.js 18+ (Desktop App y Web App)
- pnpm (Web App: `npm install -g pnpm`)
- Rust 1.70+
- PostgreSQL (o cuenta Supabase)
- GitHub Account

## Instalación (5 minutos)

### 1. Clonar y configurar Desktop

```bash
cd gitgov
npm install
```

Crear `.env`:
```env
VITE_SERVER_URL=http://localhost:3000
VITE_API_KEY=tu-api-key-aqui
```

### 2. Configurar Control Plane Server

```bash
cd gitgov-server
cp .env.example .env
```

Editar `.env`:
```env
DATABASE_URL=postgresql://user:pass@host:5432/db
GITGOV_JWT_SECRET=tu-secret-de-32-caracteres-minimo
GITGOV_SERVER_ADDR=0.0.0.0:3000
GITGOV_API_KEY=tu-api-key-aqui
GITHUB_WEBHOOK_SECRET=tu-webhook-secret
```

### 3. Inicializar Base de Datos

En Supabase SQL Editor, ejecutar los archivos **en orden**:
```sql
-- 1. Schema base
-- Ejecutar: gitgov-server/supabase_schema.sql

-- 2. Mejoras (aplicar solo si actualizas desde v1)
-- Ejecutar: supabase_schema_v2.sql
-- Ejecutar: supabase_schema_v3.sql
-- Ejecutar: supabase_schema_v4.sql  (append-only triggers)

-- 3. V1.2-A — Jenkins
-- Ejecutar: supabase_schema_v5.sql  (pipeline_events)

-- 4. V1.2-B — Jira
-- Ejecutar: supabase_schema_v6.sql  (project_tickets, commit_ticket_correlations)
```

Para una instalación limpia nueva: ejecutar todos los archivos del v1 al v6 en ese orden.

### 4. Ejecutar

**Terminal 1 - Server:**
```bash
cd gitgov/gitgov-server
cargo run
```

**Terminal 2 - Desktop:**
```bash
cd gitgov
npm run tauri dev
```

**Terminal 3 - Web App (opcional, solo para desarrollo del sitio público):**
```bash
cd gitgov-web
pnpm dev
# Abre en http://localhost:3001 (o el puerto que Next.js asigne)
```

## Verificación

### 1. Health Check
```bash
curl http://localhost:3000/health
# Esperado: OK
```

### 2. Stats
```bash
curl -H "Authorization: Bearer $API_KEY" http://localhost:3000/stats
# Esperado: JSON con github_events, client_events, violations
```

### 3. Desktop App
- Abrir la aplicación
- Iniciar sesión con GitHub
- Seleccionar un repositorio
- Hacer un commit/push
- Verificar que aparece en el Dashboard

## Estructura del Proyecto

```
GitGov/
├── gitgov/                        # Desktop App (Tauri v2)
│   ├── src/                       # Frontend React 19
│   │   ├── components/            # Componentes UI
│   │   │   ├── control_plane/     # Dashboard & widgets
│   │   │   └── git/               # Vista de git
│   │   ├── store/                 # Estado Zustand v5
│   │   └── lib/                   # Utilidades y tipos
│   ├── src-tauri/                 # Backend Rust
│   │   └── src/
│   │       ├── commands/          # Tauri commands (git_commands, server_commands)
│   │       ├── git/               # Operaciones Git
│   │       ├── outbox/            # Cola offline JSONL
│   │       ├── audit/             # SQLite local
│   │       ├── control_plane/     # HTTP client al servidor
│   │       └── lib.rs             # App init, env vars
│   └── gitgov.toml                # Config del repo
│
├── gitgov-server/                 # Control Plane Server (Axum + Rust)
│   ├── src/
│   │   ├── main.rs                # Rutas, rate limiters, bootstrap
│   │   ├── handlers.rs            # HTTP handlers (30+ endpoints)
│   │   ├── auth.rs                # Middleware auth (SHA256 + roles)
│   │   ├── db.rs                  # Database queries (COALESCE siempre)
│   │   └── models.rs              # Data structures (serde + defaults)
│   ├── supabase_schema.sql        # Schema base (v1)
│   ├── supabase_schema_v2.sql     # Índices mejorados
│   ├── supabase_schema_v3.sql     # Governance events, signals
│   ├── supabase_schema_v4.sql     # Append-only triggers
│   ├── supabase_schema_v5.sql     # Jenkins: pipeline_events
│   ├── supabase_schema_v6.sql     # Jira: project_tickets, correlations
│   └── tests/                     # Tests E2E (bash)
│
├── gitgov-web/                    # Sitio Web Público (Next.js 14)
│   ├── app/                       # App Router de Next.js
│   │   ├── page.tsx               # Home
│   │   └── (marketing)/           # Rutas de marketing
│   │       ├── features/          # Features page
│   │       ├── download/          # Download page (calcula checksum)
│   │       ├── pricing/           # Pricing page
│   │       └── contact/           # Contact page
│   ├── components/                # Componentes React
│   ├── lib/
│   │   ├── config/site.ts         # Config global (URL, versión)
│   │   └── i18n/translations.ts   # Traducciones EN/ES
│   └── public/downloads/          # Installer .exe (no commiteado)
│
├── scripts/
│   └── release/desktop-updater/   # Scripts PowerShell para releases
│
├── docs/                          # Documentación del proyecto
├── CLAUDE.md                      # Instrucciones para Claude Code
└── README.md                      # Overview del proyecto
```

## Comandos Útiles

### Desarrollo

```bash
# Desktop con hot reload
cd gitgov && npm run tauri dev

# Server con logs debug
cd gitgov/gitgov-server && RUST_LOG=debug cargo run

# Web App (sitio público)
cd gitgov-web && pnpm dev
```

### Build

```bash
# Desktop release
cd gitgov && npm run tauri build

# Server release
cd gitgov/gitgov-server && cargo build --release

# Web App
cd gitgov-web && pnpm build
```

### Tests

```bash
# Frontend Desktop (vitest)
cd gitgov && npm test

# E2E flow test
cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh

# Jenkins integration
cd gitgov/gitgov-server/tests && API_KEY="tu-key" ./jenkins_integration_test.sh

# Jira integration
cd gitgov/gitgov-server/tests && API_KEY="tu-key" ./jira_integration_test.sh

# Stress test
cd gitgov/gitgov-server/tests && ./stress_test.sh
```

### Linting

```bash
# Server Rust
cd gitgov/gitgov-server && cargo clippy -- -D warnings

# Desktop Rust
cd gitgov/src-tauri && cargo clippy -- -D warnings

# Desktop TypeScript
cd gitgov && npm run lint && npm run typecheck

# Web App
cd gitgov-web && pnpm lint
```

## Flujo de Trabajo Típico

### Hacer un Commit

1. Abrir GitGov Desktop
2. Seleccionar repositorio
3. Ver archivos modificados en el panel izquierdo
4. Seleccionar archivos para stage
5. Escribir mensaje de commit
6. Click "Commit"
7. El evento se registra automáticamente

### Hacer un Push

1. Después de commit, click "Push"
2. Seleccionar rama destino
3. GitGov valida:
   - Nomenclatura de rama
   - Permisos del usuario
   - Rama protegida
4. Si es válido → Push
5. Si no → Mensaje de error específico
6. Evento registrado en Control Plane

### Ver Auditoría

1. Ir a "Control Plane" en la UI
2. Ver estadísticas en tiempo real
3. Ver eventos recientes
4. Filtrar por usuario, tipo, fecha

## Configuración del Repositorio

### gitgov.toml

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

### Variables de Entorno

**Desktop App (`gitgov/.env`):**

| Variable | Propósito | Nota |
|----------|-----------|------|
| `VITE_SERVER_URL` | URL del Control Plane (para frontend Vite) | Solo afecta al UI React |
| `VITE_API_KEY` | API key visible en UI | Para el panel de Control Plane |
| `GITGOV_SERVER_URL` | URL del servidor (para Rust backend) | Leída por `src-tauri/src/lib.rs` |
| `GITGOV_API_KEY` | API key del servidor (para Rust backend) | Leída por `src-tauri/src/lib.rs` |

> **Importante:** La Desktop App tiene DOS capas de configuración: Vite (frontend) usa `VITE_*`, y el backend Rust de Tauri usa `GITGOV_*`. Son independientes.

**Control Plane Server (`gitgov/gitgov-server/.env`):**

| Variable | Propósito |
|----------|-----------|
| `DATABASE_URL` | Conexión PostgreSQL (Supabase pooler) |
| `GITGOV_JWT_SECRET` | Secreto JWT (mín 32 chars) |
| `GITGOV_SERVER_ADDR` | Dirección del servidor (ej. `0.0.0.0:3000`) |
| `GITGOV_API_KEY` | API key inicial (se inserta en DB si no existe) |
| `GITHUB_WEBHOOK_SECRET` | Validación HMAC de webhooks GitHub |
| `JENKINS_WEBHOOK_SECRET` | Secreto para Jenkins (opcional) |
| `JIRA_WEBHOOK_SECRET` | Secreto para Jira (opcional) |
| `RUST_LOG` | Nivel de logging (ej. `gitgov_server=info`) |

## Troubleshooting Rápido

| Problema | Solución |
|----------|----------|
| 401 Unauthorized | Usar `Authorization: Bearer`, no `X-API-Key` |
| Serialization error | Verificar structs cliente/servidor coinciden |
| Outbox no envía | Verificar `GITGOV_SERVER_URL` y `GITGOV_API_KEY` en `gitgov/.env` |
| Dashboard vacío pero outbox OK | Verificar `VITE_SERVER_URL` y `VITE_API_KEY` en `gitgov/.env` |
| DB error | Ejecutar supabase_schema.sql (base) + v2 a v6 en orden |
| App no abre | `npm install` y verificar Node.js 18+ |
| 429 Too Many Requests | Rate limit alcanzado — ajustar `GITGOV_RATE_LIMIT_*_PER_MIN` en .env del servidor |
| localhost vs 127.0.0.1 | Usar siempre `127.0.0.1:3000` como URL canónica en local |
| API key no imprime en Docker | Agregar `--print-bootstrap-key` al comando del servidor |
| Web App no compila | Usar `pnpm` (no npm) en `gitgov-web/` |

## Próximos Pasos

1. ✅ Desktop App funcional
2. ✅ Control Plane conectado
3. ✅ Eventos registrándose
4. ✅ V1.2-A Jenkins — funcional
5. ✅ V1.2-B Jira — preview funcional
6. ✅ Deploy servidor en EC2 (`3.143.150.199`)
7. ✅ Sitio web en Vercel (`https://git-gov.vercel.app`)
8. ⬜ Configurar webhooks GitHub en repos de producción
9. ⬜ Activar HTTPS en EC2 (dominio + Let's Encrypt)
10. ⬜ Configurar servidor de releases para tauri-updater

## Soporte

- Docs: `docs/`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- Arquitectura: `docs/ARCHITECTURE.md`
- Para agentes IA: `AGENTS.md`
