# GITGOV — Plan Maestro para Claude Code (Opus 4.6)

> **Instrucción inicial al agente:** Lee este documento completo antes de escribir cualquier archivo. Cada sección es una fase. No adelantes fases. Cuando termines una fase, reporta qué hiciste y espera confirmación antes de continuar. Si hay ambigüedad en algún paso, pregunta antes de implementar. El objetivo es construir un MVP funcional, robusto y con buenas prácticas desde el primer commit.

---

## Contexto del problema

Una empresa de desarrollo de software tiene un repositorio en GitHub donde todos los desarrolladores hacen `git push` directamente a ramas sin ningún control. Esto genera dos problemas graves:

1. **El desarrollador no puede elegir qué enviar:** Al hacer push, sube todo lo que tiene en el working tree, incluyendo archivos de debug, configuraciones locales, o cambios no relacionados que no debería subir.
2. **No hay visibilidad:** El arquitecto, product manager y admin no saben quién subió qué, cuándo, ni si siguió las reglas del equipo.

La solución es una **aplicación de escritorio** (GitGov) que reemplaza el flujo de git directo con un workflow controlado, con roles, nomenclatura de ramas forzada, staging selectivo y logs de auditoría.

---

## Qué resuelve GitGov

- El desarrollador abre GitGov, ve sus archivos modificados, y **elige exactamente qué archivos o hunks incluir** en el commit, sin tener que tocar la terminal.
- La app **valida que la rama cumpla la nomenclatura** de la empresa antes de permitir el push.
- La app **registra en logs** quién hizo qué, cuándo, a qué rama, con qué archivos, y si fue bloqueado o tuvo éxito.
- El **admin/arquitecto** tiene una pantalla de auditoría donde ve todos los eventos de todos los devs.
- Los **developers** solo ven y operan en las ramas y paths que su grupo tiene permitidos.

---

## Stack tecnológico

- **Framework Desktop:** Tauri v2 (Rust backend + WebView frontend)
- **Frontend:** React 18 + TypeScript + Vite
- **Estilos:** Tailwind CSS v3
- **Estado global frontend:** Zustand
- **Comunicación frontend↔backend:** Tauri Commands (invoke)
- **Git operations:** Rust crate `git2` (bindings de libgit2)
- **GitHub API:** Rust crate `reqwest` + `octocrab`
- **Autenticación GitHub:** OAuth 2.0 Device Flow
- **Base de datos local:** SQLite vía Rust crate `rusqlite` (bundled)
- **Configuración del proyecto:** Archivo `gitgov.toml` en la raíz del repo de la empresa
- **Serialización:** `serde` + `serde_json`
- **Diff viewer UI:** librería `react-diff-view` + `unidiff`
- **Linting/Format frontend:** ESLint + Prettier
- **Linting/Format backend:** `clippy` + `rustfmt`
- **Seguridad de tokens:** Rust crate `keyring` (guarda en llavero del OS)

---

## Principios de arquitectura que el agente debe respetar

1. **Rust hace el trabajo pesado.** Toda operación de Git, toda validación de reglas, todo acceso a SUPABASE, y toda llamada a GitHub API ocurre en Rust. El frontend solo muestra datos y captura intenciones del usuario.

2. **El frontend no tiene lógica de negocio.** React solo llama a comandos Tauri y renderiza la respuesta. Nunca toma decisiones sobre si un push es válido o no.

3. **Nunca guardes tokens en texto plano.** El token de GitHub se guarda en el llavero del sistema operativo con `keyring`. Nunca en un archivo, nunca en localStorage, nunca en la base de datos.

4. **Todos los eventos pasan por el audit log.** No importa si fue exitoso o bloqueado, toda acción del developer se registra en SUPABASE con timestamp, usuario, acción, rama, archivos involucrados y resultado.

5. **La configuración vive en el repositorio.** El archivo `gitgov.toml` en la raíz del repo de la empresa define las reglas: grupos, nomenclatura, ramas protegidas. Esto hace que las reglas sean versionables y auditables.

6. **Errores explícitos hacia el usuario.** Cuando algo falla o es bloqueado, el frontend muestra el motivo exacto. Nunca un error genérico.

7. **Sin dependencias de terceros para lógica crítica.** Las validaciones de nomenclatura, la lógica de permisos y el audit log son implementación propia en Rust. No dependen de servicios externos.

---

## Estructura de archivos completa del proyecto

El agente debe crear exactamente esta estructura. Se explica cada archivo a continuación.

```
gitgov/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── auth_commands.rs
│       │   ├── git_commands.rs
│       │   ├── branch_commands.rs
│       │   ├── audit_commands.rs
│       │   └── config_commands.rs
│       ├── git/
│       │   ├── mod.rs
│       │   ├── repository.rs
│       │   ├── staging.rs
│       │   ├── diff.rs
│       │   └── branch.rs
│       ├── github/
│       │   ├── mod.rs
│       │   ├── auth.rs
│       │   └── api.rs
│       ├── audit/
│       │   ├── mod.rs
│       │   └── db.rs
│       ├── config/
│       │   ├── mod.rs
│       │   └── validator.rs
│       └── models/
│           ├── mod.rs
│           ├── file_change.rs
│           ├── audit_log.rs
│           ├── branch_rule.rs
│           └── user.rs
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── router.tsx
│   ├── styles/
│   │   └── globals.css
│   ├── store/
│   │   ├── useAuthStore.ts
│   │   ├── useRepoStore.ts
│   │   └── useAuditStore.ts
│   ├── hooks/
│   │   ├── useGitStatus.ts
│   │   ├── useAuth.ts
│   │   ├── useBranches.ts
│   │   └── useAuditLogs.ts
│   ├── components/
│   │   ├── layout/
│   │   │   ├── Sidebar.tsx
│   │   │   ├── Header.tsx
│   │   │   └── MainLayout.tsx
│   │   ├── auth/
│   │   │   └── LoginScreen.tsx
│   │   ├── repo/
│   │   │   └── RepoSelector.tsx
│   │   ├── diff/
│   │   │   ├── DiffViewer.tsx
│   │   │   ├── FileList.tsx
│   │   │   └── HunkSelector.tsx
│   │   ├── branch/
│   │   │   ├── BranchSelector.tsx
│   │   │   └── BranchCreator.tsx
│   │   ├── commit/
│   │   │   └── CommitPanel.tsx
│   │   ├── audit/
│   │   │   ├── AuditLogView.tsx
│   │   │   └── AuditLogRow.tsx
│   │   └── shared/
│   │       ├── Button.tsx
│   │       ├── Badge.tsx
│   │       ├── Modal.tsx
│   │       ├── Toast.tsx
│   │       └── Spinner.tsx
│   ├── pages/
│   │   ├── DashboardPage.tsx
│   │   ├── AuditPage.tsx
│   │   └── SettingsPage.tsx
│   └── lib/
│       ├── tauri.ts
│       ├── types.ts
│       └── constants.ts
├── gitgov.toml              ← Ejemplo de config para el repo de la empresa
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
├── .eslintrc.json
└── .prettierrc
```

---

## FASE 1 — Scaffold del proyecto

### 1.1 Inicialización

Crea el proyecto con Tauri CLI v2. Usa el template de React + TypeScript + Vite. El nombre del proyecto es `gitgov`, el identificador es `com.empresa.gitgov`.

### 1.2 Dependencias del backend (Cargo.toml)

El archivo `Cargo.toml` de `src-tauri` debe declarar como dependencias:

- `tauri` versión 2 con features: `shell-open`
- `git2` versión 0.18 con features: `vendored-openssl` (para que funcione en todas las plataformas sin requerir OpenSSL del sistema)
- `reqwest` versión 0.11 con features: `json`, `blocking`
- `tokio` versión 1 con features: `full`
- `serde` versión 1 con features: `derive`
- `serde_json` versión 1
- `rusqlite` versión 0.31 con features: `bundled` (SQLite incluido en el binario, sin dependencia del sistema)
- `keyring` versión 2
- `toml` versión 0.8 (para leer `gitgov.toml`)
- `glob` versión 0.3 (para validar paths con wildcards)
- `chrono` versión 0.4 con features: `serde`
- `thiserror` versión 1 (para errores tipados en Rust)
- `uuid` versión 1 con features: `v4`
- `octocrab` versión 0.38 (cliente GitHub API)

### 1.3 Dependencias del frontend (package.json)

- `react` y `react-dom` versión 18
- `typescript` versión 5
- `vite` y `@vitejs/plugin-react`
- `@tauri-apps/api` versión 2
- `zustand` versión 4
- `react-diff-view` versión 3 (diff viewer con staging por hunk)
- `unidiff` versión 1 (para parsear diffs unificados)
- `react-router-dom` versión 6
- `tailwindcss`, `autoprefixer`, `postcss`
- `@headlessui/react` (modales, dropdowns accesibles)
- `lucide-react` (iconos)
- `date-fns` (formateo de fechas)
- `clsx` (clases CSS condicionales)
- Como devDependencies: `eslint`, `prettier`, `@typescript-eslint/eslint-plugin`, `@typescript-eslint/parser`

### 1.4 Configuración de TypeScript

El `tsconfig.json` debe usar `strict: true`, `noImplicitAny: true`, `target: ESNext`, y `moduleResolution: bundler`. Todos los tipos deben ser explícitos, sin `any`.

### 1.5 Configuración de ESLint

`.eslintrc.json` debe extender `@typescript-eslint/recommended` y tener reglas que prohíban: `any` implícito, funciones sin tipo de retorno, variables no usadas.

### 1.6 Configuración de Tailwind

`tailwind.config.js` debe apuntar a todos los archivos `.tsx` y `.ts` en `src/`. Define colores personalizados para la paleta de la app:

- `brand`: un tono de azul oscuro profesional (para botones primarios, acentos)
- `surface`: grises para fondos de paneles
- `success`, `warning`, `danger`: para badges de estado en el audit log

---

## FASE 2 — Modelos de datos (Rust)

Antes de implementar lógica, define todas las estructuras de datos en `src/models/`. Todas deben derivar `Serialize`, `Deserialize`, `Debug`, y `Clone`.

### 2.1 `models/file_change.rs`

Define una estructura `FileChange` con los campos:
- `path`: String — ruta relativa del archivo desde la raíz del repo
- `status`: enum `ChangeStatus` con variantes: `Modified`, `Added`, `Deleted`, `Renamed`, `Untracked`
- `staged`: bool — indica si este archivo ya fue incluido en el stage
- `diff`: Option<String> — el diff unificado del archivo, cargado bajo demanda

Define `ChangeStatus` como enum que implementa `Display` para mostrar texto en la UI.

### 2.2 `models/audit_log.rs`

Define `AuditLogEntry` con:
- `id`: String (UUID v4)
- `timestamp`: i64 (Unix timestamp en milisegundos)
- `developer_login`: String (GitHub username)
- `developer_name`: String (nombre real de GitHub)
- `action`: enum `AuditAction` con variantes: `Push`, `BranchCreate`, `StageFile`, `Commit`, `BlockedPush`, `BlockedBranch`
- `branch`: String
- `files`: Vec<String> (paths de archivos involucrados)
- `commit_hash`: Option<String>
- `status`: enum `AuditStatus` con variantes: `Success`, `Blocked`, `Failed`
- `reason`: Option<String> (motivo si fue bloqueado o falló)

### 2.3 `models/branch_rule.rs`

Define `GitGovConfig` que mapea directamente la estructura del `gitgov.toml`:

- `branches.patterns`: Vec<String> — patrones glob válidos para nombres de rama
- `branches.protected`: Vec<String> — ramas que nadie puede tocar directamente
- `groups`: HashMap<String, GroupConfig>

Define `GroupConfig` con:
- `members`: Vec<String> — GitHub usernames
- `allowed_branches`: Vec<String> — patrones glob de ramas que este grupo puede crear/pushear
- `allowed_paths`: Vec<String> — patrones glob de paths de archivos que este grupo puede modificar

### 2.4 `models/user.rs`

Define `AuthenticatedUser` con:
- `login`: String (GitHub username)
- `name`: String
- `avatar_url`: String
- `group`: Option<String> — nombre del grupo en `gitgov.toml` al que pertenece, `None` si es admin
- `is_admin`: bool

---

## FASE 3 — Módulo de configuración (Rust)

### 3.1 `config/mod.rs`

Implementa la función `load_config(repo_path: &str) -> Result<GitGovConfig, ConfigError>` que:
1. Construye la ruta `{repo_path}/gitgov.toml`
2. Lee el archivo a String
3. Lo parsea con la crate `toml` al tipo `GitGovConfig`
4. Retorna error descriptivo si el archivo no existe o tiene formato inválido

Define `ConfigError` como enum con `thiserror`: `FileNotFound`, `ParseError(String)`, `InvalidPattern(String)`.

### 3.2 `config/validator.rs`

Implementa todas las funciones de validación. Estas funciones son el corazón de las reglas de negocio:

**`validate_branch_name(name: &str, config: &GitGovConfig, user: &AuthenticatedUser) -> ValidationResult`**

Lógica en orden:
1. Si la rama está en `config.branches.protected`, retorna `Blocked` con mensaje "Rama protegida. No puedes operar directamente en {name}."
2. Si el usuario es admin, omite validaciones de grupo y retorna `Valid`.
3. Busca el grupo del usuario. Si no tiene grupo, retorna `Blocked` con mensaje "No perteneces a ningún grupo configurado."
4. Verifica que algún patrón en `group.allowed_branches` coincida con el nombre de la rama usando glob matching.
5. Verifica que el nombre de rama coincida con algún patrón en `config.branches.patterns`.
6. Si pasa todo, retorna `Valid`.

**`validate_file_paths(files: &[String], config: &GitGovConfig, user: &AuthenticatedUser) -> Vec<PathValidationResult>`**

Para cada archivo, verifica si el grupo del usuario tiene permiso sobre ese path según `allowed_paths`. Retorna un resultado por archivo indicando si está permitido o no.

**`validate_commit_message(message: &str) -> CommitMessageValidation`**

Valida que el mensaje siga Conventional Commits: debe comenzar con uno de los prefijos válidos (`feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `hotfix`) seguido de dos puntos y espacio. Si no cumple, retorna el mensaje de error con el formato esperado.

Define `ValidationResult` como enum: `Valid`, `Blocked(String)`. Define `PathValidationResult` con el path y si está `Allowed` o `Denied(String)`.

---

## FASE 4 — Módulo de auditoría (Rust)

### 4.1 `audit/db.rs`

Implementa `AuditDatabase` como struct que encapsula una conexión SQLite.

**`AuditDatabase::new(db_path: &str) -> Result<Self, AuditError>`**

Crea o abre la base de datos SQLite. Ejecuta las migraciones necesarias en el momento de inicialización. La tabla `audit_logs` debe tener los campos que corresponden a `AuditLogEntry`. Crea índices en `developer_login`, `timestamp`, `action`, y `status` para que las consultas del admin sean rápidas.

**`AuditDatabase::insert(&self, entry: &AuditLogEntry) -> Result<(), AuditError>`**

Inserta un registro de auditoría. Los `files` se guardan como JSON string.

**`AuditDatabase::query(&self, filter: &AuditFilter) -> Result<Vec<AuditLogEntry>, AuditError>`**

Permite filtrar logs por: rango de fechas, developer_login, action, status, y branch. Retorna los resultados ordenados por timestamp descendente. Soporte para paginación: `limit` y `offset`.

Define `AuditFilter` como struct con todos los campos opcionales y `limit: usize` con default 100.

**`AuditDatabase::get_stats(&self) -> Result<AuditStats, AuditError>`**

Retorna estadísticas agregadas: total pushes hoy, total bloqueados hoy, devs activos esta semana, acción más frecuente. Esto alimenta el dashboard del admin.

Define `AuditError` con `thiserror`: `DatabaseError(String)`, `SerializationError(String)`.

---

## FASE 5 — Módulo de Git (Rust)

### 5.1 `git/repository.rs`

**`open_repository(path: &str) -> Result<git2::Repository, GitError>`**

Abre el repositorio. Si falla, el error debe decir claramente si es porque la ruta no existe, no es un repo de git, o no tiene permisos.

**`get_working_tree_changes(repo: &git2::Repository) -> Result<Vec<FileChange>, GitError>`**

Obtiene todos los archivos con cambios. Incluye: modificados (tracked), agregados (untracked que se quieren incluir), y eliminados. No incluye archivos en `.gitignore`. El campo `diff` de cada `FileChange` se deja en `None` aquí para no cargar todos los diffs de golpe.

**`get_current_branch(repo: &git2::Repository) -> Result<String, GitError>`**

Retorna el nombre de la rama actual.

### 5.2 `git/diff.rs`

**`get_file_diff(repo: &git2::Repository, file_path: &str) -> Result<String, GitError>`**

Retorna el diff unificado de un archivo específico como String. El frontend lo envía a `react-diff-view` para renderizarlo. El contexto del diff debe ser de 3 líneas (estándar).

### 5.3 `git/staging.rs`

**`stage_files(repo: &git2::Repository, files: &[String]) -> Result<(), GitError>`**

Agrega los archivos especificados al index (staging area). Maneja correctamente archivos nuevos, modificados, y eliminados (para los eliminados usa `index.remove_path`).

**`unstage_all(repo: &git2::Repository) -> Result<(), GitError>`**

Limpia el index volviendo al HEAD. Útil para que el usuario pueda empezar de nuevo.

**`create_commit(repo: &git2::Repository, message: &str, author_name: &str, author_email: &str) -> Result<String, GitError>`**

Crea un commit con los archivos que están en el index. Retorna el hash del commit creado. Si el index está vacío (nada en staging), retorna error descriptivo.

### 5.4 `git/branch.rs`

**`list_branches(repo: &git2::Repository) -> Result<Vec<BranchInfo>, GitError>`**

Lista todas las ramas locales y remotas. Retorna `BranchInfo` con: nombre, si es la rama actual, si es remota, y último commit hash + mensaje.

**`create_branch(repo: &git2::Repository, name: &str, from_branch: &str) -> Result<(), GitError>`**

Crea una nueva rama a partir de la rama especificada. Si `from_branch` no existe, retorna error.

**`checkout_branch(repo: &git2::Repository, name: &str) -> Result<(), GitError>`**

Cambia a la rama especificada. Si hay cambios sin commitear, retorna error claro (no hace stash automático para no confundir al usuario).

**`push_to_remote(repo: &git2::Repository, branch: &str, token: &str) -> Result<(), GitError>`**

Hace push de la rama al remote `origin`. Usa el token de GitHub como credencial (`oauth2` / token). Maneja el caso de que la rama remota no exista aún (primer push, agrega `--set-upstream` equivalente).

Define `BranchInfo` como struct serializable. Define `GitError` como enum con `thiserror` cubriendo todos los casos: `RepoNotFound`, `InvalidPath`, `EmptyStaging`, `PushFailed(String)`, `BranchExists`, `BranchNotFound`, `UncommittedChanges`, `Unauthorized`, `UnknownError(String)`.

---

## FASE 6 — Módulo de GitHub (Rust)

### 6.1 `github/auth.rs`

Implementa el flujo OAuth 2.0 Device Flow. Este es el único flujo correcto para apps de escritorio — no requiere servidor intermediario.

**`start_device_flow(client_id: &str) -> Result<DeviceFlowResponse, AuthError>`**

Llama a `https://github.com/login/device/code` con el `client_id` de la GitHub App. Retorna `DeviceFlowResponse` con: `device_code`, `user_code`, `verification_uri`, `expires_in`, `interval`.

**`poll_for_token(client_id: &str, device_code: &str, interval: u64) -> Result<String, AuthError>`**

Hace polling a `https://github.com/login/oauth/access_token` cada `interval` segundos. Maneja los estados intermedios: `authorization_pending` (sigue esperando), `slow_down` (aumenta el intervalo), `expired_token` (error), `access_denied` (error). Retorna el access token cuando el usuario autoriza.

**`save_token(service: &str, username: &str, token: &str) -> Result<(), AuthError>`**

Guarda el token en el llavero del OS con la crate `keyring`. El `service` es `"gitgov"`, el `username` es el GitHub login.

**`load_token(service: &str, username: &str) -> Result<String, AuthError>`**

Carga el token del llavero. Retorna error si no existe o fue revocado.

**`delete_token(service: &str, username: &str) -> Result<(), AuthError>`**

Elimina el token del llavero (logout).

Define `AuthError` con: `NetworkError(String)`, `TokenExpired`, `AccessDenied`, `KeyringError(String)`, `Unauthorized`.

### 6.2 `github/api.rs`

**`get_authenticated_user(token: &str) -> Result<GithubUser, ApiError>`**

Llama a `GET https://api.github.com/user`. Retorna `GithubUser` con `login`, `name`, `email`, `avatar_url`. Incluye el header `User-Agent: GitGov/1.0` en todas las llamadas (GitHub lo requiere).

**`get_repository_info(token: &str, owner: &str, repo: &str) -> Result<RepoInfo, ApiError>`**

Verifica que el repositorio existe y el usuario tiene acceso. Retorna `RepoInfo` con nombre, descripción, y si el usuario tiene permisos de escritura.

**`setup_branch_protection(token: &str, owner: &str, repo: &str, branch: &str) -> Result<(), ApiError>`**

Configura branch protection en GitHub para la rama especificada. Requiere que el usuario sea admin del repo. Esto previene pushes directos desde fuera de GitGov a ramas protegidas.

---

## FASE 6.5 — GitGov Control Plane (Backend central en Rust) [IMPLEMENTADO]

### Estado: COMPLETADO

### Objetivo
Centralizar auditoría, políticas y visibilidad cross-dev (un "single pane of glass" para admin/arquitecto/PM).

### Arquitectura Implementada

```
┌─────────────────┐     ┌─────────────────┐
│   GitHub        │────▶│  Webhook        │
│   (Webhooks)    │     │  POST /webhooks │
└─────────────────┘     └────────┬────────┘
                                 │
                                 ▼
┌─────────────────┐     ┌─────────────────┐
│   Desktop App   │────▶│  POST /events   │
│   (Outbox)      │     │  (Batch)        │
└─────────────────┘     └────────┬────────┘
                                 │
                                 ▼
                        ┌─────────────────┐
                        │   Supabase      │
                        │   PostgreSQL    │
                        │   (Append-only) │
                        └─────────────────┘
```

### Separación Source of Truth vs Telemetría

| Tabla | Origen | Propósito | Append-only |
|-------|--------|-----------|-------------|
| `github_events` | Webhooks GitHub | Source of truth - eventos reales | Sí |
| `client_events` | Desktop app | Telemetría de intentos/bloqueos | Sí |
| `violations` | Server | Violaciones de política detectadas | Sí |

### Componentes Implementados

#### 1. Supabase Schema (`gitgov-server/supabase_schema.sql`)

**Tablas:**
- `orgs` - Organizaciones
- `repos` - Repositorios (vinculados a org)
- `members` - Miembros con roles (Admin, Architect, Developer, PM)
- `github_events` - Eventos de GitHub webhooks (append-only)
- `client_events` - Telemetría del desktop (append-only)
- `violations` - Violaciones de política (append-only)
- `policies` - Config gitgov.toml por repo
- `api_keys` - API keys para autenticación
- `webhook_events` - Raw webhooks para debugging

**Append-Only Triggers:**
```sql
CREATE TRIGGER github_events_append_only
    BEFORE UPDATE OR DELETE ON github_events
    FOR EACH ROW EXECUTE FUNCTION prevent_update_delete();
```

**Idempotencia:**
- `github_events.delivery_id` UNIQUE (X-GitHub-Delivery header)
- `client_events.event_uuid` UNIQUE (generado por cliente)

**RLS Policies:**
- Admins ven todo
- Developers ven solo sus propios eventos

#### 2. Server Endpoints (`gitgov-server/src/handlers.rs`)

| Método | Path | Handler | Descripción |
|--------|------|---------|-------------|
| GET | `/health` | health | Health check |
| POST | `/webhooks/github` | handle_github_webhook | Recibe webhooks con HMAC validation |
| POST | `/events` | ingest_client_events | Batch de eventos del desktop |
| GET | `/logs` | get_logs | Query combinada con filtros |
| GET | `/stats` | get_stats | Estadísticas agregadas |
| GET | `/dashboard` | get_dashboard | Datos para dashboard |
| GET | `/policy/:repo` | get_policy | Obtener política del repo |
| PUT | `/policy/:repo` | save_policy | Guardar política |
| POST | `/api-keys` | create_api_key | Crear API key |

#### 3. GitHub Webhook Handler

**Eventos soportados:**
- `push` - Commits a ramas
- `create` - Creación de branches/tags

**Validación HMAC:**
```rust
fn validate_github_signature(secret: &str, payload: &serde_json::Value, signature: &str) -> bool
```

**Flujo:**
1. Recibe webhook con signature
2. Valida HMAC con GITHUB_WEBHOOK_SECRET
3. Extrae delivery_id para idempotencia
4. Parsea payload según event_type
5. Upsert org y repo
6. Inserta en github_events
7. Almacena raw webhook en webhook_events

#### 4. Client Events Batch

**Request:**
```json
{
  "events": [
    {
      "event_uuid": "uuid-v4",
      "event_type": "blocked_push",
      "user_login": "developer",
      "branch": "main",
      "status": "blocked",
      "reason": "Branch is protected",
      "repo_full_name": "org/repo",
      "timestamp": 1234567890000
    }
  ],
  "client_version": "0.1.0"
}
```

**Response:**
```json
{
  "accepted": ["uuid-1", "uuid-2"],
  "duplicates": [],
  "errors": []
}
```

#### 5. Desktop Outbox (`src-tauri/src/outbox/`)

**Características:**
- Almacenamiento JSONL (una línea por evento)
- Generación de event_uuid único
- Batch flush al server
- Reintentos con backoff exponencial (máx 5)
- Background worker para flush automático
- Deduplicación automática

**Flujo:**
```
Evento auditado → Escribir en outbox.jsonl
                        ↓
              Intentar enviar batch
                        ↓
         ┌─────── Éxito ───────┐
         │                      │
    Marcar como enviado    Queda pendiente
         │                      │
    Limpiar archivo       Reintento con backoff
```

### Variables de Entorno

```env
# Supabase PostgreSQL
DATABASE_URL=postgresql://postgres:PASSWORD@db.PROJECT.supabase.co:5432/postgres

# JWT para autenticación
GITGOV_JWT_SECRET=your-secret

# Dirección del servidor
GITGOV_SERVER_ADDR=0.0.0.0:3000

# GitHub Webhook Secret (validación HMAC)
GITHUB_WEBHOOK_SECRET=your-webhook-secret
```

### Configuración de GitHub Webhook

1. Ir a Repository Settings > Webhooks > Add webhook
2. Payload URL: `https://your-server.com/webhooks/github`
3. Content type: `application/json`
4. Secret: Mismo que `GITHUB_WEBHOOK_SECRET`
5. Events: `push`, `create`

### Pruebas con cURL

```bash
# Health check
curl http://localhost:3000/health

# Enviar eventos batch
curl -X POST http://localhost:3000/events \
  -H "Content-Type: application/json" \
  -d '{"events":[{"event_uuid":"550e8400-e29b-41d4-a716-446655440000","event_type":"blocked_push","user_login":"test","branch":"main","status":"blocked","timestamp":1234567890000}]}'

# Simular webhook push
curl -X POST http://localhost:3000/webhooks/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: test-123" \
  -d '{"ref":"refs/heads/main","before":"abc","after":"def","repository":{"id":1,"name":"repo","full_name":"org/repo","private":false,"owner":{"id":1,"login":"org"},"organization":{"id":1,"login":"org"}},"sender":{"id":1,"login":"dev"},"commits":[]}'
```

### Archivos del Server

```
gitgov-server/
├── Cargo.toml           # Dependencias: axum, sqlx, hmac, hex
├── .env.example         # Template de variables
├── README.md            # Documentación de setup
├── supabase_schema.sql  # Schema completo para SQL Editor
└── src/
    ├── main.rs          # Entry point con rutas
    ├── handlers.rs      # HTTP handlers
    ├── db.rs            # Operaciones PostgreSQL
    └── models.rs        # Structs Rust
```

### Notas Importantes

1. **Desktop NUNCA escribe directo a Supabase** - Todo pasa por el server
2. **Source of truth = GitHub webhooks** - client_events es telemetría
3. **Append-only es inviolable** - Triggers SQL lanzan excepción
4. **Idempotencia obligatoria** - UUIDs únicos previenen duplicados

### Siguientes Pasos

- [ ] Integrar outbox en los commands de desktop
- [ ] Pruebas end-to-end con Supabase real
- [ ] Web frontend para admin (opcional)
- [ ] Deploy del server

---


### 7.1 Sobre el manejo de errores en comandos

Todos los comandos Tauri deben retornar `Result<T, String>` donde el `String` de error es un JSON con la estructura: `{"code": "ERROR_CODE", "message": "Mensaje legible para el usuario"}`. Esto permite al frontend distinguir tipos de error y mostrar mensajes apropiados.

Crea una función helper en Rust `fn to_command_error(e: impl std::fmt::Display, code: &str) -> String` que serialice este formato.

### 7.2 `commands/auth_commands.rs`

**`cmd_start_auth() -> Result<DeviceFlowInfo, String>`**
Inicia el Device Flow. Retorna `user_code` y `verification_uri` para mostrarlos al usuario en la pantalla de login.

**`cmd_poll_auth(device_code: String, interval: u64) -> Result<AuthenticatedUser, String>`**
Hace una ronda de polling. El frontend llama esto en un loop con el interval indicado. Cuando retorna `Ok(user)`, el login fue exitoso. Si retorna error con code `"PENDING"`, el frontend sigue esperando.

**`cmd_get_current_user() -> Result<Option<AuthenticatedUser>, String>`**
Verifica si hay una sesión activa (token guardado en keyring). Retorna el usuario si está autenticado, `None` si no. Se llama al iniciar la app.

**`cmd_logout() -> Result<(), String>`**
Elimina el token del keyring y limpia el estado de la sesión.

### 7.3 `commands/config_commands.rs`

**`cmd_load_repo_config(repo_path: String) -> Result<GitGovConfig, String>`**
Lee y parsea el `gitgov.toml` del repo. Si no existe, retorna un error específico con instrucciones de cómo crear uno.

**`cmd_validate_repo(repo_path: String) -> Result<RepoValidation, String>`**
Verifica que: la ruta existe, es un repo de git válido, tiene remote `origin` configurado, y tiene `gitgov.toml`. Retorna un objeto con el resultado de cada verificación para que la UI muestre qué está bien y qué falta.

### 7.4 `commands/git_commands.rs`

**`cmd_get_status(repo_path: String) -> Result<Vec<FileChange>, String>`**
Obtiene los cambios del working tree.

**`cmd_get_file_diff(repo_path: String, file_path: String) -> Result<String, String>`**
Obtiene el diff unificado de un archivo específico.

**`cmd_stage_files(repo_path: String, files: Vec<String>, developer_login: String) -> Result<(), String>`**
Valida permisos de paths según `gitgov.toml`, hace stage de los archivos permitidos, registra en audit log. Si algún archivo no está permitido para el grupo del usuario, lo excluye y retorna advertencia (no error total).

**`cmd_unstage_all(repo_path: String) -> Result<(), String>`**
Limpia el staging area.

**`cmd_commit(repo_path: String, message: String, developer_login: String) -> Result<String, String>`**
Valida que el mensaje de commit sea Conventional Commits, crea el commit. Registra en audit log. Retorna el hash del commit.

**`cmd_push(repo_path: String, branch: String, developer_login: String) -> Result<(), String>`**
Este es el comando más crítico. Lógica en orden:
1. Carga el config del repo
2. Valida que la rama no sea protegida y que el usuario tenga permiso
3. Si alguna validación falla, registra en audit log como `BlockedPush` y retorna error con el motivo
4. Ejecuta el push con el token del keyring
5. Registra en audit log como `Push` exitoso
6. Retorna éxito

### 7.5 `commands/branch_commands.rs`

**`cmd_list_branches(repo_path: String) -> Result<Vec<BranchInfo>, String>`**
Lista todas las ramas.

**`cmd_create_branch(repo_path: String, name: String, from_branch: String, developer_login: String) -> Result<(), String>`**
Lógica:
1. Valida la nomenclatura del nombre
2. Valida que el grupo del usuario puede crear ese tipo de rama
3. Crea la rama
4. Registra en audit log
5. Si algún paso falla por permisos, registra como `BlockedBranch`

**`cmd_checkout_branch(repo_path: String, name: String) -> Result<(), String>`**
Cambia de rama. No requiere validación de permisos (solo es lectura local).

### 7.6 `commands/audit_commands.rs`

**`cmd_get_audit_logs(filter: AuditFilter) -> Result<Vec<AuditLogEntry>, String>`**
Solo accesible para admins (la validación de rol ocurre aquí en Rust, no en el frontend).

**`cmd_get_audit_stats() -> Result<AuditStats, String>`**
Estadísticas para el dashboard del admin.

**`cmd_get_my_logs(developer_login: String, limit: usize) -> Result<Vec<AuditLogEntry>, String>`**
Logs propios del developer. Cualquier usuario puede ver sus propios logs.

---

## FASE 8 — Estado global del frontend (Zustand)

### 8.1 `store/useAuthStore.ts`

Define el store de autenticación con el estado:
- `user`: `AuthenticatedUser | null`
- `isLoading`: boolean
- `authStep`: enum `'idle' | 'waiting_device' | 'polling' | 'authenticated'`
- `deviceFlowInfo`: `DeviceFlowInfo | null`

Acciones:
- `startAuth()`: llama `cmd_start_auth`, actualiza estado a `waiting_device`
- `pollAuth()`: llama `cmd_poll_auth` en loop con el interval correcto
- `checkExistingSession()`: llama `cmd_get_current_user` al iniciar la app
- `logout()`: llama `cmd_logout`, limpia el estado

### 8.2 `store/useRepoStore.ts`

Estado:
- `repoPath`: `string | null`
- `config`: `GitGovConfig | null`
- `currentBranch`: `string | null`
- `branches`: `BranchInfo[]`
- `fileChanges`: `FileChange[]`
- `selectedFiles`: `Set<string>` — archivos seleccionados para staging
- `stagedFiles`: `Set<string>` — archivos ya en staging
- `isLoadingStatus`: boolean
- `activeDiffFile`: `string | null`
- `activeDiff`: `string | null`

Acciones:
- `setRepoPath(path)`: establece el repo y carga config
- `refreshStatus()`: llama `cmd_get_status`
- `selectFile(path)` / `deselectFile(path)` / `selectAll()` / `deselectAll()`
- `stageSelected()`: llama `cmd_stage_files` con los seleccionados
- `loadDiff(filePath)`: llama `cmd_get_file_diff`
- `refreshBranches()`: llama `cmd_list_branches`
- `createBranch(name, from)`: llama `cmd_create_branch`
- `checkoutBranch(name)`: llama `cmd_checkout_branch`

### 8.3 `store/useAuditStore.ts`

Estado:
- `logs`: `AuditLogEntry[]`
- `stats`: `AuditStats | null`
- `filter`: `AuditFilter`
- `isLoading`: boolean

Acciones:
- `loadLogs()`: llama `cmd_get_audit_logs` con el filtro actual
- `loadStats()`: llama `cmd_get_audit_stats`
- `setFilter(filter)`: actualiza el filtro y recarga
- `loadMyLogs(login)`: carga los logs propios del developer

---

## FASE 9 — Componentes del frontend

### 9.1 Componentes shared

**`shared/Button.tsx`**
Variantes: `primary` (fondo brand), `secondary` (outline), `danger` (rojo), `ghost` (sin fondo). Props: `variant`, `size` (sm/md/lg), `disabled`, `loading` (muestra spinner), `onClick`, `children`. Siempre usa `type="button"` a menos que se especifique, para evitar submissions accidentales.

**`shared/Badge.tsx`**
Variantes: `success` (verde), `warning` (amarillo), `danger` (rojo), `neutral` (gris). Usado en el audit log para mostrar `Success`, `Blocked`, `Failed`.

**`shared/Modal.tsx`**
Modal accesible con `@headlessui/react`. Props: `isOpen`, `onClose`, `title`, `children`. Trampa el foco dentro del modal. Se cierra con Escape.

**`shared/Toast.tsx`**
Sistema de notificaciones. Las notificaciones se apilan en la esquina inferior derecha. Tipos: `success`, `error`, `warning`, `info`. Se auto-descartan a los 5 segundos. Los errores no se auto-descartan.

**`shared/Spinner.tsx`**
Spinner animado con CSS puro (sin librería). Props: `size` (sm/md/lg), `color`.

### 9.2 `auth/LoginScreen.tsx`

Pantalla completa que cubre la app cuando no hay sesión. Flujo:

1. **Estado inicial:** Botón "Conectar con GitHub". Al clickar, llama `startAuth()` del store.
2. **Estado `waiting_device`:** Muestra el `user_code` en tipografía grande y clara. Muestra el link `verification_uri`. Botón "Abrir GitHub" que abre el link en el browser del sistema. Texto explicativo: "Ve a GitHub, ingresa este código y autoriza GitGov."
3. **Estado `polling`:** Spinner + "Esperando autorización...". Contador de segundos restantes antes de expirar.
4. **Error:** Si expira o el usuario deniega, muestra el error y permite reintentar.

### 9.3 `repo/RepoSelector.tsx`

Panel que permite al usuario seleccionar la carpeta del repositorio local. Usa Tauri's dialog API para abrir un selector de carpetas nativo. Llama `cmd_validate_repo` y muestra el estado de cada verificación (ícono verde/rojo para: "Es un repo Git", "Tiene remote origin", "Tiene gitgov.toml").

### 9.4 `diff/FileList.tsx`

Lista de archivos con cambios. Cada fila muestra:
- Checkbox para seleccionar el archivo
- Ícono de estado (M para modified, A para added, D para deleted) con color diferenciado
- Ruta del archivo, con el directorio en gris y el nombre en blanco/negro
- Badge indicando si está en staging o no
- Botón "Ver diff" que carga el diff de ese archivo

Soporte para "Select All" / "Deselect All". Cuando un archivo está seleccionado y pertenece a un path no permitido para el grupo del usuario, se muestra un warning icon con tooltip explicativo.

### 9.5 `diff/DiffViewer.tsx`

Usa `react-diff-view` para renderizar el diff unificado. Configuración:
- Vista unificada (no split) por defecto, con opción de cambiar
- Resaltado de sintaxis básico
- Los hunks se muestran claramente separados
- El componente recibe el diff como string y usa `parseDiff` de `react-diff-view` para procesarlo

### 9.6 `branch/BranchSelector.tsx`

Dropdown con búsqueda para seleccionar la rama activa. Muestra:
- La rama actual destacada con un punto verde
- Ramas locales agrupadas separadas de remotas
- Input de búsqueda para filtrar por nombre
- Botón "Nueva rama" que abre `BranchCreator`

### 9.7 `branch/BranchCreator.tsx`

Modal para crear una nueva rama. Contiene:
- Input del nombre de la rama
- Indicador en tiempo real de si el nombre es válido (valida contra los patrones del config mientras el usuario escribe — esta validación es solo visual/client-side; la definitiva ocurre en Rust)
- Selector de "rama base" (de qué rama se crea)
- Muestra los patrones válidos como ayuda: "Ejemplos válidos: feat/TICKET-descripcion, fix/TICKET-descripcion"
- Botón "Crear rama" que llama `cmd_create_branch`

### 9.8 `commit/CommitPanel.tsx`

Panel fijo en la parte inferior del dashboard. Contiene:
- Textarea para el mensaje de commit con validación visual de Conventional Commits
- Dropdown de tipo de commit (feat, fix, docs, etc.) que auto-prefija el mensaje
- Contador de archivos en staging
- Botón "Commit" (deshabilitado si staging está vacío o mensaje inválido)
- Botón "Push" (deshabilitado si no hay commits que pushear)
- Los dos botones son secuenciales: primero se hace commit, después se habilita push

### 9.9 `audit/AuditLogView.tsx`

Vista completa del audit log para admins. Contiene:
- Filtros en la parte superior: rango de fechas, dropdown de developer, dropdown de action, dropdown de status
- Tabla de logs con paginación
- Panel de estadísticas en la parte superior: número de pushes hoy, bloqueados hoy, devs activos

### 9.10 `audit/AuditLogRow.tsx`

Fila de la tabla de audit log. Muestra:
- Timestamp formateado con `date-fns` (relativo para recientes: "hace 5 minutos", absoluto para antiguos)
- Avatar del developer (desde `avatar_url`)
- Nombre y login del developer
- Badge de action (`Push`, `BranchCreate`, `BlockedPush`, etc.)
- Nombre de la rama
- Número de archivos involucrados, con tooltip que lista los paths
- Badge de status (`Success` en verde, `Blocked` en rojo, `Failed` en amarillo)
- Si fue bloqueado, la razón se muestra expandible al clickar la fila

---

## FASE 10 — Páginas

### 10.1 `pages/DashboardPage.tsx`

La pantalla principal para developers. Layout:
- **Columna izquierda (30% del ancho):** `BranchSelector` en la parte superior, luego `FileList` con todos los cambios.
- **Columna derecha (70% del ancho):** `DiffViewer` arriba mostrando el diff del archivo seleccionado, `CommitPanel` abajo fijo.
- **Header superior:** Muestra el repo activo, la rama actual, avatar + nombre del usuario, y botón de logout.

Al cargar, llama `refreshStatus()` y `refreshBranches()` del store. Configura un intervalo de auto-refresh del status cada 30 segundos (para detectar cambios nuevos en archivos).

### 10.2 `pages/AuditPage.tsx`

Solo accesible si `user.is_admin === true`. Si un dev no admin intenta acceder, redirige al dashboard. Renderiza `AuditLogView` con todos los filtros y estadísticas.

### 10.3 `pages/SettingsPage.tsx`

Página de configuración básica:
- Muestra el `gitgov.toml` actual en modo lectura (con resaltado de sintaxis básico)
- Botón para cambiar el repositorio activo
- Información de la sesión actual
- Botón de logout

---

## FASE 11 — Layout y routing

### 11.1 `components/layout/Sidebar.tsx`

Sidebar izquierdo con navegación:
- Ícono de repo activo
- Link a Dashboard
- Link a Audit Log (solo visible si `is_admin`)
- Link a Settings
- En la parte inferior: avatar del usuario y botón logout

### 11.2 `components/layout/MainLayout.tsx`

Componente wrapper que incluye `Sidebar` y renderiza el `children` en el área principal. Si `user === null`, renderiza `LoginScreen` en su lugar. Si `repoPath === null`, renderiza `RepoSelector` en su lugar.

### 11.3 `router.tsx`

Define las rutas con `react-router-dom`:
- `/` → `DashboardPage`
- `/audit` → `AuditPage` (protegida por rol)
- `/settings` → `SettingsPage`

---

## FASE 12 — Archivo `gitgov.toml` de ejemplo

Crea un `gitgov.toml` de ejemplo en la raíz del proyecto que el agente documente bien con comentarios en formato TOML. Debe tener:

- Sección `[branches]` con `patterns` que incluyan ejemplos de feat, fix, hotfix, release, y `protected` con main, develop, staging.
- Sección `[groups.frontend]` con members de ejemplo y `allowed_branches` y `allowed_paths` acotados al directorio de frontend.
- Sección `[groups.backend]` con su propio scope.
- Sección `[groups.devops]` con acceso más amplio.
- Sección `[admins]` con una lista de GitHub usernames que tienen acceso de admin.

---

## FASE 13 — Inicialización de la app (main.rs y App.tsx)

### 13.1 `src-tauri/src/main.rs`

En `main.rs`:
1. Inicializa la base de datos SQLite. La ruta de la DB es `{app_data_dir}/gitgov/audit.db`, donde `app_data_dir` es el directorio de datos de la app según el OS (Tauri provee esto).
2. Guarda la conexión a la DB como estado de Tauri (`tauri::State`).
3. Registra todos los comandos de las fases anteriores en `invoke_handler`.
4. Configura la ventana inicial: 1200x800, título "GitGov", sin barra de título nativa en macOS (usa titleBarStyle hiddenInset para look nativo), con fondo de la app.

### 13.2 `src/App.tsx`

Al montar:
1. Llama `checkExistingSession()` del auth store.
2. Mientras verifica, muestra un splash screen simple con el logo de GitGov y un spinner.
3. Si hay sesión, renderiza `MainLayout` con el router.
4. Si no hay sesión, `MainLayout` se encarga de mostrar `LoginScreen`.

---

## FASE 14 — Validaciones y casos edge críticos

El agente debe implementar estas situaciones específicas:

**Si el usuario no tiene un grupo en `gitgov.toml`:**
Al intentar hacer cualquier operación de push o crear rama, recibe el mensaje: "Tu usuario ({login}) no está asignado a ningún grupo. Contacta al administrador." El log registra el intento.

**Si `gitgov.toml` no existe en el repo:**
La app muestra un panel de bienvenida con instrucciones de cómo crear el archivo. No bloquea completamente la app, pero sí bloquea las operaciones de push y creación de ramas.

**Si el token de GitHub expira:**
Al recibir un error 401 de la API, la app cierra la sesión automáticamente, elimina el token del keyring, y muestra la pantalla de login con el mensaje "Tu sesión expiró. Por favor vuelve a conectarte."

**Si hay conflicto al hacer push:**
El error de git2 se traduce a un mensaje claro: "No se pudo hacer push porque la rama remota tiene cambios que no tienes localmente. Haz pull primero." No se registra como `BlockedPush` (es un conflicto técnico, no un bloqueo de permisos). Se registra como `Failed`.

**Si el staging está vacío al intentar commitear:**
El botón de commit debe estar deshabilitado, pero si se llama el comando de todas formas, retorna error: "No hay archivos en el staging area. Selecciona al menos un archivo antes de commitear."

**Si un developer intenta ver el Audit Log:**
El comando `cmd_get_audit_logs` en Rust verifica el `is_admin` del usuario autenticado. Si no es admin, retorna error con code `"UNAUTHORIZED"` aunque el frontend ya debería haber ocultado el link.

---

## FASE 15 — Performance y buenas prácticas adicionales

**En Rust:**
- Todas las operaciones de I/O (git, red, DB) deben ser `async`. Usa `tokio::spawn` para operaciones largas si es necesario.
- Los comandos Tauri que pueden tardar (push, fetch de la API) deben retornar inmediatamente un estado "loading" usando eventos Tauri (`tauri::Window::emit`) para actualizar la UI progresivamente. Implementa eventos: `push:start`, `push:progress`, `push:done`, `push:error`.
- Abre la conexión SQLite una sola vez al iniciar la app y reutilizala (no abras una conexión por query).

**En React:**
- Usa `React.memo` en `AuditLogRow` y `FileList` row para evitar re-renders innecesarios cuando la lista tiene muchos elementos.
- El `DiffViewer` es costoso de renderizar. Usa `useMemo` para el resultado de `parseDiff` y solo re-parsea cuando el diff string cambia.
- El auto-refresh de status cada 30 segundos debe limpiar su intervalo en el `useEffect` cleanup para evitar memory leaks.
- Usa `useCallback` en todos los handlers que se pasan a componentes hijos.

**General:**
- Define todos los tipos TypeScript en `src/lib/types.ts` y que coincidan exactamente con las estructuras Rust serializadas.
- Usa `const` y nunca `let` a menos que la variable necesite ser reasignada.
- Los componentes React deben ser functional components. Sin class components.
- Cada componente en su propio archivo. Sin barrel exports que causen circular dependencies.

---

## FASE 16 — Orden de implementación recomendado para el agente

Implementa en este orden exacto para poder probar cada parte antes de construir la siguiente:

1. Modelos de datos Rust (Fase 2)
2. Config loader y validator (Fase 3)
3. Audit DB (Fase 4)
4. Git operations en Rust (Fase 5)
5. GitHub auth en Rust (Fase 6)
6. GitHub API en Rust (Fase 6)
7. Todos los comandos Tauri (Fase 7)
8. main.rs con todo registrado (Fase 13 parcial)
9. Verificar que `cargo build` pasa sin errores
10. Setup frontend (dependencias, Tailwind, router)
11. Stores de Zustand (Fase 8)
12. Componentes shared (Fase 9.1)
13. LoginScreen (Fase 9.2)
14. App.tsx y MainLayout (Fases 13.2 y 11.2)
15. RepoSelector (Fase 9.3)
16. FileList y DiffViewer (Fases 9.4 y 9.5)
17. BranchSelector y BranchCreator (Fases 9.6 y 9.7)
18. CommitPanel (Fase 9.8)
19. DashboardPage (Fase 10.1)
20. AuditLogView y AuditLogRow (Fases 9.9 y 9.10)
21. AuditPage y SettingsPage (Fases 10.2 y 10.3)
22. Sidebar y routing completo (Fases 11.1 y 11.3)
23. Validaciones de edge cases (Fase 14)
24. Optimizaciones de performance (Fase 15)
25. `gitgov.toml` de ejemplo documentado (Fase 12)

---

## Notas finales para el agente

- **No uses `unwrap()` en producción.** Cada `unwrap()` es un crash potencial. Usa `?`, `map_err`, o manejo explícito de errores.
- **No expongas el token en logs.** Si logueas errores de red, asegúrate de que el token no aparezca en el mensaje de error.
- **El `gitgov.toml` es la ley.** Si hay conflicto entre lo que el usuario quiere hacer y lo que el config dice, el config gana siempre.
- **El audit log es inmutable.** Nunca implementes una función de borrar o editar logs. Solo inserción y lectura.
- **Cuando hagas push, primero valida, luego actúa.** Nunca hagas el push y luego valides. La validación va antes de cualquier operación irreversible.
- **Si no estás seguro del comportamiento esperado en algún edge case, pregunta antes de implementar.** Es mejor pausar y clarificar que construir algo que hay que deshacer.
