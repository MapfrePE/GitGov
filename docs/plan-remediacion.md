# Plan de Remediacion Segura - GitGov

Fecha: 2026-02-24
Objetivo: corregir hallazgos de `tareas-errores.md` sin romper el flujo funcional actual (deteccion de cambios -> commit -> push -> eventos en Control Plane).

## Principios

- Proteger la ruta actual que funciona como `golden path`.
- Hacer cambios pequenos y verificables.
- Mantener compatibilidad temporal entre contratos cuando sea necesario.
- Separar fixes de seguridad de refactors grandes.
- Validar regresion despues de cada fase.

## Baseline funcional a proteger (smoke test)

1. Editar un archivo en un repo Git valido.
2. Ver el archivo en la lista de cambios de Desktop.
3. Hacer `commit` desde la app.
4. Hacer `push` desde la app.
5. Ver el commit en GitHub (hash visible).
6. Ver eventos en Control Plane (`stage_files`, `commit`, `attempt_push`, `successful_push`).
7. Confirmar que no se rompe el dashboard al refrescar.

## Fases de ejecucion

### Fase 0 - Estabilizacion de validaciones locales (bajo riesgo)

Objetivo: recuperar confianza en cambios con `typecheck/build/lint` y dejar baseline reproducible.

Tareas:
- Corregir errores TS/Lint de bajo riesgo del frontend.
- Agregar script `typecheck`.
- Ejecutar `npm run typecheck`, `npm run lint`, `npm run build`.
- Documentar resultado y gaps restantes.

Salida:
- Validaciones locales del frontend ejecutables.
- Sin cambios de logica de negocio en commit/push.

### Fase 1 - Seguridad critica backend (P0)

Objetivo: cerrar exposiciones de datos y spoofing sin romper clientes actuales.

Tareas:
- Corregir authz/scoping en `signals`, `export`, governance endpoints.
- Corregir `/events` para no confiar en identidad enviada por cliente.
- Arreglar validacion HMAC GitHub con body raw.
- Sanitizar errores de auth y mejorar comparacion segura.
- Agregar pruebas de autorizacion y webhooks.

Salida:
- Sin fuga cross-user/cross-org.
- Webhooks validos/invalidos manejados correctamente.

### Fase 2 - Integridad DB y migraciones

Objetivo: alinear backend con esquema real y evitar fallos por triggers append-only.

Tareas:
- Definir esquema canonico y migraciones versionadas.
- Corregir operaciones `UPDATE` incompatibles con append-only.
- Alinear `signals/violations/decisions`.
- Validar version de esquema en arranque.

Salida:
- Esquema reproducible y compatible con el codigo.

### Fase 3 - Desktop/Tauri hardening (P0/P1)

Objetivo: estabilidad del outbox y manejo seguro de secretos.

Tareas:
- Corregir shutdown del outbox (evitar bloqueo al cerrar).
- Eliminar fallback de token GitHub en archivo.
- Limpiar tokens en logout.
- Corregir URL encoding para rutas `owner/repo`.
- Alinear tipos de eventos emitidos con backend.

Salida:
- Desktop cierra limpio y no deja secretos en disco.

### Fase 4 - Contratos frontend-backend y UX de logs

Objetivo: alinear tipos/payloads y completar datos utiles (mensaje de commit).

Tareas:
- Unificar contratos TS/Rust (`ServerStats`, `CombinedEvent`, filtros).
- Corregir stores/control plane client drift.
- Persistir y mostrar `commit_message` en logs/dashboard.
- Mejorar estados de error y vacios.

Salida:
- UI estable, compilando y mostrando informacion correcta del commit.

### Fase 5 - Rendimiento y edge cases

Objetivo: corregir bugs de consultas/filtros y validar carga moderada.

Tareas:
- Corregir SQL de filtros/paginacion (`OFFSET`, placeholders).
- Revisar indices y consultas frecuentes.
- Probar concurrencia, duplicados y red intermitente.
- Agregar limites/rate limiting basico.

Salida:
- Consultas consistentes y mejor comportamiento bajo carga.

### Fase 6 - Hardening final pre-produccion

Objetivo: validar end-to-end y preparar despliegue controlado.

Tareas:
- Ejecutar E2E real Desktop -> Server -> DB -> Dashboard.
- Validar webhook GitHub real.
- Completar checklist de `tareas-errores.md`.
- Preparar rollback y despliegue gradual.

Salida:
- Release candidate verificable.

## Orden de implementacion recomendado

1. Fase 0
2. Fase 1
3. Fase 2
4. Fase 3
5. Fase 4
6. Fase 5
7. Fase 6

## Criterio de exito transversal (anti-regresion)

Despues de cada fase debe seguir funcionando:

- Desktop detecta cambios
- Commit desde app
- Push desde app
- Eventos llegan al Control Plane
- Dashboard carga y refresca
