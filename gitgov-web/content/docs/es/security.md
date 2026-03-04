---
title: Política de Seguridad
description: Qué captura GitGov, dónde se almacena, quién puede acceder y cómo se protegen tus datos en cada capa.
order: 7
---

GitGov se diseña con el principio de **mínimos datos, máxima protección**. Esta página detalla la postura de seguridad completa de la plataforma — qué entra al sistema, cómo se almacena, quién puede acceder y qué controles técnicos están en vigor.

---

## Qué Captura GitGov

GitGov Desktop captura **únicamente metadatos operacionales** — nunca código fuente, contenido de archivos, diffs, mensajes de commit, secretos ni valores de `.env`.

| Dato | Ejemplo | ¿Se captura? |
|------|---------|-------------|
| Tipo de evento | `commit`, `push`, `stage_files` | Sí |
| SHA del commit | `a3f8c2e` | Sí |
| Nombre de rama | `feat/auth` | Sí |
| Autor Git | `alice` | Sí |
| Timestamp | ISO 8601 (almacenado en UTC) | Sí |
| Conteo de archivos | `12` | Sí |
| Rutas de archivos | `src/main.rs` | Sí (limitado a 500) |
| Nombre del repo | `org/repo` | Sí |
| Versión del cliente | `0.1.0` | Sí |
| Estado del evento | `success`, `blocked`, `failed` | Sí |
| Razón de bloqueo | `protected branch` | Sí (cuando aplica) |
| **Código fuente** | — | **Nunca** |
| **Contenido de archivos** | — | **Nunca** |
| **Contenido de diffs** | — | **Nunca** |
| **Cuerpo del mensaje de commit** | — | **Nunca** |
| **Contraseñas / secretos** | — | **Nunca** |
| **Valores de .env** | — | **Nunca** |

> El código fuente nunca abandona la estación de trabajo del desarrollador. Esta es una garantía arquitectónica, no una opción de configuración.

---

## Dónde se Almacenan los Datos

### Servidor Control Plane

Los registros de eventos se almacenan en una **base de datos PostgreSQL** (alojada en Supabase en nuestro despliegue gestionado). La organización desplegante controla la instancia de base de datos y su ubicación geográfica.

| Capa | Tecnología | Detalles |
|------|-----------|---------|
| **Base de datos** | PostgreSQL 15+ | Gestionado por Supabase o self-hosted |
| **Conexión** | Pooler cifrado con TLS | Connection string usa `sslmode=require` |
| **Backups** | Backups diarios de Supabase | O política de backup de la organización |
| **Región** | Configurable | La organización selecciona la región al desplegar |

### Cliente Desktop

La aplicación de escritorio almacena un **outbox local en formato JSONL** para resiliencia offline. Los eventos se encolan localmente y se sincronizan con el servidor cuando se restablece la conectividad. Una **base de datos SQLite local** separada almacena el log de auditoría para verificación de políticas offline.

| Almacenamiento Local | Propósito | Protección |
|---------------------|-----------|------------|
| Outbox (JSONL) | Eventos pendientes de sincronizar | Sistema de archivos local, limpiado tras entrega exitosa |
| Audit DB (SQLite) | Log de auditoría local para verificación offline de políticas | Permisos del sistema de archivos del SO |
| API key | Autenticación con el servidor | Almacenada en keyring del SO (nunca en archivos de texto plano) |
| `gitgov.toml` | Configuración de políticas | Commiteado en el repositorio — sin secretos |

---

## Cifrado

### En Tránsito

Toda comunicación entre GitGov Desktop y el Control Plane está protegida por **TLS (HTTPS)** en entornos de producción.

- HTTP solo se soporta para **desarrollo local**.
- Los despliegues en producción **deben** usar HTTPS con un certificado válido.
- Los payloads de webhooks de GitHub, Jenkins y Jira se validan con **firmas HMAC** o secretos de webhook dedicados antes del procesamiento.
- Todas las solicitudes a la API requieren un token `Authorization: Bearer` válido — no es posible el acceso anónimo a endpoints protegidos.

### En Reposo

| Componente | Cifrado en Reposo |
|-----------|------------------|
| PostgreSQL (Supabase) | Cifrado de disco AES-256 (por defecto en Supabase) |
| Backups de base de datos | Cifrados en reposo según política de Supabase/proveedor |
| API keys en base de datos | Almacenadas como **hashes SHA-256** — texto plano nunca se persiste tras emisión inicial |
| Outbox local (JSONL) | Protegido por permisos del sistema de archivos del SO |
| Audit DB local (SQLite) | Protegido por permisos del sistema de archivos del SO |
| Keyring del SO (API key) | Protegido por almacenamiento de credenciales del SO (Windows DPAPI, macOS Keychain, Linux Secret Service) |

---

## Quién Puede Acceder a Qué

GitGov aplica **control de acceso basado en roles (RBAC)** a nivel de API. Cada solicitud requiere un token `Authorization: Bearer` válido.

| Rol | Eventos Propios | Todos los Eventos | Stats/Dashboard | Integraciones | Gestión API Keys | Gestión Equipo y Org |
|-----|----------------|-------------------|----------------|-------------|-----------------|---------------------|
| **Developer** | Lectura | — | — | — | — | — |
| **Architect** | Lectura | — | — | — | — | — |
| **PM** | Lectura | — | — | — | — | — |
| **Admin** | Lectura | Lectura | Lectura | Lectura/Escritura | Crear/Revocar | Completa |

### Detalles del Control de Acceso

- Los **Developers** solo pueden ver sus propios registros de eventos (`GET /logs` está filtrado por `user_login`). No pueden ver datos de otros developers, estadísticas ni información de integraciones.
- Los **Admins** tienen visibilidad completa: todos los eventos, estadísticas, dashboard, integraciones, señales de cumplimiento, gestión de API keys, vista de equipo y configuración de organización.
- **No existe bypass de superusuario** — el rol Admin es el nivel de privilegio más alto, y sigue sujeto a autenticación.
- Las API keys se hashean (SHA-256) antes del almacenamiento. El servidor nunca almacena ni loguea claves en texto plano.
- **Scoping por organización** — todos los datos están delimitados por organización. Un admin de una organización no puede ver datos de otra organización.

### Asignación de Roles

Los roles se asignan al momento de crear la API key o al provisionar un miembro en la organización. Roles disponibles:

- **Admin** — Acceso completo a todos los endpoints, integraciones, gestión de equipo y operaciones de API keys.
- **Architect** — Actualmente igual que Developer; reservado para permisos granulares futuros.
- **Developer** — Acceso de lectura solo a sus propios eventos.
- **PM** — Actualmente igual que Developer; reservado para acceso futuro centrado en reportes.

---

## Integridad de la Pista de Auditoría

Los registros de eventos son **append-only**. El sistema está arquitectónicamente diseñado para prevenir manipulaciones:

- **Sin UPDATE** — los registros de eventos de auditoría no pueden modificarse vía API.
- **Sin DELETE** — los registros de eventos de auditoría no pueden eliminarse vía API.
- **Triggers en base de datos** — PostgreSQL aplica append-only a nivel de base de datos (no solo a nivel de API).
- **Deduplicación** — cada evento lleva un `event_uuid` único; los duplicados se rechazan y se reportan al cliente.
- **Logging de exportación** — cada exportación de datos (`POST /export`) se registra a su vez como un evento de auditoría, creando una cadena de custodia inmutable.
- **Retención forzada** — los datos de auditoría tienen un piso mínimo de retención de **1.825 días (5 años)**, configurable al alza por la organización.

Este diseño soporta frameworks de cumplimiento incluyendo **SOC 2**, **ISO 27001** y los requisitos de pista de auditoría de **PCI-DSS**.

---

## Seguridad de Autenticación

| Mecanismo | Detalles |
|-----------|---------|
| **Hashing de API Key** | SHA-256 — el servidor calcula el hash del token bearer y busca por `key_hash` |
| **Almacenamiento de clave (desktop)** | Keyring del SO — Windows DPAPI, macOS Keychain, Linux Secret Service |
| **Almacenamiento de clave (servidor)** | Columna hasheada en PostgreSQL — texto plano nunca se persiste |
| **Ciclo de vida de claves** | Las claves pueden crearse, listarse y revocarse. La revocación surte efecto inmediato. |
| **Firma JWT** | `GITGOV_JWT_SECRET` — debe ser un secreto fuerte y único en producción |
| **Validación de webhooks** | GitHub: HMAC-SHA256 con `X-Hub-Signature-256`; Jenkins: `x-gitgov-jenkins-secret`; Jira: `x-gitgov-jira-secret` |
| **Rate limiting** | Límites de tasa configurables por ruta para prevenir abuso |
| **Tokens de invitación** | Hasheados (SHA-256) antes del almacenamiento; expiran tras un período configurable |

### Límites de Tasa por Defecto

| Ruta | Límite por Defecto |
|------|-------------------|
| Ingesta de eventos (`/events`) | 240 req/min |
| Audit stream (`/audit-stream/github`) | 60 req/min |
| Integración Jenkins | 120 req/min |
| Integración Jira | 120 req/min |
| Endpoints admin (logs, stats, dashboard) | 60 req/min |

Todos los límites son configurables mediante variables de entorno.

---

## Seguridad de Red

- El Control Plane escucha en una dirección y puerto configurables (mediante la variable de entorno `GITGOV_SERVER_ADDR`).
- El desarrollo local usa enlace solo a loopback para prevenir exposición accidental en la red.
- Los despliegues en producción deben colocarse detrás de un **proxy inverso** (p.ej., Nginx, Caddy) con terminación TLS.
- Se aplican límites de CORS y tamaño de cuerpo de solicitud por endpoint de integración.
- Los tamaños máximos de body son configurables por integración (Jenkins, Jira, audit stream) para prevenir abuso.

---

## Organización y Aislamiento de Datos

- Todos los datos están **delimitados por organización** — eventos, logs, integraciones, miembros de equipo y API keys pertenecen a una org específica.
- El acceso inter-organizacional es arquitectónicamente imposible: el middleware de autenticación aplica el scoping de org en cada solicitud.
- Las organizaciones son creadas por un admin y los miembros se agregan mediante **provisión directa** o **tokens de invitación**.
- Los tokens de invitación se hashean antes del almacenamiento y expiran automáticamente tras el período configurado.
- Cuando un miembro es deshabilitado, sus API keys dejan de funcionar inmediatamente en la siguiente solicitud.

---

## Qué GitGov NO Hace

Esta sección es crítica para establecer expectativas precisas:

| GitGov NO... | Explicación |
|-------------|-------------|
| **Lee tu código fuente** | Solo se capturan metadatos (SHA, rama, autor, timestamp, conteo de archivos). |
| **Analiza calidad de código** | No tiene funcionalidad de análisis estático, linting ni code review. |
| **Monitoriza pulsaciones de tecla o pantalla** | GitGov solo observa operaciones Git, no el comportamiento del desarrollador. |
| **Toma decisiones de RRHH** | Las señales son observaciones consultivas, no determinaciones disciplinarias. |
| **Reemplaza CI/CD** | GitGov traza pipelines de CI pero no ejecuta builds, tests ni despliegues. |
| **Aplica protección de ramas** | GitGov detecta violaciones de política; no bloquea operaciones Git. |
| **Almacena contraseñas o secretos** | Las API keys se hashean; nunca se recopilan contraseñas. |
| **Accede a repositorios privados** | GitGov no clona, hace fetch ni lee contenido de repositorios. |
| **Perfila productividad individual** | No hay puntuaciones de "líneas de código" ni "frecuencia de commits". |
| **Vende o comparte tus datos** | Los datos pertenecen a la organización desplegante. GitGov no monetiza datos. |
| **Recopila telemetría de tu máquina** | No hay perfilado de hardware, red ni SO más allá de lo que los metadatos de Git contienen. |

---

## Retención de Datos y Derecho al Olvido

- **Retención de auditoría** — Mínimo 1.825 días (5 años) para datos de eventos de auditoría. Configurable al alza por la organización.
- **Retención de sesiones** — Los datos operacionales de sesión tienen un período de retención más corto, configurable por separado.
- **Derecho al olvido** — GitGov soporta anonimización/eliminación de datos de un developer específico dentro del scope de la organización, conforme a RGPD/LOPD. El endpoint de borrado devuelve 404 para usuarios inexistentes (privacy-preserving: indistinguible de "usuario no encontrado").
- **Exportación** — `POST /export` permite a usuarios autorizados exportar eventos en formato legible por máquina. Cada exportación se registra como evento de auditoría.

---

## Respuesta a Incidentes

Si se descubre una vulnerabilidad de seguridad en GitGov:

1. Repórtala a **security@gitgov.io** con una descripción detallada.
2. No la divulgues públicamente hasta que haya un fix disponible.
3. Aspiramos a confirmar los reportes en **48 horas** y proporcionar un plan de remediación en **7 días hábiles**.

---

## Relacionado

- [**Privacidad y Responsabilidad de Señales**](/docs/privacy) — Límites legales y cumplimiento RGPD.
- [**Política de Privacidad**](/privacy) — Términos legales completos para usuarios finales.
- [**Gobernanza y Políticas**](/docs/governance) — Configurar reglas en `gitgov.toml`.
- [**Preguntas Frecuentes**](/docs/faq) — Preguntas comunes sobre datos, seguridad y cumplimiento.
