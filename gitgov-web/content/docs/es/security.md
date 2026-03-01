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
| Tipo de evento | `commit`, `push` | Sí |
| SHA del commit | `a3f8c2e` | Sí |
| Nombre de rama | `feat/auth` | Sí |
| Autor Git | `alice` | Sí |
| Timestamp | ISO 8601 | Sí |
| Conteo de archivos | `12` | Sí |
| Nombre del repo | `org/repo` | Sí |
| Versión del cliente | `0.1.0` | Sí |
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

La aplicación de escritorio almacena un **outbox local SQLite** para resiliencia offline. Los eventos se encolan localmente y se sincronizan con el servidor cuando se restablece la conectividad.

| Almacenamiento Local | Propósito | Protección |
|---------------------|-----------|------------|
| Outbox (SQLite) | Eventos pendientes de sincronizar | Sistema de archivos local, limpiado tras entrega exitosa |
| API key | Autenticación con el servidor | Almacenada en keyring del SO (nunca en archivos de texto plano) |
| `gitgov.toml` | Configuración de políticas | Commiteado en el repositorio — sin secretos |

---

## Cifrado

### En Tránsito

Toda comunicación entre GitGov Desktop y el Control Plane está protegida por **TLS (HTTPS)** en entornos de producción.

- HTTP solo se soporta para **desarrollo local** (`127.0.0.1:3000`).
- Los despliegues en producción **deben** usar HTTPS con un certificado válido.
- Los payloads de webhooks de GitHub, Jenkins y Jira se validan con **firmas HMAC** o secretos de webhook dedicados antes del procesamiento.

### En Reposo

| Componente | Cifrado en Reposo |
|-----------|------------------|
| PostgreSQL (Supabase) | Cifrado de disco AES-256 (por defecto en Supabase) |
| Backups de base de datos | Cifrados en reposo según política de Supabase/proveedor |
| API keys en base de datos | Almacenadas como **hashes SHA-256** — texto plano nunca se persiste tras emisión inicial |
| Outbox local (SQLite) | Protegido por permisos del sistema de archivos del SO |
| Keyring del SO (API key) | Protegido por almacenamiento de credenciales del SO (Windows DPAPI, macOS Keychain, Linux Secret Service) |

---

## Quién Puede Acceder a Qué

GitGov aplica **control de acceso basado en roles (RBAC)** a nivel de API. Cada solicitud requiere un token `Authorization: Bearer` válido.

| Rol | Eventos Propios | Todos los Eventos | Stats/Dashboard | Integraciones | Gestión API Keys |
|-----|----------------|-------------------|----------------|-------------|-----------------|
| **Developer** | Lectura | — | — | — | — |
| **Architect** | Lectura | — | — | — | — |
| **PM** | Lectura | — | — | — | — |
| **Admin** | Lectura | Lectura | Lectura | Lectura/Escritura | Crear |

### Detalles del Control de Acceso

- Los **Developers** solo pueden ver sus propios registros de eventos (`GET /logs` está filtrado por `user_login`).
- Los **Admins** tienen visibilidad completa: todos los eventos, estadísticas, dashboard, integraciones, señales de cumplimiento y creación de API keys.
- **No existe bypass de superusuario** — el rol Admin es el nivel de privilegio más alto, y sigue sujeto a autenticación.
- Las API keys se hashean (SHA-256) antes del almacenamiento. El servidor nunca almacena ni loguea claves en texto plano.

---

## Integridad de la Pista de Auditoría

Los registros de eventos son **append-only**. El sistema está arquitectónicamente diseñado para prevenir manipulaciones:

- **Sin UPDATE** — los registros de eventos de auditoría no pueden modificarse vía API.
- **Sin DELETE** — los registros de eventos de auditoría no pueden eliminarse vía API.
- **Deduplicación** — cada evento lleva un `event_uuid` único; los duplicados se rechazan.
- **Logging de exportación** — cada exportación de datos (`POST /export`) se registra a su vez como un evento de auditoría, creando una cadena de custodia inmutable.

Este diseño soporta frameworks de cumplimiento incluyendo **SOC 2**, **ISO 27001** y los requisitos de pista de auditoría de **PCI-DSS**.

---

## Seguridad de Autenticación

| Mecanismo | Detalles |
|-----------|---------|
| **Hashing de API Key** | SHA-256 — el servidor calcula el hash del token bearer y busca por `key_hash` |
| **Almacenamiento de clave** | Keyring del SO en Desktop; columna hasheada en PostgreSQL en el servidor |
| **Firma JWT** | `GITGOV_JWT_SECRET` — debe ser un secreto fuerte y único en producción (`openssl rand -hex 32`) |
| **Validación de webhooks** | GitHub: HMAC-SHA256; Jenkins/Jira: secretos dedicados vía headers |
| **Rate limiting** | Límites de tasa configurables por ruta (eventos: 240/min, admin: 60/min) |

---

## Seguridad de Red

- El Control Plane escucha en `0.0.0.0:3000` por defecto (configurable vía `GITGOV_SERVER_ADDR`).
- El desarrollo local usa `127.0.0.1:3000` (solo loopback) para prevenir exposición accidental.
- Los despliegues en producción deben colocarse detrás de un **proxy inverso** (p.ej., Nginx, Caddy) con terminación TLS.
- Se aplican límites de CORS y tamaño de cuerpo de solicitud por endpoint de integración.

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
