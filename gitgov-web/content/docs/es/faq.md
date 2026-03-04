---
title: Preguntas Frecuentes
description: Preguntas frecuentes sobre GitGov — qué hace, qué no hace y cómo funciona.
order: 8
---

## General

### ¿Qué es GitGov?

GitGov es una **plataforma de gobernanza de Git** que captura metadatos operacionales de las estaciones de trabajo (commits, pushes, ramas) y los envía a un Control Plane central para auditoría, cumplimiento y monitoreo de políticas. No lee, analiza ni transmite código fuente.

### ¿Para quién es GitGov?

Para **equipos de ingeniería y organizaciones** que necesitan:

- Pistas de auditoría para cumplimiento regulatorio (SOC 2, ISO 27001, PCI-DSS).
- Visibilidad sobre operaciones de desarrollo sin inspeccionar código.
- Conciencia de políticas (ramas protegidas, horarios laborales).
- Trazabilidad CI/CD vinculando commits con pipelines Jenkins y tickets Jira.
- Gobernanza de equipo y organización en múltiples repositorios.

### ¿GitGov es open source?

GitGov es desarrollado por **Yohandry Chirinos**, ingeniero de software venezolano con más de 10 años de experiencia en entornos empresariales, banca, fintech y startups. GitGov no es open source — surge como producto de software para mejorar la trazabilidad operacional.

### ¿Qué stack tecnológico usa GitGov?

- **Desktop App**: Tauri v2 + React 19 + Tailwind v4 + Zustand v5 — app nativa para Windows, macOS y Linux.
- **Control Plane Server**: Axum (Rust) — API REST de alto rendimiento para ingesta de eventos, verificación de políticas y dashboard admin.
- **Base de datos**: PostgreSQL (Supabase o self-hosted) — almacena eventos de auditoría, datos de organización e integraciones.
- **Web App**: Next.js 14 — sitio de marketing y documentación en git-gov.vercel.app.

---

## Qué GitGov NO Hace

### ¿GitGov lee mi código fuente?

**No.** Solo captura metadatos: tipo de evento, SHA del commit, nombre de rama, autor, timestamp, rutas de archivos (hasta 500) y nombre del repositorio. Código fuente, contenido de archivos, diffs y mensajes de commit **nunca se transmiten**.

### ¿GitGov monitoriza mi pantalla, teclado o aplicaciones?

**No.** Solo observa operaciones Git. No tiene acceso a pantalla, portapapeles, navegador, IDE ni aplicaciones fuera de Git.

### ¿GitGov analiza calidad de código o ejecuta análisis estático?

**No.** Captura metadatos sobre *cuándo* y *dónde* ocurren eventos Git, no *qué* contiene el código.

### ¿GitGov reemplaza CI/CD?

**No.** Se integra con herramientas CI/CD (Jenkins, GitHub Actions) para correlacionar commits con resultados de pipelines. No ejecuta builds, tests ni despliegues.

### ¿GitGov bloquea operaciones Git?

**No.** Es una herramienta de **detección y observabilidad**. Puede señalar que ocurrió un push a rama protegida, pero no lo impide.

### ¿GitGov toma decisiones de RRHH o disciplinarias?

**No.** Las señales son **observaciones consultivas**. La organización es plenamente responsable de cualquier decisión.

### ¿GitGov perfila la productividad individual?

**No.** No hay "líneas de código", "puntuaciones de frecuencia" ni rankings de productividad.

### ¿GitGov vende o comparte mis datos?

**No.** Los datos pertenecen a la organización. Sin monetización ni compartición con terceros.

### ¿GitGov almacena contraseñas o secretos?

**No.** Las API keys se hashean (SHA-256). Nunca se recopilan contraseñas, tokens ni valores de `.env`.

### ¿GitGov recopila información de mi computadora?

**No.** Sin hardware, red, software instalado ni telemetría del sistema. Solo la versión del cliente.

---

## Datos y Seguridad

### ¿Dónde se almacenan mis datos?

En una **base de datos PostgreSQL** controlada por la organización (Supabase o self-hosted). La app de escritorio usa un **outbox JSONL** para eventos offline y una **base de datos SQLite** separada para logs de auditoría locales. Ver [Política de Seguridad](/docs/security).

### ¿Mis datos están cifrados?

**Sí, en múltiples capas:**

- **En tránsito:** TLS (HTTPS) entre Desktop y Control Plane.
- **En reposo:** AES-256 en bases de datos Supabase; API keys como hashes SHA-256.
- **En la estación:** API keys en keyring del SO (Windows DPAPI, macOS Keychain, Linux Secret Service).

### ¿Se pueden modificar o eliminar los registros de auditoría?

**No.** Son **append-only** por diseño. Triggers de PostgreSQL lo aplican a nivel de base de datos. Cada exportación se registra como evento de auditoría.

### ¿Quién puede ver mis eventos?

- Los **Developers** solo ven sus propios eventos.
- Los **Admins** ven todos los eventos de su organización.
- Los datos están delimitados por organización — admins de una org no ven otra.

### ¿Cómo se protegen las API keys?

Hasheadas con SHA-256. Texto plano mostrado una sola vez al crear. En desktop, en keyring del SO. La revocación surte efecto inmediato.

### ¿Cuánto tiempo se retienen mis datos?

Datos de auditoría: mínimo **1.825 días (5 años)**. Configurable al alza. Datos de sesión con retención separada más corta.

### ¿Puedo exportar mis datos?

**Sí.** `POST /export` proporciona JSON legible por máquina. Cada exportación queda registrada (cadena de custodia).

### ¿GitGov soporta el derecho al olvido (RGPD)?

**Sí.** Endpoint de anonimización/eliminación dentro del scope de la org. Devuelve 404 para usuarios inexistentes (privacy-preserving).

---

## Aplicación de Escritorio

### ¿Qué plataformas soporta?

**Windows**, **macOS** y **Linux** — construido con Tauri.

### ¿Qué pasa si pierdo la conexión a internet?

Los eventos se encolan en un **outbox JSONL local** con reintentos exponenciales. No se pierde ningún evento.

### ¿Requiere privilegios de administrador?

No. Se instala en espacio de usuario.

### ¿Qué captura la Desktop App?

1. **Stage files** — rutas de archivos preparados para commit (sin contenido).
2. **Commit** — SHA, rama, conteo de archivos, estado.
3. **Push** — rama destino, éxito/fallo, razón de bloqueo.
4. **Operaciones de rama** — creación, checkout.

Cada evento lleva un `event_uuid` único para deduplicación.

### ¿Puedo poner un PIN de bloqueo?

**Sí.** PIN opcional de 4-6 dígitos en Configuración. Protección local — separada de la autenticación del servidor.

### ¿Cómo actualizo la app?

Configuración > Actualizaciones: selecciona canal (Stable/Beta), busca actualizaciones, descarga e instala. Changelog disponible por versión.

### ¿Cómo configuro las políticas de gobernanza?

Define un archivo `gitgov.toml` en la raíz de tu repositorio. Ver [Gobernanza y Políticas](/docs/governance).

---

## Organizaciones y Equipos

### ¿Cómo creo una organización?

Un admin la crea desde el panel de Configuración de la Desktop App. Todos los datos (eventos, miembros, keys, integraciones) quedan delimitados a esa org.

### ¿Cómo agrego developers a mi organización?

Dos métodos:

1. **Provisión directa** — Admin ingresa login, email y rol. Miembro creado al instante.
2. **Invitación** — Admin genera token con rol y fecha de expiración. El developer acepta desde la Desktop App, creando su cuenta y API key automáticamente.

### ¿Qué roles están disponibles?

| Rol | Acceso |
|-----|--------|
| **Admin** | Completo: eventos, stats, dashboard, integraciones, equipo, API keys, políticas |
| **Developer** | Solo sus propios eventos |
| **Architect** | Igual que Developer (reservado para permisos granulares futuros) |
| **PM** | Igual que Developer (reservado para acceso futuro a reportes) |

### ¿Puedo deshabilitar un miembro sin eliminarlo?

**Sí.** Cambiar estado a **disabled**. API keys dejan de funcionar inmediatamente. Datos históricos preservados.

### ¿Cómo funcionan los tokens de invitación?

Generados por admins, hasheados (SHA-256) antes del almacenamiento, con expiración configurable. Una vez aceptados, se consumen y no se reutilizan. Se pueden reenviar o revocar.

### ¿Cómo gestiono las API keys de mi equipo?

Configuración > API Keys: listar, crear (con rol), revocar (efecto inmediato), o emitir para un miembro específico. Texto plano mostrado una sola vez.

---

## Dashboard y Analítica

### ¿Qué muestra el Dashboard de Admin?

- **Métricas** — Total eventos, tasa de éxito, repos activos, pushes, eventos bloqueados, developers activos.
- **Pipeline Health** — Métricas Jenkins 7 días: total builds, tasa de éxito, fallos, duración promedio.
- **Actividad Diaria** — Tendencias de commits/pushes (14 días).
- **Cobertura de Tickets** — Porcentaje de commits vinculados a tickets Jira.
- **Desglose de Eventos** — Distribución por tipo/estado, commits sin ticket, tickets sin commits.
- **Tabla de Commits Recientes** — Eventos paginados con badges CI, badges PR y links a tickets Jira.
- **Chat de Gobernanza** — Asistente IA integrado.

### ¿Cada cuánto se refresca?

Cada **30 segundos** por defecto. Se puede activar/desactivar.

### ¿Qué significan los badges CI?

- **Verde** — Pipeline exitoso.
- **Rojo** — Pipeline fallido.
- **Amarillo** — Inestable/abortado.
- **Sin badge** — Sin correlación Jenkins.

### ¿Qué significan los badges de ticket?

Referencias Jira detectadas en ramas o metadatos de commit (ej. `PROJ-123`). Click para ver detalle: estado, asignado, ramas/commits/PRs relacionados.

### ¿Qué ve un Developer?

Sus propios commits/pushes y opción de aceptar invitaciones. Sin stats de org ni gestión de equipo.

### ¿Qué timezone usa el Dashboard?

Almacenado en **UTC**. Timezone de visualización configurable en Configuración (12 zonas IANA). Solo afecta la vista — datos en UTC.

---

## Chat de Gobernanza (Asistente IA)

### ¿Qué puedo preguntarle?

- **Analítica**: "¿Quién hizo push a main sin ticket esta semana?", "¿Cuántos commits tiene alice?"
- **Configuración**: "¿Cómo configuro Jenkins?", "¿Cómo protejo una rama?"
- **Troubleshooting**: "¿Por qué recibo 401?", "¿Por qué mi dashboard está vacío?"
- **Producto**: "¿Qué integraciones existen?", "¿Qué roles hay?"

### ¿El chat accede a mi código?

**No.** Solo metadatos de eventos y conocimiento del producto. Nunca código fuente, diffs ni contenido de archivos.

### ¿Qué pasa si no sabe la respuesta?

Indica datos insuficientes u ofrece registrar un **feature request** para el equipo de producto.

---

## Control Plane y Servidor

### ¿Qué es el Control Plane?

El servidor central **Axum (Rust)** — recibe eventos, procesa webhooks, ejecuta verificaciones de política, sirve el dashboard.

### ¿Puedo self-hostear el Control Plane?

**Sí.** Cualquier servidor que ejecute binarios Rust + PostgreSQL. Ver [Conectar al Control Plane](/docs/control-plane).

### ¿Qué integraciones están soportadas?

- **GitHub** — Webhooks push/ramas, validación HMAC, streaming de audit log.
- **Jenkins** — Ingesta de pipelines, correlación commit-pipeline, widget de salud, verificación advisoria.
- **Jira** — Ingesta de tickets, correlación batch, reportes de cobertura, detalle de ticket.

### ¿Qué pasa si el servidor se cae?

Los clientes desktop siguen funcionando. Los eventos se encolan localmente y se sincronizan al volver.

### ¿Cómo funciona el rate limiting?

Configurable por endpoint. Defaults: 240 req/min (eventos), 60 req/min (admin), 120 req/min (Jenkins/Jira). Respuesta 429 = aumentar límites via variables de entorno.

---

## Cumplimiento

### ¿GitGov ayuda con SOC 2?

**Sí.** Pistas de auditoría append-only, RBAC, registros inmutables, capacidades de exportación — controles clave para SOC 2 Tipo II.

### ¿GitGov ayuda con RGPD?

**Sí.** Minimización de datos, derecho de acceso, portabilidad (exportación), derecho al olvido, distinción responsable/encargado. Ver [Privacidad y Responsabilidad de Señales](/docs/privacy).

### ¿GitGov ayuda con ISO 27001?

**Sí.** Pista de auditoría append-only, RBAC, almacenamiento cifrado, exportaciones soportan controles del Anexo A.

### ¿Qué es una "señal"?

Un marcador automatizado de posible desviación de política (ej. push no autorizado a rama protegida, commit sin ticket). **Solo consultiva** — requiere revisión humana. Ver [Privacidad y Responsabilidad de Señales](/docs/privacy).

### ¿Puedo revisar y descartar señales?

**Sí.** Los admins confirman, escalan o descartan. Cada decisión queda registrada con actor, razón y timestamp.

---

## Relacionado

- [**Política de Seguridad**](/docs/security) — Cifrado, almacenamiento, controles de acceso.
- [**Privacidad y Responsabilidad de Señales**](/docs/privacy) — Límites legales y RGPD.
- [**Política de Privacidad**](/privacy) — Términos legales para usuarios finales.
- [**Introducción**](/docs/introduction) — Primeros pasos con GitGov.
