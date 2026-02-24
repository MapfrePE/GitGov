# GitGov - Arquitectura del Sistema

## Qué es GitGov

GitGov es un sistema de gobernanza de Git distribuido que permite auditar y controlar las operaciones de Git en una organización. Funciona como un "testigo" que observa y registra todo lo que hacen los desarrolladores con sus repositorios.

El sistema está compuesto por tres piezas que trabajan juntas: una aplicación de escritorio que usan los desarrolladores, un servidor central que recopila datos, y la plataforma GitHub donde viven los repositorios.

---

## Visión General de la Arquitectura

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

## Los Tres Componentes

### 1. Desktop App (Aplicación de Escritorio)

**Qué hace:** Es la aplicación que usa cada desarrollador en su computadora. Se conecta a GitHub, permite hacer operaciones Git, y registra todo lo que pasa.

**Tecnologías:**
- Frontend: React con TypeScript y Tailwind CSS
- Backend: Rust con el framework Tauri v2
- Comunicación con Git: librería git2
- Almacenamiento local: SQLite para auditoría offline
- Credenciales: usa el keyring del sistema operativo

**Responsabilidades principales:**

| Módulo | Qué hace |
|--------|----------|
| Operaciones Git | Ejecuta push, commit, stage, diff, merge |
| Outbox | Cola de eventos que funciona sin internet |
| Autenticación | Maneja el login con GitHub OAuth |
| Ramas | Crea y valida ramas según políticas |
| Auditoría local | Guarda eventos en SQLite cuando no hay conexión |

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
- Base de datos: PostgreSQL (hosteado en Supabase)
- Autenticación: hash SHA256 de API keys
- Procesamiento: jobs en background con reintentos

**Responsabilidades principales:**

| Módulo | Qué hace |
|--------|----------|
| Handlers | Recibe requests HTTP y devuelve respuestas |
| Auth | Verifica que quien hace el request está autorizado |
| Database | Guarda y lee datos de PostgreSQL |
| Jobs | Procesa tareas en background (detección de anomalías) |

**Endpoints principales:**

| Endpoint | Para qué sirve |
|----------|----------------|
| /health | Verificar que el servidor está vivo |
| /events | Recibir eventos de las desktop apps |
| /webhooks/github | Recibir notificaciones de GitHub |
| /stats | Estadísticas para el dashboard |
| /logs | Lista de eventos para auditoría |
| /dashboard | Datos agregados para visualización |

### 3. GitHub Integration

**Qué hace:** GitGov se integra con GitHub de dos formas: para que los usuarios se autentiquen, y para recibir notificaciones cuando pasan cosas en los repositorios.

**Mecanismos de integración:**

| Mecanismo | Uso |
|-----------|-----|
| OAuth Device Flow | Login de usuarios desde la desktop app |
| Webhooks | Recibir eventos cuando alguien hace push |
| API REST | Operaciones adicionales en repositorios |

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
| stage_files | Desktop | Se agregaron archivos al staging area |
| create_branch | Desktop | Se creó una rama nueva |
| blocked_branch | Desktop | Creación de rama bloqueada por política |
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
| API Key | `Authorization: Bearer {key}` | Desktop → Server |
| HMAC | `X-Hub-Signature-256: sha256={sig}` | GitHub → Server (webhooks) |

### Validación de Webhooks

Cuando GitHub envía un webhook, incluye una firma criptográfica. El servidor:
1. Calcula la firma con el payload recibido y el secreto conocido
2. Compara con la firma que envió GitHub
3. Si no coinciden → rechaza el webhook

Esto previene que alguien envíe webhooks falsos.

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

## Próximos Pasos

Para más información:

- **Guía rápida:** [QUICKSTART.md](./QUICKSTART.md)
- **Solución de problemas:** [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- **Para agentes de IA:** [AGENTS.md](../AGENTS.md)
