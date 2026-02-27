# GitGov - Arquitectura del Sistema

## Qué es GitGov

GitGov es un sistema de gobernanza de Git distribuido que permite auditar y controlar las operaciones de Git en una organización. Funciona como un "testigo" que observa y registra todo lo que hacen los desarrolladores con sus repositorios.

El sistema está compuesto por **cuatro componentes** que trabajan juntas: una aplicación de escritorio que usan los desarrolladores, un servidor central que recopila datos, la plataforma GitHub donde viven los repositorios, y un sitio web de marketing/documentación público.

---

## Visión General de la Arquitectura

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                ARQUITECTURA GITGOV                                │
├──────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌─────────────────┐   ┌─────────────────┐   ┌───────────────┐  ┌─────────────┐ │
│  │   DESKTOP APP   │   │ CONTROL PLANE   │   │    GITHUB     │  │  WEB APP    │ │
│  │    (Tauri v2)   │   │    SERVER       │   │               │  │ (Next.js 14)│ │
│  │                 │   │    (Axum)       │   │               │  │             │ │
│  │  React 19       │   │  ┌───────────┐  │   │  ┌─────────┐  │  │  Marketing  │ │
│  │  Zustand v5     │HTTP│  │  Handlers │  │   │  │   API   │  │  │  /docs      │ │
│  │  Tailwind v4    │◄──►│  │  Auth     │  │Wh.│  │  OAuth  │  │  │  /download  │ │
│  │  react-router v7│   │  │  Jobs     │  │◄─►│  │  Repos  │  │  │  /pricing   │ │
│  │  ─────────────  │   │  └───────────┘  │   │  └─────────┘  │  │  i18n EN/ES │ │
│  │  Rust + git2    │   │  ┌───────────┐  │   │               │  │  Vercel     │ │
│  │  Outbox (JSONL) │   │  │ PostgreSQL│  │   │               │  │             │ │
│  │  SQLite local   │   │  │ Supabase  │  │   │               │  │             │ │
│  │  tauri-updater  │   │  │ Job Queue │  │   │               │  │             │ │
│  └─────────────────┘   │  └───────────┘  │   └───────────────┘  └─────────────┘ │
│                         └─────────────────┘                                      │
│                         EC2: 3.143.150.199                                       │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## Los Cuatro Componentes

### 1. Desktop App (Aplicación de Escritorio)

**Qué hace:** Es la aplicación que usa cada desarrollador en su computadora. Se conecta a GitHub, permite hacer operaciones Git, y registra todo lo que pasa.

**Tecnologías:**
- Frontend: React 19 + TypeScript + Tailwind CSS v4
- Estado global: Zustand v5
- Routing: react-router-dom v7
- Backend: Rust con el framework Tauri v2
- Comunicación con Git: librería git2
- Almacenamiento local: SQLite para auditoría offline (`{data_local_dir}/gitgov/audit.db`)
- Credenciales: usa el keyring del sistema operativo
- Actualizaciones: tauri-plugin-updater v2 (OTA, aún sin servidor de releases configurado)
- Tests: vitest 4 + @testing-library/react 16

**Responsabilidades principales:**

| Módulo | Qué hace |
|--------|----------|
| Operaciones Git | Ejecuta push, commit, stage, diff, merge |
| Outbox | Cola de eventos JSONL que funciona sin internet (flush cada 60s) |
| Autenticación | Maneja el login con GitHub OAuth |
| Ramas | Crea y valida ramas según políticas |
| Auditoría local | Guarda eventos en SQLite cuando no hay conexión |
| Ignore rules | Gestiona .gitignore / .gitgovignore / git info/exclude (`cmd_apply_ignore_rules`) |
| Stage limiter | `stage_files` events se truncan a 500 archivos máximo (`MAX_STAGE_FILES_EVENT_LIST`) |

**Constantes importantes del cliente:**

| Constante | Valor | Propósito |
|-----------|-------|-----------|
| `MAX_STAGE_FILES_EVENT_LIST` | 500 | Máx. archivos listados en un evento stage_files |
| Outbox flush interval | 60s | Worker de background |
| HTTP connect timeout | 5s | Tiempo máximo para conectar al servidor |
| HTTP request timeout | 30s | Tiempo máximo para una request completa |
| HTTP pool idle timeout | 90s | Keep-alive de conexiones HTTP |
| TCP keepalive | 30s | Ping TCP para detectar conexiones muertas |

**Cómo fluyen los datos:**

```
Usuario hace click → React UI → Comando Tauri → Lógica Rust → Operación Git
                                                    ↓
                                              Crear evento
                                                    ↓
                                              Guardar en Outbox
                                                    ↓
                                        Enviar al servidor (cuando haya conexión)
```

### 2. Control Plane Server (Servidor Central)

**Qué hace:** Es el cerebro del sistema. Recibe eventos de todas las desktop apps, los almacena, y proporciona dashboards para ver qué está pasando en la organización.

**Tecnologías:**
- Framework: Axum (basado en Tokio para async)
- Base de datos: PostgreSQL (hosteado en Supabase; pooler de conexiones)
- Autenticación: hash SHA256 de API keys + roles (Admin, Architect, Developer, PM)
- Procesamiento: jobs en background con reintentos (`FOR UPDATE SKIP LOCKED`)
- Deploy: Ubuntu 22.04 + Nginx + systemd en EC2 (`3.143.150.199`) o Docker

**Job Worker — constantes hardcoded:**

| Constante | Valor | Descripción |
|-----------|-------|-------------|
| `JOB_WORKER_TTL_SECS` | 300 | Job se considera atascado si lleva más de 5 min en RUNNING |
| `JOB_POLL_INTERVAL_SECS` | 5 | El worker revisa la cola cada 5 segundos |
| `JOB_ERROR_BACKOFF_SECS` | 10 | Espera 10s antes de reintentar tras error |

**Responsabilidades principales:**

| Módulo | Qué hace |
|--------|----------|
| Handlers | Recibe requests HTTP y devuelve respuestas |
| Auth | Verifica que quien hace el request está autorizado |
| Database | Guarda y lee datos de PostgreSQL |
| Jobs | Procesa tareas en background (detección de anomalías) |

**Endpoints principales:**

| Endpoint | Auth | Para qué sirve |
|----------|------|----------------|
| `/health` | None | Health check básico |
| `/health/detailed` | None | Latencia DB + uptime |
| `/events` | Bearer | Ingesta de eventos del cliente |
| `/webhooks/github` | HMAC | Webhooks de GitHub |
| `/audit-stream/github` | Bearer (admin) | Batch de audit logs de GitHub |
| `/stats` | Bearer (admin) | Estadísticas para el dashboard |
| `/logs` | Bearer | Eventos combinados (admin=todos, dev=propios) |
| `/dashboard` | Bearer (admin) | Datos agregados |
| `/jobs/metrics` | Bearer (admin) | Estado del job queue |
| `/jobs/dead` | Bearer (admin) | Jobs muertos |
| `/jobs/retry/{id}` | Bearer (admin) | Reintentar job muerto |
| `/governance-events` | Bearer (admin) | Cambios de políticas GitHub |
| `/signals` | Bearer (admin) | Señales de no-cumplimiento |
| `/violations` | Bearer (admin) | Violaciones detectadas |
| `/policy/check` | Bearer | Advisory de política (no bloqueante) |
| `/compliance` | Bearer (admin) | Estado de compliance |
| `/export` | Bearer (admin) | Export de audit data |
| `/api-keys` | Bearer (admin) | Gestión de API keys |
| `/integrations/jenkins` | Bearer | Ingesta de pipeline events |
| `/integrations/jenkins/status` | Bearer (admin) | Health check Jenkins |
| `/integrations/jenkins/correlations` | Bearer (admin) | Correlaciones commit↔pipeline |
| `/integrations/jira` | Bearer | Ingesta de issues Jira |
| `/integrations/jira/status` | Bearer (admin) | Health check Jira |
| `/integrations/jira/correlate` | Bearer (admin) | Correlación batch commit↔ticket |
| `/integrations/jira/ticket-coverage` | Bearer (admin) | Cobertura de tickets |
| `/integrations/jira/tickets/{id}` | Bearer (admin) | Detalle de ticket |

**Headers de integración:**
- Jenkins: `x-gitgov-jenkins-secret` (si `JENKINS_WEBHOOK_SECRET` configurado)
- Jira: `x-gitgov-jira-secret` (si `JIRA_WEBHOOK_SECRET` configurado)

**Schema versionado:** La DB se inicializa con 6 archivos SQL incrementales:
`supabase_schema.sql` → `v2` → `v3` → `v4` → `v5` (Jenkins) → `v6` (Jira)

### 3. GitHub Integration

**Qué hace:** GitGov se integra con GitHub de dos formas: para que los usuarios se autentiquen, y para recibir notificaciones cuando pasan cosas en los repositorios.

**Mecanismos de integración:**

| Mecanismo | Uso |
|-----------|-----|
| OAuth Device Flow | Login de usuarios desde la desktop app |
| Webhooks | Recibir eventos cuando alguien hace push |
| Audit Log Stream | Cambios de branch protection, rulesets, permisos → `governance_events` |
| API REST | Operaciones adicionales en repositorios |

---

### 4. Web App (gitgov-web) — Sitio Público

**Qué hace:** Es el sitio público de marketing y documentación, desplegado en Vercel. No tiene conexión directa con el Control Plane ni con la Desktop App.

**Directorio:** `gitgov-web/`

**URL:** `https://git-gov.vercel.app`

**Tecnologías:**
- Framework: Next.js 14 (App Router)
- Frontend: React 18 + TypeScript + Tailwind CSS v3
- Animaciones: framer-motion
- Documentación: gray-matter + remark (markdown)
- i18n: bilingual EN/ES via `lib/i18n/translations.ts` (context + hook)
- Package manager: pnpm
- Deploy: Vercel (CD automático desde main)

**Rutas:**

| Ruta | Propósito |
|------|-----------|
| `/` | Home / hero / what-is |
| `/features` | Features de la plataforma |
| `/download` | Descarga del installer Windows (.exe) |
| `/pricing` | Planes y precios |
| `/contact` | Formulario de contacto |
| `/docs` | Documentación (markdown) |
| `/docs/installation` | Guía de instalación |
| `/docs/control-plane` | Setup del servidor |

**Download page:**
- Componente `DownloadPage` es un Server Component de Next.js
- Calcula el SHA256 del installer en build time desde `public/downloads/GitGov_0.1.0_x64-setup.exe`
- Si el archivo no existe, muestra `downloadChecksum: 'sha256:pending-build'`
- Versión actual: `0.1.0`, archivo: `GitGov_0.1.0_x64-setup.exe`

**Diferencias importantes vs Desktop App:**

| Aspecto | Desktop App | Web App |
|---------|-------------|---------|
| React version | 19 | 18 |
| Tailwind version | v4 | v3 |
| Framework | Tauri v2 + Vite | Next.js 14 |
| Package manager | npm | pnpm |
| i18n | No | Sí (EN/ES) |
| Server URL config | `GITGOV_SERVER_URL` (Rust) / `VITE_SERVER_URL` (Vite) | N/A |

---

## Cómo Funciona la Autenticación

### Login con GitHub (Desktop → GitHub)

El usuario no ingresa usuario y contraseña en la app. En su lugar:

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│ Desktop  │     │ GitHub   │     │ Usuario  │
└────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │
     │ 1. Pide código │                │
     │───────────────►│                │
     │                │                │
     │ 2. Devuelve    │                │
     │    código      │                │
     │◄───────────────│                │
     │                │                │
     │ 3. Muestra     │                │
     │    código      │                │
     │────────────────┼───────────────►│
     │                │                │
     │                │ 4. Usuario    │
     │                │    va a       │
     │                │    github.com │
     │                │    /login/    │
     │                │    device     │
     │                │                │
     │                │ 5. Ingresa    │
     │                │    código     │
     │                │◄───────────────│
     │                │                │
     │ 6. Pregunta    │                │
     │    si ya       │                │
     │    autorizó    │                │
     │───────────────►│                │
     │                │                │
     │ 7. Recibe      │                │
     │    token       │                │
     │◄───────────────│                │
     │                │                │
     │ 8. Guarda en   │                │
     │    keyring     │                │
     │    (NUNCA en   │                │
     │    archivo)    │                │
```

### Autenticación Desktop → Control Plane

Cuando la desktop app quiere enviar eventos al servidor:

```
┌─────────────────────────────────────────────────────────────┐
│                   API KEY AUTHENTICATION                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Desktop                                                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Tiene una API key guardada en configuración         │   │
│  │ Ejemplo: 57f1ed59-371d-46ef-9fdf-508f59bc4963       │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ La envía en el header:                              │   │
│  │ Authorization: Bearer 57f1ed59-...                  │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
└─────────────────────────┼───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                      SERVER                                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Extrae el token del header                              │
│                                                             │
│  2. Calcula su hash SHA256                                  │
│     (el hash es lo que se guarda en la base de datos,       │
│      nunca la key original)                                 │
│                                                             │
│  3. Busca en la base de datos si existe una key             │
│     con ese hash y que esté activa                          │
│                                                             │
│  4. Si existe → Request autenticado                         │
│     Si no existe → Error 401 Unauthorized                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**IMPORTANTE:** El servidor SOLO acepta el header `Authorization: Bearer`, NO acepta `X-API-Key`.

**Roles del sistema:**

| Rol | Permisos |
|-----|----------|
| Admin | Acceso total — stats, dashboard, integrations, signals, violations, export, api-keys |
| Architect | Acceso a governance y compliance |
| Developer | Solo sus propios eventos (`/logs` filtrado por user_login) |
| PM | Acceso a vistas de tickets y cobertura |

**Bootstrap de API key en el servidor:**
- Si `GITGOV_API_KEY` está en el entorno → inserta en DB si no existe (sin imprimir en logs)
- Si NO está → genera UUID nuevo automáticamente
- La key generada solo se imprime si: la salida es una TTY interactiva, O se usó el flag `--print-bootstrap-key`
- En Docker/CI (sin TTY) la key generada NO aparece en stdout. Usar `--print-bootstrap-key` explícitamente.

---

## El Patrón Outbox (Cola de Eventos)

### Por qué existe el Outbox

Los desarrolladores pueden estar trabajando sin conexión a internet. GitGov necesita registrar todo lo que hacen, incluso cuando no pueden enviar los datos al servidor central.

El Outbox es una cola local que:
1. Guarda cada evento en un archivo JSONL en el disco
2. Intenta enviar los eventos al servidor periódicamente
3. Reintenta automáticamente si falla el envío

### Arquitectura del Outbox

```
┌─────────────────────────────────────────────────────────────┐
│                      OUTBOX ARCHITECTURE                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │   Acción    │    │   Memoria   │    │   Disco     │     │
│  │   de Git    │───►│   (Queue)   │───►│   (JSONL)   │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
│                                               │             │
│                                               │ persistir   │
│                                               ▼             │
│                                        ~/.gitgov/           │
│                                        outbox.jsonl         │
│                                                             │
│  Worker en Background (cada 60 segundos)                     │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 1. Lee eventos pendientes (los que no se enviaron)   │   │
│  │ 2. Los agrupa en un batch                            │   │
│  │ 3. Hace POST /events al servidor                     │   │
│  │ 4. Si éxito → marca como enviados                    │   │
│  │ 5. Si error → incrementa contador de intentos        │   │
│  │ 6. Espera más tiempo antes de reintentar (backoff)   │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Lógica de Reintentos

El outbox no intenta enviar inmediatamente cuando falla. Espera cada vez más tiempo:

- Intento 1: inmediato
- Intento 2: espera 30 segundos
- Intento 3: espera 60 segundos
- Intento 4: espera 120 segundos
- Intento 5: espera 240 segundos
- Máximo: 5 intentos, después el evento se marca como "muerto"

---

## Flujo de Eventos

### Secuencia completa de un Push

Este es el camino que siguen los datos cuando un desarrollador hace push:

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│ Usuario  │     │ Desktop  │     │ Outbox   │     │ Server   │
└────┬─────┘     └────┬─────┘     └────┬─────┘     └────┬─────┘
     │                │                │                │
     │ git push       │                │                │
     │───────────────►│                │                │
     │                │                │                │
     │                │ 1. Registra    │                │
     │                │    "intento    │                │
     │                │    de push"    │                │
     │                │───────────────►│                │
     │                │                │                │
     │                │ 2. Valida si   │                │
     │                │    rama está   │                │
     │                │    protegida   │                │
     │                │                │                │
     │                │ 3. Ejecuta     │                │
     │                │    push real   │                │
     │                │────────────────┼───────────────►│
     │                │                │                │
     │                │ 4. Registra    │                │
     │                │    "push       │                │
     │                │    exitoso"    │                │
     │                │───────────────►│                │
     │                │                │                │
     │                │ 5. Dispara     │                │
     │                │    envío       │                │
     │                │───────────────►│                │
     │                │                │                │
     │                │                │ 6. POST al    │
     │                │                │    servidor   │
     │                │                │───────────────►│
     │                │                │                │
     │                │                │                │ 7. Guarda en
     │                │                │                │    PostgreSQL
     │                │                │                │
     │                │                │ 8. Confirma   │
     │                │                │◄───────────────│
     │                │                │                │
     │ OK             │                │                │
     │◄───────────────│                │                │
```

### Tipos de Eventos que se Registran

| Evento | Origen | Cuándo se genera |
|--------|--------|------------------|
| attempt_push | Desktop | Antes de cada push |
| successful_push | Desktop | Push completado sin errores |
| blocked_push | Desktop | Push a rama protegida (rechazado) |
| push_failed | Desktop | Push falló por error de Git |
| commit | Desktop | Se creó un commit |
| stage_files | Desktop | Se agregaron archivos al staging area (max 500 archivos) |
| create_branch | Desktop | Se creó una rama nueva |
| blocked_branch | Desktop | Creación de rama bloqueada por política |
| checkout_branch | Desktop | Cambio de rama |
| login | Desktop | Inicio de sesión del usuario |
| logout | Desktop | Cierre de sesión |
| push | GitHub | Webhook recibido por push |

---

## Modelo de Datos

### Entidades Principales

El sistema trabaja con estas entidades principales:

**Organizaciones (orgs)**
- Representa una empresa o equipo
- Tiene miembros y repositorios

**Repositorios (repos)**
- Pertenecen a una organización
- Tienen políticas de gobernanza

**Eventos de GitHub (github_events)**
- Llegan via webhooks
- Registra pushes, creaciones de rama, etc.
- Son append-only (nunca se modifican)

**Eventos del Cliente (client_events)**
- Llegan de las desktop apps
- Registra todo lo que hacen los desarrolladores
- Son append-only (nunca se modifican)

**Violaciones (violations)**
- Se generan cuando se detecta comportamiento sospechoso
- Pueden ser investigadas y resueltas

**API Keys (api_keys)**
- Permiten autenticar requests
- Se guardan hasheadas (nunca en texto plano)
- Tienen rol asociado: Admin, Architect, Developer, PM

**Pipeline Events (pipeline_events) — V1.2-A**
- Llegan de Jenkins vía `/integrations/jenkins`
- Registra builds, stages, resultados (append-only)
- Correlación con commits por `commit_sha`

**Project Tickets (project_tickets) — V1.2-B**
- Llegan de Jira vía `/integrations/jira`
- Snapshot de issues (estado, asignado, summary)
- Actualizable (campos `related_commits`, `related_branches`, `related_prs`)

**Commit Ticket Correlations (commit_ticket_correlations) — V1.2-B**
- Correlaciones entre commits y tickets Jira (append-only)
- Deduplicación por `(commit_sha, ticket_id)`
- Extraída de mensajes de commit y nombres de ramas

**Governance Events (governance_events)**
- Cambios de configuración de seguridad desde GitHub Audit Log
- Append-only. Incluye cambios de branch protection, rulesets, permisos

**Versioning del Schema:**

| Archivo | Contenido |
|---------|-----------|
| `supabase_schema.sql` | Schema base: orgs, repos, events, violations, jobs, api_keys |
| `supabase_schema_v2.sql` | Mejoras de índices y funciones |
| `supabase_schema_v3.sql` | Governance events, signals, decisions |
| `supabase_schema_v4.sql` | Append-only triggers y compliance signals |
| `supabase_schema_v5.sql` | Jenkins: `pipeline_events` + índices de correlación |
| `supabase_schema_v6.sql` | Jira: `project_tickets` + `commit_ticket_correlations` |

### Relaciones entre Entidades

```
Organización
    │
    ├─── tiene muchos ───► Repositorios
    │                           │
    │                           ├─── generan ──► Eventos GitHub
    │                           │
    │                           └─── generan ──► Eventos Cliente
    │
    └─── tiene muchos ───► Miembros
                                │
                                └─── usan ──► API Keys
```

---

## Seguridad

### Principios de Seguridad

1. **Tokens en keyring:** Los tokens de GitHub nunca se guardan en archivos, van al keyring del sistema operativo
2. **API keys hasheadas:** Solo se guarda el hash SHA256, nunca la key original
3. **HTTPS obligatorio:** En producción, toda comunicación es encriptada
4. **Append-only:** Los eventos de auditoría no se pueden modificar ni borrar
5. **Deduplicación:** Cada evento tiene un UUID único que previene duplicados

### Headers de Autenticación

| Tipo | Header | Uso |
|------|--------|-----|
| API Key | `Authorization: Bearer {key}` | Desktop → Server (TODOS los endpoints autenticados) |
| HMAC GitHub | `X-Hub-Signature-256: sha256={sig}` | GitHub → Server (webhooks) |
| Secret Jenkins | `x-gitgov-jenkins-secret: {secret}` | Jenkins → Server (si `JENKINS_WEBHOOK_SECRET` configurado) |
| Secret Jira | `x-gitgov-jira-secret: {secret}` | Jira → Server (si `JIRA_WEBHOOK_SECRET` configurado) |

### Validación de Webhooks

Cuando GitHub envía un webhook, incluye una firma criptográfica. El servidor:
1. Calcula la firma con el payload recibido y el secreto conocido
2. Compara con la firma que envió GitHub
3. Si no coinciden → rechaza el webhook

Esto previene que alguien envíe webhooks falsos.

### Rate Limiting

El servidor implementa un rate limiter en memoria por `{IP}:{SHA256(auth_header)[0:12]}`.

| Variable de entorno | Default | Descripción |
|---------------------|---------|-------------|
| `GITGOV_RATE_LIMIT_EVENTS_PER_MIN` | 240 | req/min para `/events` |
| `GITGOV_RATE_LIMIT_AUDIT_STREAM_PER_MIN` | 60 | req/min para `/audit-stream/github` |
| `GITGOV_RATE_LIMIT_JENKINS_PER_MIN` | 120 | req/min para `/integrations/jenkins` |
| `GITGOV_RATE_LIMIT_JIRA_PER_MIN` | 120 | req/min para `/integrations/jira` |
| `GITGOV_RATE_LIMIT_ADMIN_PER_MIN` | 60 | req/min para endpoints admin (logs, stats, dashboard) |
| `GITGOV_JENKINS_MAX_BODY_BYTES` | 262144 | Límite de body para Jenkins (256 KB) |
| `GITGOV_JIRA_MAX_BODY_BYTES` | 524288 | Límite de body para Jira (512 KB) |

Respuesta cuando se supera el límite: HTTP `429 Too Many Requests`.
La clave de rate limiting es `{IP}:{SHA256(auth_header)[0:12]}` — cada API key distinta tiene su propio bucket por IP.

---

## Observabilidad

### Logs Estructurados

El sistema usa logs estructurados con niveles:

| Nivel | Cuándo usar |
|-------|-------------|
| ERROR | Algo crítico falló |
| WARN | Algo inesperado pasó pero el sistema sigue funcionando |
| INFO | Eventos normales del sistema |
| DEBUG | Información detallada para debugging |

### Métricas Disponibles

| Métrica | Dónde verla | Qué significa |
|---------|-------------|---------------|
| Job Queue | /jobs/metrics | Cuántos jobs pendientes, corriendo, muertos |
| Stats | /stats | Contadores de eventos por tipo |
| Health | /health/detailed | Latencia de DB, uptime del servidor |

---

## Extensibilidad

### Agregar un Nuevo Tipo de Evento

Para agregar un nuevo tipo de evento:

1. **En Desktop:** Definir el nuevo tipo en el módulo de outbox
2. **En Server:** Agregar el tipo al enum de eventos
3. **En SQL:** Actualizar la función de eventos combinados si es necesario

### Agregar un Nuevo Endpoint

Para agregar un nuevo endpoint al servidor:

1. Crear el handler (función que procesa el request)
2. Agregar la ruta en la configuración del servidor
3. Decidir si requiere autenticación y/o rol de admin
4. Agregar tests

---

## Diagrama de Estados de un Job

Los jobs (tareas en background) pasan por varios estados:

```
                    ┌─────────────┐
                    │   PENDING   │  ← Job creado, esperando ser procesado
                    └──────┬──────┘
                           │
                    Worker lo toma
                           │
                           ▼
                    ┌─────────────┐
                    │   RUNNING   │  ← Job siendo ejecutado
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
         Éxito │       Error │      Timeout │
              │            │            │
              ▼            ▼            ▼
       ┌──────────┐ ┌──────────┐ ┌──────────┐
       │COMPLETED │ │  FAILED  │ │  Reset   │
       └──────────┘ └────┬─────┘ └────┬─────┘
                         │            │
                    ¿Reintentar?  Vuelve a
                         │        PENDING
                         ▼
              ┌──────────────────┐
              │ ¿Max intentos?   │
              └────────┬─────────┘
                       │
              Sí ──────┴────── No
               │              │
               ▼              ▼
        ┌──────────┐   Vuelve a
        │   DEAD   │   RUNNING
        └──────────┘
```

---

## Pruebas y Contratos

El servidor tiene tres capas de testing con distintos requisitos de infraestructura:

### Capa 1 — Unit tests (sin DB, sin server)

```bash
cd gitgov/gitgov-server
cargo test        # 36 tests, ~0.01s
make test         # equivalente con Makefile
```

Ubicación: bloques `#[cfg(test)]` en `src/models.rs`, `src/handlers.rs`, `src/auth.rs`.

| Suite | Qué valida |
|-------|-----------|
| `auth::tests` (5) | `require_admin`, `require_same_user_or_admin` con todos los roles |
| `handlers::tests` (10) | Validación HMAC GitHub, extracción de ticket IDs, audit delivery ID |
| `models::tests::pagination` (5) | `EventFilter` y `JenkinsCorrelationFilter` sin `offset`/`limit` → defaults correctos |
| `models::tests::golden_path` (6) | Payload de cada evento del Golden Path + `ClientEventResponse` shape |
| `models::tests::roundtrip` (10) | Enums `UserRole`, `ClientEventType`, `EventStatus`, `PipelineStatus`, `SignalType`, etc. |

Estos tests corren en CI en cada push/PR (job `server-lint`). Si fallan, se sube un artefacto con el output completo.

### Capa 2 — Smoke / contrato live (requiere server + DB)

```bash
cd gitgov/gitgov-server
make smoke
# o directamente:
SERVER_URL=http://127.0.0.1:3000 API_KEY=<key> bash tests/smoke_contract.sh
```

Script `tests/smoke_contract.sh` con 14 checks en dos secciones:

**Sección A — Paginación (8 checks):** verifica que los endpoints de lectura responden correctamente sin `offset`/`limit`, con `offset` explícito (compat), y sin ningún parámetro.

**Sección B — Golden Path (6 checks):** envía los 4 eventos del flujo principal al servidor real, verifica que aparecen en `/logs`, y confirma que un reenvío del mismo UUID aparece en `duplicates[]`.

Este script NO corre en CI (requiere DB Supabase real).

### Capa 3 — Integración completa (scripts E2E)

```bash
cd gitgov/gitgov-server/tests
bash e2e_flow_test.sh                                          # Golden Path base
API_KEY=<key> bash jenkins_integration_test.sh                # V1.2-A Jenkins
API_KEY=<key> bash jira_integration_test.sh                   # V1.2-B Jira
```

Scripts más completos que ejercitan ingesta, deduplicación, correlaciones y autenticación.

### Defaults de paginación (tras corrección Feb 2026)

Todos los endpoints de lectura paginada aceptan `offset` y `limit` como opcionales:

| Endpoint | `limit` default | max |
|----------|----------------|-----|
| `/logs` | 100 | 500 |
| `/integrations/jenkins/correlations` | 20 | — |
| `/signals` | 100 | — |
| `/governance-events` | 100 | — |

Si `offset` no se envía, equivale a `offset=0`.

---

## Próximos Pasos

Para más información:

- **Guía rápida:** [QUICKSTART.md](./QUICKSTART.md)
- **Solución de problemas:** [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- **Para agentes de IA:** [AGENTS.md](../AGENTS.md)
