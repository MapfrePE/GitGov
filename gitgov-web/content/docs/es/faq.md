---
title: Preguntas Frecuentes
description: Preguntas frecuentes sobre GitGov — qué hace, qué no hace y cómo funciona.
order: 8
---

## General

### ¿Qué es GitGov?

GitGov es una **plataforma de gobernanza de Git** que captura metadatos operacionales de las estaciones de trabajo de los desarrolladores (commits, pushes, ramas) y los envía a un Control Plane central para auditoría, cumplimiento y monitoreo de políticas. No lee, analiza ni transmite código fuente.

### ¿Para quién es GitGov?

GitGov está diseñado para **equipos de ingeniería y organizaciones** que necesitan:

- Pistas de auditoría para cumplimiento regulatorio (SOC 2, ISO 27001, PCI-DSS).
- Visibilidad sobre las operaciones de desarrollo sin inspeccionar código.
- Conciencia de aplicación de políticas (p.ej., reglas de ramas protegidas, horarios laborales).
- Trazabilidad de CI/CD vinculando commits con pipelines de Jenkins y tickets de Jira.

### ¿GitGov es open source?

GitGov es desarrollado por **Yohandry Chirinos**, un ingeniero de software venezolano con más de 10 años de experiencia en distintos entornos empresariales, banca, fintech y startups. GitGov no es open source, surge como un software producto para mejorar la trazabilidad de las empresas.

---

## Qué GitGov NO Hace

Esta es una de las secciones más importantes. Entender qué **no** hace GitGov es esencial tanto para desarrolladores como para tomadores de decisiones.

### ¿GitGov lee mi código fuente?

**No.** GitGov solo captura metadatos: tipo de evento, SHA del commit, nombre de rama, autor, timestamp, conteo de archivos y nombre del repositorio. El código fuente, contenido de archivos, diffs y cuerpos de mensajes de commit **nunca se transmiten** y nunca abandonan la estación de trabajo del desarrollador.

### ¿GitGov monitoriza mi pantalla, pulsaciones de tecla o aplicaciones?

**No.** GitGov solo observa operaciones Git (commit, push, creación de ramas). No tiene acceso a tu pantalla, portapapeles, navegador, IDE ni ninguna aplicación fuera de Git.

### ¿GitGov analiza la calidad del código o ejecuta análisis estático?

**No.** GitGov no hace lint, review ni evalúa la calidad de tu código. Captura metadatos sobre *cuándo* y *dónde* ocurren los eventos Git, no *qué* contiene el código.

### ¿GitGov reemplaza CI/CD?

**No.** GitGov se integra con herramientas de CI/CD (Jenkins, GitHub Actions) para correlacionar commits con resultados de pipelines. **No** ejecuta builds, tests ni despliegues.

### ¿GitGov bloquea operaciones Git?

**No.** GitGov es una herramienta de **detección y observabilidad**, no de enforcement. Puede señalar que ocurrió un push a una rama protegida, pero no impide que el push se realice.

### ¿GitGov toma decisiones de RRHH o disciplinarias?

**No.** Las señales generadas por GitGov son **observaciones consultivas** — indican que una regla de política se activó. Las señales no establecen intención, negligencia ni culpa. La organización desplegante es plenamente responsable de cualquier decisión tomada en base a señales.

### ¿GitGov perfila la productividad individual del desarrollador?

**No.** No hay "líneas de código por día", "puntuaciones de frecuencia de commits" ni rankings de productividad. GitGov es una herramienta de gobernanza y cumplimiento, no un rastreador de rendimiento.

### ¿GitGov vende o comparte mis datos?

**No.** Todos los datos pertenecen a la organización desplegante. GitGov no tiene modelo de monetización de datos. Los datos no se comparten con terceros.

### ¿GitGov almacena contraseñas o secretos?

**No.** Las API keys se hashean (SHA-256) antes del almacenamiento. GitGov nunca recopila, almacena ni transmite contraseñas, tokens ni valores de `.env` de tu estación de trabajo.

---

## Datos y Seguridad

### ¿Dónde se almacenan mis datos?

Los datos de eventos se almacenan en una **base de datos PostgreSQL** controlada por la organización desplegante (gestionada por Supabase o self-hosted). La app de escritorio mantiene un outbox local SQLite para resiliencia offline. Ver [Política de Seguridad](/docs/security) para detalles completos.

### ¿Mis datos están cifrados?

**Sí, en múltiples capas:**

- **En tránsito:** TLS (HTTPS) entre Desktop y Control Plane.
- **En reposo:** Cifrado de disco AES-256 en bases de datos gestionadas por Supabase; API keys almacenadas como hashes SHA-256.
- **En la estación de trabajo:** API keys almacenadas en el keyring del SO (Windows DPAPI, macOS Keychain, Linux Secret Service).

### ¿Se pueden modificar o eliminar los registros de auditoría?

**No.** Los registros de eventos de auditoría son **append-only** por diseño. La API no expone operaciones UPDATE ni DELETE sobre las tablas de eventos. Cada exportación de datos se registra a su vez como un evento de auditoría.

### ¿Quién puede ver mis eventos?

El acceso se controla mediante **control de acceso basado en roles (RBAC)**:

- Los **Developers** solo ven sus propios eventos.
- Los **Admins** ven todos los eventos, estadísticas y datos del dashboard.
- No existe forma de que un usuario con rol Developer acceda a los eventos de otro desarrollador.

### ¿Cómo se protegen las API keys?

Las API keys se hashean con SHA-256 antes de almacenarse en la base de datos. La clave en texto plano solo se muestra una vez en el momento de la creación y nunca se persiste en el servidor. En el escritorio, las claves se almacenan en el gestor de credenciales del SO (keyring).

---

## Aplicación de Escritorio

### ¿Qué plataformas soporta GitGov Desktop?

GitGov Desktop está construido con **Tauri** y soporta **Windows**, **macOS** y **Linux**.

### ¿Qué pasa si pierdo la conexión a internet?

Los eventos se encolan en un **outbox local SQLite** y se sincronizan automáticamente cuando se restablece la conectividad. No se pierden eventos.

### ¿GitGov requiere privilegios de administrador para instalarse?

No. GitGov Desktop se instala en el espacio de usuario y no requiere permisos elevados en la mayoría de sistemas.

### ¿Cómo configuro las políticas de gobernanza?

Las políticas se definen en un archivo `gitgov.toml` en la raíz de tu repositorio. Ver [Gobernanza y Políticas](/docs/governance) para la referencia completa de configuración.

---

## Control Plane y Servidor

### ¿Qué es el Control Plane?

El Control Plane es el servidor central **Axum (Rust)** que recibe eventos de los clientes de escritorio, procesa webhooks de GitHub/Jenkins/Jira, ejecuta verificaciones de política y sirve el dashboard de administración.

### ¿Puedo self-hostear el Control Plane?

**Sí.** El Control Plane puede desplegarse en cualquier servidor que ejecute binarios Rust. Requiere una base de datos PostgreSQL. Ver [Conectar al Control Plane](/docs/control-plane) para instrucciones de configuración.

### ¿Qué integraciones están soportadas?

- **GitHub** — webhooks para eventos de push y ramas, streaming de audit log.
- **Jenkins** — ingesta de eventos de pipeline y correlación commit-a-pipeline.
- **Jira** — ingesta de tickets, correlación commit-a-ticket y reportes de cobertura.

---

## Cumplimiento

### ¿GitGov ayuda con el cumplimiento SOC 2?

**Sí.** GitGov proporciona pistas de auditoría append-only, control de acceso basado en roles y registros de eventos inmutables — todos controles clave para compromisos SOC 2 Tipo II.

### ¿GitGov ayuda con el cumplimiento RGPD?

GitGov se diseña con principios RGPD:

- **Minimización de datos** — solo metadatos, nunca código fuente.
- **Derecho de acceso** — los desarrolladores pueden ver sus propios eventos.
- **Portabilidad** — `POST /export` proporciona exportación de datos legible por máquina.
- **Distinción responsable/encargado** — la organización desplegante es el responsable; GitGov es el encargado.

Ver [Privacidad y Responsabilidad de Señales](/docs/privacy) para la referencia RGPD completa.

---

## Relacionado

- [**Política de Seguridad**](/docs/security) — Detalles técnicos completos sobre cifrado, almacenamiento y acceso.
- [**Privacidad y Responsabilidad de Señales**](/docs/privacy) — Límites legales y RGPD.
- [**Política de Privacidad**](/privacy) — Términos legales para usuarios finales.
- [**Introducción**](/docs/introduction) — Primeros pasos con GitGov.
