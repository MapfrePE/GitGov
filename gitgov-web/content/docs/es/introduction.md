---
title: Introducción a GitGov
description: Domine la gobernanza distribuida de Git y establezca trazabilidad operativa completa en todo su ciclo de vida de desarrollo.
order: 1
---

GitGov es un **Sistema de Gobernanza de Git Distribuido de Grado Empresarial**. Está diseñado específicamente para equipos de ingeniería conscientes de la seguridad que requieren evidencia operativa inmutable, trazabilidad criptográfica y cumplimiento automatizado en cada commit, push y despliegue.

## El Problema: Pistas de Auditoría Fragmentadas

En las organizaciones de ingeniería modernas, la "fuente de verdad" está dispersa en una docena de silos desconectados:
- **Repositorios Git**: Donde ocurre el desarrollo.
- **Pipelines de CI/CD**: (Jenkins, GitHub Actions) Donde ocurren las construcciones.
- **Sistemas de Tickets**: (Jira) Donde se definen los requerimientos.
- **Máquinas de Desarrolladores**: Donde el código se genera y manipula realmente.

Cuando ocurre una auditoría o se investiga un incidente de seguridad, los equipos suelen tener dificultades para responder: *"¿Quién autorizó que este código omitiera el servidor de construcción y llegara a producción?"*. Tradicionalmente, esto se reconstruye a partir de logs disparatados que podrían estar incompletos o ya rotados.

## La Solución: Gobernanza en el Punto de Origen

GitGov cambia el modelo. En lugar de depender de servidores centrales para adivinar qué sucedió en la máquina de un desarrollador, GitGov captura metadatos de alta fidelidad **en el punto de origen**.

Al correlacionar las operaciones locales de Git con los resultados de construcción ascendentes y los datos de tickets, GitGov construye una **cadena de custodia unificada** para cada byte de código en su organización.

---

## Pilares Fundamentales de la Plataforma

### 1. Evidencia Operativa Inmutable
Cada acción (commit, rebase, merge, push) se registra como un evento discreto e inmutable. Estos eventos se firman criptográficamente y se sincronizan con un Control Plane central, creando una pista de auditoría a prueba de manipulaciones.

### 2. Trazabilidad Profunda
GitGov no solo ve un "commit". Ve un commit vinculado a un ticket específico de Jira, validado por un build específico de Jenkins y enviado desde una estación de trabajo de desarrollador verificada.

### 3. Aplicación Progresiva de Políticas
- **Modo Consultivo**: Advierte a los desarrolladores sobre nombres de ramas o mensajes de commit no convencionales en tiempo real.
- **Modo de Aplicación**: Bloquea operaciones que no cumplen con los estándares organizacionales (por ejemplo, commits sin firmar o falta de IDs de tickets).

---

## Arquitectura de Componentes

GitGov se compone de tres capas críticas para la misión:

| Componente | Responsabilidad | Stack Tecnológico |
|-----------|----------------|------------------|
| **GitGov Desktop** | Captura local y feedback en tiempo real para el desarrollador | Tauri, Rust, React |
| **Control Plane** | Ingesta central de eventos, almacenamiento e informes | Rust, Axum, PostgreSQL |
| **Integraciones** | Correlación de datos de Jenkins, Jira y GitHub | Webhooks y APIs REST |

> [!TIP]
> **Consejo Pro**: Puede ejecutar GitGov en "Modo Silencioso" durante el despliegue inicial para recopilar datos de cumplimiento base sin interrumpir los flujos de trabajo de los desarrolladores.

## Navegación y Siguientes Pasos

¿Listo para comenzar? Siga el camino a continuación para asegurar su flujo de trabajo de Git:

1. [**Instalar GitGov Desktop**](/docs/installation) — Ejecute el agente de captura en su máquina.
2. [**Conectar al Control Plane**](/docs/control-plane) — Vincule su instancia local con el servidor central.
3. [**Configurar Políticas**](/docs/governance) — Defina las reglas que mantienen su código limpio.
