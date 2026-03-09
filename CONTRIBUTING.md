# Contributing to GitGov

Thank you for your interest in contributing to GitGov! This guide will help you get started.

## Project Structure

```
GitGov/
├── gitgov/                    # Desktop App (Tauri v2 + React 19)
│   ├── src/                   # React frontend (TypeScript, Tailwind v4)
│   │   ├── components/        # UI components
│   │   ├── store/             # Zustand v5 stores
│   │   ├── lib/               # Utilities and types
│   │   ├── pages/             # Route pages
│   │   └── test/              # Vitest unit tests
│   ├── src-tauri/             # Tauri backend (Rust)
│   │   ├── src/commands/      # Tauri commands exposed to frontend
│   │   ├── src/control_plane/ # HTTP client for server communication
│   │   └── src/outbox/        # Offline event queue
│   └── gitgov-server/         # Control Plane Server (Axum + Rust)
│       ├── src/handlers/      # API route handlers
│       ├── src/models.rs      # Shared data structures
│       ├── src/auth.rs        # Authentication middleware
│       └── src/db.rs          # Database queries
├── gitgov-web/                # Marketing website (Next.js 14)
└── docs/                      # Documentation
```

## Prerequisites

- **Rust** (latest stable) — for server and Tauri backend
- **Node.js** 20+ — for frontend and web app
- **PostgreSQL** — for the Control Plane database
- **Tauri v2 prerequisites** — see [Tauri docs](https://v2.tauri.app/start/prerequisites/)

## Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/MapfrePE/GitGov.git
   cd GitGov
   ```

2. **Set up environment variables:**
   ```bash
   # Server
   cp gitgov/gitgov-server/.env.example gitgov/gitgov-server/.env
   # Edit .env with your DATABASE_URL, GITGOV_JWT_SECRET, GITGOV_API_KEY

   # Desktop
   cp gitgov/.env.example gitgov/.env
   # Edit .env with your VITE_SERVER_URL, VITE_API_KEY
   ```

3. **Start the server:**
   ```bash
   cd gitgov/gitgov-server && cargo run
   ```

4. **Start the desktop app:**
   ```bash
   cd gitgov && npm install && npm run tauri dev
   ```

## Running Tests

```bash
# Server unit tests (99+ tests, no DB required)
cd gitgov/gitgov-server && cargo test

# Desktop Rust tests
cd gitgov/src-tauri && cargo test

# Frontend unit tests (vitest)
cd gitgov && npm test

# Lint checks
cd gitgov/gitgov-server && cargo clippy -- -D warnings
cd gitgov/src-tauri && cargo clippy -- -D warnings
cd gitgov && npm run lint && npm run typecheck
```

## Making Changes

### Golden Path Rule

The following flow is sacred and must **never** be broken:

1. Desktop detects changed files
2. User commits from the app
3. User pushes from the app
4. Control Plane receives events (`stage_files`, `commit`, `attempt_push`, `successful_push`)
5. Dashboard shows logs/commits without `401` errors

If your change touches auth, tokens, API handlers, or the dashboard, you must verify this flow still works.

### Code Conventions

**Rust (Server + Desktop backend):**
- Use `#[serde(default)]` on all optional and HashMap fields
- Use `COALESCE` in SQL aggregations that may return NULL
- Audit tables are append-only (no UPDATE/DELETE)
- Authentication: always `Authorization: Bearer {key}` (never `X-API-Key`)

**TypeScript (Frontend):**
- State management: Zustand v5 stores in `src/store/`
- Types: interfaces in `src/lib/types.ts`
- Styles: Tailwind v4 utility classes

**Shared structs** must stay in sync across three layers:
- `gitgov-server/src/models.rs` (canonical)
- `src-tauri/src/control_plane/server.rs` (Tauri copy)
- `src/lib/types.ts` (frontend copy)

### Commit Messages

Use conventional commits:
```
feat: add pipeline health widget
fix: resolve 401 on dashboard refresh
refactor: extract SSE reconnect logic
docs: update API endpoint table
test: add useAuthStore unit tests
```

### Acceptance Criteria

Before opening a PR, ensure:

- [ ] `cargo test` passes in `gitgov-server/`
- [ ] `cargo clippy -- -D warnings` passes in `gitgov-server/` and `src-tauri/`
- [ ] `npm run typecheck` passes in `gitgov/`
- [ ] Zero new ESLint errors in files you touched
- [ ] No secrets committed (check `.env` files are in `.gitignore`)
- [ ] Golden Path still works (if touching auth/handlers/dashboard)

## Security

- **Never** commit tokens, API keys, or secrets
- API keys are SHA256-hashed before storage
- Audit events are append-only and immutable
- Report security issues privately (do not open public issues)
