# GitGov

**Git Governance Control Plane** - Sistema de gobernanza de Git con auditoría centralizada.

## Estado del Proyecto

**✅ Funcional** - El pipeline Desktop → Server → Dashboard está operativo.

## Inicio Rápido

```bash
# 1. Control Plane Server
cd gitgov-server
cp .env.example .env
# Editar .env con credenciales de Supabase
cargo run

# 2. Desktop App
cd gitgov
npm install
npm run tauri dev
```

Ver [QUICKSTART.md](./docs/QUICKSTART.md) para guía completa.

## Componentes

| Componente | Tecnología | Ubicación |
|------------|------------|-----------|
| Desktop App | Tauri v2 + React | `gitgov/` |
| Control Plane Server | Axum + Rust | `gitgov-server/` |
| Database | PostgreSQL (Supabase) | Supabase Cloud |

## Funcionalidades

- ✅ Dashboard principal con commits y pushes
- ✅ Control Plane conectado
- ✅ Pipeline de eventos E2E
- ✅ Autenticación GitHub OAuth
- ✅ Outbox offline con reintentos
- ✅ Auditoría centralizada

## Documentación

| Documento | Propósito |
|-----------|-----------|
| [QUICKSTART.md](./docs/QUICKSTART.md) | Guía de inicio (5 min) |
| [ARCHITECTURE.md](./docs/ARCHITECTURE.md) | Arquitectura del sistema |
| [TROUBLESHOOTING.md](./docs/TROUBLESHOOTING.md) | Solución de problemas |
| [PROGRESS.md](./docs/PROGRESS.md) | Registro de cambios |
| [AGENTS.md](./AGENTS.md) | Guía para agentes IA |

## Scripts de Prueba

```bash
# E2E flow test
cd gitgov-server/tests
./e2e_flow_test.sh

# Stress test
./stress_test.sh
```

## Licencia

MIT
