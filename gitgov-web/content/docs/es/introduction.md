---
title: Introducción a GitGov
description: Domina la gobernanza distribuida de Git y establece trazabilidad operativa completa en todo tu ciclo de desarrollo.
order: 1
---

GitGov es un **Sistema de Gobernanza de Git Distribuido de Grado Empresarial**. Está diseñado específicamente para equipos de ingeniería con altos requisitos de seguridad que necesitan evidencia operativa inmutable, trazabilidad profunda y aplicación automatizada de cumplimiento en cada commit, push y despliegue.

## El Problema: Pistas de Auditoría Fragmentadas

En las organizaciones de ingeniería modernas, la "fuente de verdad" está dispersa en una docena de silos desconectados:
- **Repositorios Git**: Donde ocurre el desarrollo.
- **Pipelines CI/CD**: (Jenkins, GitHub Actions) Donde se ejecutan los builds.
- **Sistemas de Tickets**: (Jira) Donde se definen los requisitos.
- **Máquinas de los Desarrolladores**: Donde el código es realmente escrito y manipulado.

Cuando ocurre una auditoría o se investiga un incidente de seguridad, los equipos a menudo luchan por responder: *"¿Quién autorizó que este código evitara el servidor de build y llegara a producción?"* Tradicionalmente, esto se reconstruye a partir de logs dispersos que pueden estar incompletos o ya rotados.

## La Solución: Gobernanza en el Origen

GitGov invierte el modelo. En lugar de depender de servidores centrales para inferir lo que ocurrió en la máquina de un desarrollador, GitGov captura metadatos de alta fidelidad **en el punto de origen** — la estación de trabajo del desarrollador.

Al correlacionar operaciones Git locales con resultados de builds y datos de tickets, GitGov construye una **cadena de custodia unificada** para cada línea de código en tu organización.

---

## Pilares Fundamentales de la Plataforma

### 1. Evidencia Operativa Inmutable
Cada acción — commit, push, stage, rebase, merge — se registra como un evento discreto y append-only. Los eventos se deduplicitan por UUID único y se almacenan en una tabla de auditoría append-only a prueba de manipulaciones. Los registros nunca se sobreescriben ni se eliminan.

### 2. Trazabilidad Profunda
GitGov no solo ve un "commit." Ve un commit vinculado a un ticket específico de Jira, validado por un build específico de Jenkins, enviado desde una estación de trabajo verificada — todo correlacionado automáticamente por el Control Plane.

### 3. Aplicación Progresiva de Políticas
- **Protección de Ramas**: Definida en `gitgov.toml`, evita pushes directos no autorizados a ramas protegidas (p.ej. `main`, `release/*`).
- **Acceso Basado en Grupos**: Restringe qué equipos pueden hacer push a qué ramas y modificar qué rutas de código.
- **Verificaciones Advisorías CI**: El endpoint `/policy/check` permite a Jenkins y otros sistemas CI consultar el estado de cumplimiento antes de ejecutar un build.

---

## Arquitectura de Componentes

GitGov está compuesto por cuatro componentes misión-críticos:

| Componente | Responsabilidad | Stack Tecnológico |
|------------|-----------------|-------------------|
| **GitGov Desktop** | Captura local de eventos Git y feedback en tiempo real al desarrollador | Tauri v2, Rust, React 19 |
| **Control Plane** | Ingestión central de eventos, almacenamiento, reporting y motor de políticas | Rust, Axum, PostgreSQL |
| **Integraciones** | Correlación de datos de Jenkins, Jira y GitHub | Webhooks y REST APIs |
| **Web App** | Documentación, marketing y portal de descarga | Next.js 14, React 18 |

---

## Navegación y Próximos Pasos

¿Listo para empezar? Sigue la ruta a continuación para asegurar tu flujo de trabajo Git:

1. [**Instalar GitGov Desktop**](/docs/installation) — Pon el agente de captura en funcionamiento.
2. [**Conectar al Control Plane**](/docs/control-plane) — Vincula tu instancia local al servidor central.
3. [**Configurar Políticas**](/docs/governance) — Define las reglas que mantienen limpio tu repositorio.
4. [**Trazabilidad CI/CD**](/docs/ci-traces) — Conecta tus pipelines de Jenkins para proveniencia completa de builds.
