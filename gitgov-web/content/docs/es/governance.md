---
title: Gobernanza y Políticas
description: Defina y aplique políticas computacionales en toda su cadena de suministro de software.
order: 4
---

GitGov transforma Git de una simple herramienta de almacenamiento en una **plataforma gobernada**. Al definir políticas en el Control Plane, puede asegurar que cada desarrollador en su organización siga estándares idénticos de seguridad, calidad y trazabilidad.

---

## Modos Operativos de las Políticas

Las políticas de GitGov funcionan en dos modos distintos, permitiendo un despliegue progresivo dentro de su organización:

### 1. Modo Consultivo (No Bloqueante)
El agente de captura monitorea las acciones del desarrollador y proporciona feedback en tiempo real en la UI de Desktop sin bloquear los comandos de Git. Esto es ideal para enseñar mejores prácticas y recopilar métricas base de cumplimiento.

### 2. Modo de Aplicación (Bloqueante)
Las operaciones que violan una política definida son bloqueadas activamente en la estación de trabajo. Los desarrolladores reciben una explicación detallada de la violación e instrucciones sobre cómo remediarla.

---

## Dominios Críticos de Gobernanza

### Estándares de Mensajes de Commit
Asegure que cada commit esté vinculado a un propósito.
- **Validación por Regex**: Fuerza a que los mensajes sigan patrones como `[JIRA-123]: Descripción corta`.
- **Restricciones de Longitud**: Evita descripciones crípticas o excesivamente verbosas.
- **Palabras Clave**: Exige la presencia de palabras clave críticas (ej. `fix`, `feat`, `chore`).

### Convenciones de Nombres de Ramas
Mantenga una estructura de repositorio limpia y fácil de buscar.
- **Requisitos de Prefijo**: `feature/*`, `bugfix/*`, `hotfix/*`.
- **Etiquetas de Propietario**: Incluya identificadores de desarrollador o equipo en los nombres de las ramas.

---

## Definiendo una Política (Ejemplo)

Las políticas se almacenan por repositorio en un archivo `gitgov.toml`. Aquí un ejemplo para una rama de producción:

```toml
[policy]
name = "Standard Security Policy"
target_branches = ["main", "release/*"]

[[policy.rules]]
id = "commit_message_format"
pattern = "^(feat|fix|refactor|docs|test|chore): .+"
enforcement = "advisory"

[[policy.rules]]
id = "branch_naming"
pattern = "^(feat|fix|hotfix|release)/.+"
enforcement = "advisory"

[[policy.rules]]
id = "max_diff_size"
limit_lines = 500
enforcement = "advisory"
```

> [!NOTE]
> **Advisory primero**: Todas las reglas actualmente operan en modo consultivo. El modo de aplicación (bloqueante en la estación de trabajo) está en el roadmap. Usa el modo consultivo ahora para recopilar métricas de cumplimiento base antes de endurecer la política.

---

## Mejores Prácticas para el Despliegue

1. **Fase 1 (Observación)**: Despliegue la app Desktop con todas las políticas en **Modo Consultivo**. Use el dashboard del Control Plane para identificar violaciones frecuentes.
2. **Fase 2 (Aplicación Selectiva)**: Cambie las reglas de alto riesgo (como la firma de commits) a **Modo de Aplicación**.
3. **Fase 3 (Gobernanza Total)**: Aplique la aplicación completa en todos los repositorios críticos.

## Siguiente Fase

- [**Trazabilidad CI/CD**](/docs/ci-traces) — Cierre la brecha entre commits y despliegues.
