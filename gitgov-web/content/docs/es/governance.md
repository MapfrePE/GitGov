---
title: Gobernanza y Políticas
description: Define controles de acceso, protección de ramas y permisos basados en grupos en tus repositorios mediante gitgov.toml.
order: 4
---

GitGov transforma Git de una simple herramienta de almacenamiento en una **plataforma gobernada**. Al definir un archivo `gitgov.toml` por repositorio, puedes aplicar protección de ramas, control de acceso basado en grupos y verificaciones advisorías de políticas en cada estación de trabajo de desarrollador.

---

## Modos de Operación de Políticas

La gobernanza de GitGov opera en dos niveles:

### 1. Nivel de Estación de Trabajo (gitgov.toml)
El archivo `gitgov.toml` en la raíz del repositorio configura la protección de ramas y los permisos de grupos. Cuando un desarrollador intenta hacer push a una rama protegida sin la membresía de grupo requerida, la operación se bloquea y se registra un evento.

### 2. Verificación Advisoria CI (/policy/check)
El Control Plane expone un endpoint `/policy/check` que tu pipeline CI (p.ej. Jenkins) puede consultar antes de ejecutar un build. Devuelve una respuesta advisoria — `allowed`, `reasons` y `warnings` — para que puedas aplicar gobernanza a nivel de pipeline sin bloquear las estaciones de trabajo de los desarrolladores.

---

## Configurando gitgov.toml

Las políticas se almacenan por repositorio en un archivo `gitgov.toml` en la raíz del repositorio. El archivo admite tres secciones de configuración:

### [branches]
Define los patrones de nombre de rama reconocidos y la lista de ramas protegidas que bloquean los pushes directos.

### [groups.*]
Define equipos nombrados con sus miembros, las ramas a las que tienen permitido hacer push y las rutas de código que están autorizados a modificar.

### admins
Una lista de nombres de usuario con acceso administrativo a todas las ramas y rutas.

---

## Ejemplo de Configuración

```toml
# gitgov.toml — coloca en la raíz del repositorio

[branches]
# Convenciones de nomenclatura reconocidas (informativo — usado para verificaciones advisorías)
patterns  = ["feat/*", "fix/*", "hotfix/*", "release/*", "docs/*"]
# Los pushes directos a estas ramas están bloqueados para no-admins
protected = ["main", "release/*"]

[groups.backend]
members          = ["alice", "carlos"]
allowed_branches = ["feat/*", "fix/*", "hotfix/*"]
allowed_paths    = ["gitgov-server/", "src/"]

[groups.frontend]
members          = ["bob", "diana"]
allowed_branches = ["feat/*", "fix/*"]
allowed_paths    = ["gitgov/src/", "gitgov-web/"]

admins = ["admin-lead", "devops-ops"]
```

> **Nota**: La aplicación de políticas bloquea los pushes a ramas `protected` para desarrolladores que no están listados como `admins` o en un grupo con acceso explícito en `allowed_branches`. Todas las operaciones bloqueadas se registran como eventos `blocked_push` en la pista de auditoría.

---

## Verificación Advisoria CI

Para Jenkins y otros sistemas CI, el Control Plane provee un endpoint `/policy/check` que evalúa si un commit u operación cumple con las políticas:

```bash
curl -s -X POST https://tu-control-plane/policy/check \
  -H "Authorization: Bearer $GITGOV_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "repo_name": "TuOrg/TuRepo",
    "commit_sha": "a3f8c2e",
    "branch": "main",
    "user_login": "alice"
  }'
```

La respuesta incluye:
- `advisory` — `true` si la verificación no es bloqueante
- `allowed` — si la operación pasa la política actual
- `reasons` — lista de violaciones específicas
- `warnings` — avisos suaves (no bloqueantes)
- `evaluated_rules` — las reglas aplicadas para tomar esta decisión

> **Estado actual**: El endpoint `/policy/check` opera en **modo advisorio**. Informa a los pipelines CI sobre el estado de cumplimiento pero no detiene automáticamente los despliegues. La aplicación estricta a nivel CI está en el roadmap.

---

## Mejores Prácticas de Despliegue

1. **Fase 1 — Observación**: Despliega `gitgov.toml` sin ramas `protected`. Revisa los datos advisorios de `/policy/check` para identificar violaciones frecuentes.
2. **Fase 2 — Protección de Ramas**: Añade ramas críticas a `protected`. Solo los admins y grupos explícitamente autorizados pueden hacer push directamente.
3. **Fase 3 — Gobernanza Completa**: Aplica restricciones `allowed_paths` basadas en grupos e integra `/policy/check` en tu pipeline de Jenkins como paso de control.

## Siguiente Fase

- [**Trazabilidad CI/CD**](/docs/ci-traces) — Cierra la brecha entre commits y despliegues.
