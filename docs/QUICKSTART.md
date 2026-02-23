# GitGov - Guía de Inicio Rápido

## Requisitos Previos

- Node.js 18+
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

En Supabase SQL Editor:
```sql
-- Ejecutar contenido de supabase_schema.sql
```

### 4. Ejecutar

**Terminal 1 - Server:**
```bash
cd gitgov-server
cargo run
```

**Terminal 2 - Desktop:**
```bash
cd gitgov
npm run tauri dev
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
├── gitgov/                    # Desktop App (Tauri)
│   ├── src/                   # Frontend React
│   │   ├── components/        # Componentes UI
│   │   ├── store/             # Estado Zustand
│   │   └── lib/               # Utilidades
│   ├── src-tauri/             # Backend Rust
│   │   └── src/
│   │       ├── commands/      # Tauri commands
│   │       ├── git/           # Operaciones Git
│   │       ├── outbox/        # Cola offline
│   │       └── audit/         # SQLite local
│   └── gitgov.toml            # Config del repo
│
├── gitgov-server/             # Control Plane Server
│   ├── src/
│   │   ├── handlers.rs        # HTTP handlers
│   │   ├── auth.rs            # Middleware auth
│   │   ├── db.rs              # Database access
│   │   └── models.rs          # Data structures
│   ├── supabase_schema.sql    # DB schema
│   └── tests/                 # Tests E2E
│
├── docs/                      # Documentación
├── AGENTS.md                  # Guía para agentes IA
└── README.md                  # Este archivo
```

## Comandos Útiles

### Desarrollo

```bash
# Desktop con hot reload
cd gitgov && npm run tauri dev

# Server con logs debug
cd gitgov-server && RUST_LOG=debug cargo run

# Ver logs en tiempo real
RUST_LOG=gitgov=debug,gitgov_server=debug cargo run
```

### Build

```bash
# Desktop release
cd gitgov && npm run tauri build

# Server release
cd gitgov-server && cargo build --release
```

### Tests

```bash
# E2E flow test
cd gitgov-server/tests
./e2e_flow_test.sh

# Stress test
./stress_test.sh
```

### Linting

```bash
# Rust
cargo clippy -- -D warnings

# TypeScript
npm run lint
npm run typecheck
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

| Variable | Dónde | Propósito |
|----------|-------|-----------|
| `VITE_SERVER_URL` | Desktop .env | URL del Control Plane |
| `VITE_API_KEY` | Desktop .env | API key para autenticación |
| `DATABASE_URL` | Server .env | Conexión PostgreSQL |
| `GITGOV_API_KEY` | Server .env | API key admin (se inserta en DB) |
| `GITHUB_WEBHOOK_SECRET` | Server .env | Validación de webhooks |

## Troubleshooting Rápido

| Problema | Solución |
|----------|----------|
| 401 Unauthorized | Usar `Authorization: Bearer`, no `X-API-Key` |
| Serialization error | Verificar structs cliente/servidor coinciden |
| Outbox no envía | Verificar SERVER_URL y API_KEY en .env |
| DB error | Ejecutar supabase_schema.sql |
| App no abre | `npm install` y verificar Node.js 18+ |

## Próximos Pasos

1. ✅ Desktop App funcional
2. ✅ Control Plane conectado
3. ✅ Eventos registrándose
4. ⬜ Configurar webhooks de GitHub
5. ⬜ Implementar correlation engine
6. ⬜ Deploy a producción

## Soporte

- Docs: `docs/`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- Arquitectura: `docs/ARCHITECTURE.md`
- Para agentes IA: `AGENTS.md`
