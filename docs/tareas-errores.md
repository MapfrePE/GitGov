# Analisis Forense Pre-Production - GitGov

Fecha del analisis: 2026-02-24
Repositorio auditado: `c:\Users\PC\Desktop\GitGov`

## Alcance y metodologia

Analisis estatico y de integracion de:

- Backend Control Plane (`gitgov/gitgov-server`)
- Desktop App (Tauri + Rust) (`gitgov/src-tauri`)
- Frontend React (`gitgov/src`)
- Esquema y migraciones SQL (`gitgov/gitgov-server/supabase_schema*.sql`)
- Scripts de pruebas (`gitgov/gitgov-server/tests`)

Validaciones ejecutadas:

- `cargo check` en `gitgov/gitgov-server` (compila con warnings)
- `cargo check` en `gitgov/src-tauri` (compila con warnings)
- `npm run build` en `gitgov` (falla)
- `npm run lint` en `gitgov` (falla)
- `npm run typecheck` en `gitgov` (script inexistente)

Limitaciones (importantes):

- No se levanto PostgreSQL/Supabase local para pruebas E2E reales contra endpoints.
- No se ejecutaron scripts `e2e_flow_test.sh` / `stress_test.sh` por dependencia de entorno (server+DB+curl/bash) en esta revision.
- No se realizaron pruebas manuales cross-browser (Chromium/Firefox/Safari).

## Resumen ejecutivo

El sistema presenta **bloqueos criticos de produccion** en seguridad, autorizacion, integridad de datos y consistencia de migraciones. Los mas graves:

1. Endpoints autenticados con **autorizacion rota** (`/signals`, `/export`, `/events`, `/signals/detect/...`).
2. Validacion HMAC de GitHub **incorrecta** (usa JSON reserializado, no body raw).
3. Migracion SQL v3 **incompatible consigo misma** (`add_violation_decision()` queda bloqueada por trigger).
4. Desktop guarda token GitHub en archivo (contradice politica "solo keyring") y no lo elimina al logout.
5. Cierre de la app desktop puede **colgarse** por shutdown incorrecto del worker de outbox.
6. Frontend actual **no builda** y tiene API key hardcodeada.

## Priorizacion global de correcciones (orden sugerido)

### Fase 0 - Bloqueantes de despliegue (antes de produccion)

- `SEC-BE-001`, `SEC-BE-002`, `SEC-BE-003`, `SEC-BE-004`
- `INT-BE-001`
- `DB-MIG-001`
- `DB-BE-001`
- `SEC-DESK-001`
- `INT-DESK-001`
- `INT-BE-002`
- `FE-BUILD-001`
- `FE-SEC-001`

### Fase 1 - Riesgo alto (semana 1 de hardening)

- `AUTH-BE-001`, `AUTH-BE-002`, `AUTH-BE-003`
- `INT-BE-003`, `INT-BE-004`
- `DB-API-001`, `DB-API-002`, `DB-DATA-001`, `DB-OP-001`
- `INT-DESK-002`, `INT-DESK-003`, `INT-DESK-004`, `INT-DESK-005`
- `FE-AUTH-001`, `FE-QA-001`, `TEST-001`

### Fase 2 - Rendimiento, observabilidad y calidad

- `PERF-BE-001`, `PERF-BE-002`, `OBS-BE-001`, `UX-BE-001`
- `SEC-BE-005`, `SEC-BE-006`
- `FE-UX-001`, `FE-LINT-001`, `FE-LINT-002`, `FE-UX-002`, `FE-A11Y-001`, `FE-UX-003`
- `TEST-002`, `MAINT-BE-001`

## Hallazgos detallados

---

## 1) Backend / API / Seguridad

### `SEC-BE-001` - `GET /signals` filtra por usuario solo en comentario, no en consulta (exposicion de datos)

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:140`
  - `gitgov/gitgov-server/src/handlers.rs:147`
  - `gitgov/gitgov-server/src/db.rs:713`
- Descripcion tecnica:
  - `get_signals()` calcula `filter_user` para restringir a no-admin (`handlers.rs:141-145`), pero la variable **no se usa**.
  - `db.get_noncompliance_signals()` no recibe filtro por usuario, solo org/confidence/status/type/limit/offset.
  - Resultado: cualquier usuario autenticado puede enumerar senales de otros usuarios.
- Impacto en produccion:
  - Fuga de informacion de auditoria/compliance entre desarrolladores u organizaciones (si las API keys no estan correctamente segmentadas).
- Tareas de correccion (ordenadas):
  - Agregar parametro `user_login` a `get_noncompliance_signals()`.
  - En `get_signals()`, aplicar `auth_user.client_id` para no-admin de forma obligatoria.
  - Aplicar scope por `auth_user.org_id` cuando exista.
  - Agregar tests de autorizacion (admin vs developer).
- Estimacion: **3-5 h**
- Criterios de aceptacion:
  - Un usuario no-admin solo ve senales con `actor_login == auth_user.client_id`.
  - Un admin puede filtrar por `user_login` opcional.
  - Tests automatizados cubren ambos casos.

### `SEC-BE-002` - `POST /signals/{id}` permite actualizar senales sin control de rol ni ownership

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:177`
  - `gitgov/gitgov-server/src/main.rs:263`
- Descripcion tecnica:
  - `update_signal()` no recibe `Extension<AuthUser>` y no verifica admin/owner.
  - La ruta esta bajo middleware de auth, pero eso solo autentica; no autoriza la operacion.
- Impacto en produccion:
  - Cualquier API key valida puede cambiar estado/notas de cualquier senal (manipulacion de evidencia).
- Tareas de correccion (ordenadas):
  - Inyectar `AuthUser` en handler.
  - Definir politica: solo admin (recomendado) o admin + investigator asignado.
  - Validar `status` contra enum permitido.
  - Registrar actor en tabla append-only (`signal_decisions`) en lugar de mutar senal (ver `DB-BE-001`).
- Estimacion: **4-6 h**
- Criterios de aceptacion:
  - No-admin recibe `403` al intentar actualizar una senal.
  - Estados invalidos retornan `400`.
  - Cambio queda auditado con actor/autorizacion verificable.

### `SEC-BE-003` - `POST /export` exporta datos globales y no usa scope del usuario autenticado

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:383`
  - `gitgov/gitgov-server/src/handlers.rs:393`
  - `gitgov/gitgov-server/src/handlers.rs:399`
  - `gitgov/gitgov-server/src/main.rs:271`
- Descripcion tecnica:
  - `export_events()` no recibe `AuthUser`.
  - Calcula `org_id` desde `payload.org_name` solo para log (`ExportLog`), pero no lo aplica al `EventFilter`.
  - `get_combined_events()` se invoca con filtro que solo usa fechas; cualquier usuario autenticado puede exportar datos fuera de su scope.
- Impacto en produccion:
  - Exfiltracion de logs combinados de toda la organizacion/plataforma.
- Tareas de correccion (ordenadas):
  - Requerir `AuthUser` en handler.
  - En no-admin: forzar scope por `auth_user.client_id` y/o `auth_user.org_id`.
  - Validar `payload.org_name` contra `auth_user.org_id` cuando aplique.
  - Reusar una funcion de scope comun para `/logs`, `/export`, `/governance-events`.
  - Agregar test de intento de export cruzado.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - No-admin no puede exportar datos fuera de su usuario/org.
  - Admin puede exportar por org y por rango de fechas.
  - `ExportLog.exported_by` guarda actor real.

### `SEC-BE-004` - `/events` (ingesta cliente) confia en payload y permite spoofing de `user_login` / org / repo

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:746`
  - `gitgov/gitgov-server/src/handlers.rs:772`
  - `gitgov/gitgov-server/src/auth.rs:15`
  - `gitgov/gitgov-server/src/main.rs:273`
- Descripcion tecnica:
  - `ingest_client_events()` esta autenticado por Bearer, pero no recibe `AuthUser`.
  - Usa `input.user_login`, `input.org_name`, `input.repo_full_name` directamente desde el cliente.
  - No se verifica que el `user_login` coincida con `auth_user.client_id` ni que `org_id` coincida con `auth_user.org_id`.
- Impacto en produccion:
  - Un cliente con API key valida puede inyectar telemetria atribuida a otro usuario/otra org.
  - Corrompe auditoria, correlacion y reportes de cumplimiento.
- Tareas de correccion (ordenadas):
  - Inyectar `AuthUser` en el handler.
  - Sobrescribir `user_login` con `auth_user.client_id` para claves de desktop.
  - Si `auth_user.org_id` existe, validar/forzar `org_id`.
  - Rechazar eventos con `repo_full_name` fuera del scope de org (si hay metadata suficiente).
  - Registrar discrepancias como intento de spoofing.
- Estimacion: **6-10 h**
- Criterios de aceptacion:
  - El servidor ignora o rechaza `user_login` distinto al autenticado.
  - Una key scoped a org no puede enviar eventos de otra org.
  - Tests cubren casos de spoofing.

### `AUTH-BE-001` - `POST /signals/detect/{org}` no verifica admin pese a que el endpoint se documenta como admin

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:232`
  - `gitgov/gitgov-server/src/main.rs:265`
  - `gitgov/gitgov-server/src/main.rs:311`
- Descripcion tecnica:
  - `trigger_detection()` no recibe `AuthUser` ni llama a `require_admin()`.
  - El log de arranque lo anuncia como endpoint admin.
- Impacto en produccion:
  - Cualquier usuario autenticado puede disparar procesamiento intensivo (posible abuso/DoS).
- Tareas de correccion (ordenadas):
  - Inyectar `AuthUser` y exigir `require_admin`.
  - Considerar encolar job en lugar de ejecutar sincronamente.
  - Rate-limit por org/actor.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - No-admin recibe `403`.
  - Admin obtiene `202/200` y el trabajo queda trazado.

### `AUTH-BE-002` - `POST /violations/{id}/decisions` permite suplantar `decided_by`

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:266`
  - `gitgov/gitgov-server/src/handlers.rs:298`
  - `gitgov/gitgov-server/src/handlers.rs:309`
- Descripcion tecnica:
  - El request incluye `decided_by` en el body.
  - Aunque el endpoint requiere admin, el actor real (`auth_user.client_id`) no se impone; se persiste el valor del payload.
- Impacto en produccion:
  - Corrupcion del audit trail y no repudio debilitado.
- Tareas de correccion (ordenadas):
  - Eliminar `decided_by` del request publico.
  - Usar `auth_user.client_id` como unica fuente.
  - Mantener `decided_by` solo en respuesta (derivado del auth).
- Estimacion: **2-3 h**
- Criterios de aceptacion:
  - El body ya no acepta `decided_by`.
  - El valor persistido coincide siempre con el admin autenticado.

### `AUTH-BE-003` - Endpoints de lectura sin scoping efectivo (`/governance-events`, `/violations/.../decisions`)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:1259`
  - `gitgov/gitgov-server/src/handlers.rs:1260`
  - `gitgov/gitgov-server/src/handlers.rs:330`
  - `gitgov/gitgov-server/src/handlers.rs:331`
- Descripcion tecnica:
  - Ambos handlers reciben `AuthUser` pero lo ignoran (`_auth_user`).
  - `/governance-events` permite query global si no se envia `org_name`.
  - `/violations/{id}/decisions` no valida pertenencia a org.
- Impacto en produccion:
  - Exposicion de historiales de gobernanza/violaciones fuera de scope.
- Tareas de correccion (ordenadas):
  - Aplicar scope por `auth_user.org_id` en DB queries.
  - En no-admin, prohibir `org_name` arbitrario distinto a su org.
  - En decisiones de violacion, validar la violacion antes de devolver historial.
- Estimacion: **4-6 h**
- Criterios de aceptacion:
  - Usuario scoped solo obtiene datos de su org.
  - Requests sin `org_name` no devuelven dataset global a no-admin.

### `INT-BE-001` - Validacion HMAC de GitHub incorrecta (firma sobre JSON reserializado, no body raw)

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:463`
  - `gitgov/gitgov-server/src/handlers.rs:466`
  - `gitgov/gitgov-server/src/handlers.rs:585`
  - `gitgov/gitgov-server/src/handlers.rs:586`
- Descripcion tecnica:
  - `handle_github_webhook()` usa `Json<serde_json::Value>` y luego `validate_github_signature()` calcula HMAC sobre `serde_json::to_vec(payload)`.
  - GitHub firma el **raw request body**; la reserializacion cambia espacios/orden/escapes y rompe firmas validas.
- Impacto en produccion:
  - Ingestion de webhooks rota en produccion cuando `GITHUB_WEBHOOK_SECRET` esta configurado.
- Tareas de correccion (ordenadas):
  - Cambiar extractor a body raw (`Bytes`) y parsear JSON despues de validar HMAC.
  - Validar HMAC con raw bytes exactos.
  - Mantener payload raw para storage + payload parseado para procesamiento.
  - Agregar test con fixture firmado real.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - Webhook firmado por GitHub pasa con secret habilitado.
  - La validacion usa el body raw recibido.

### `SEC-BE-005` - Comparacion de firma HMAC no constante y errores de DB filtrados en auth

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:600`
  - `gitgov/gitgov-server/src/auth.rs:57`
- Descripcion tecnica:
  - `signature == computed` usa comparacion normal de string (no constant-time).
  - `auth_middleware()` devuelve detalles de DB en respuesta `401`.
- Impacto en produccion:
  - Hardening insuficiente y leakage de informacion interna.
- Tareas de correccion (ordenadas):
  - Usar comparacion constant-time / `verify_slice`.
  - Sanitizar errores de auth hacia cliente.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - No se exponen errores SQL en respuestas 401.
  - HMAC usa comparacion constante.

### `SEC-BE-006` - CORS abierto a cualquier origen/metodo/header para endpoints administrativos

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/main.rs:286`
- Descripcion tecnica:
  - `CorsLayer` permite `Any` para origenes, metodos y headers globalmente.
- Impacto en produccion:
  - Aumenta superficie de abuso si una API key llega a contexto browser.
- Tareas de correccion (ordenadas):
  - Restringir CORS por entorno.
  - Configurar lista de origins permitidos.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - Solo origins aprobados pasan CORS en prod.

### `INT-BE-002` - Endpoints de policy no aceptan `owner/repo` por routing con `{repo_name}` y slash

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/main.rs:268`
  - `gitgov/gitgov-server/src/main.rs:269`
  - `gitgov/gitgov-server/src/main.rs:270`
  - `gitgov/gitgov-server/src/handlers.rs:920`
  - `gitgov/src-tauri/src/control_plane/server.rs:235`
  - `gitgov/src-tauri/src/control_plane/server.rs:236`
- Descripcion tecnica:
  - Las rutas usan `Path<String>` y no capturan slash de `owner/repo`.
  - El cliente Tauri no URL-encodea `repo_name` al construir la URL.
- Impacto en produccion:
  - Policy/history/override inutilizables para repos reales.
- Tareas de correccion (ordenadas):
  - Cambiar contrato a query param o ruta `/policy/{owner}/{repo}`.
  - URL-encode desde cliente si persiste path param.
- Estimacion: **3-6 h**
- Criterios de aceptacion:
  - `owner/repo` funciona end-to-end en get/history/override.

### `INT-BE-003` - Ingesta de GitHub Audit Stream rompe deduplicacion (delivery_id aleatorio por entrada)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:1214`
  - `gitgov/gitgov-server/src/db.rs:1076`
  - `gitgov/gitgov-server/src/db.rs:1084`
- Descripcion tecnica:
  - `governance_events` deduplica por `delivery_id`.
  - `ingest_audit_stream()` genera `delivery_id` con UUID aleatorio, impidiendo idempotencia en reintentos.
- Impacto en produccion:
  - Duplicados en eventos de gobernanza y estadisticas infladas.
- Tareas de correccion (ordenadas):
  - Construir idempotency key deterministica desde campos estables.
  - Agregar tests de reingesta del mismo batch.
- Estimacion: **4-6 h**
- Criterios de aceptacion:
  - Reenviar el mismo lote no inserta duplicados.

### `INT-BE-004` - `is_relevant_audit_action()` hace match por prefijo demasiado amplio

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:1295`
  - `gitgov/gitgov-server/src/handlers.rs:1297`
- Descripcion tecnica:
  - La funcion acepta prefijos por primer segmento (`repo.*`, `org.*`, etc.) aunque solo exista una accion concreta en la lista.
- Impacto en produccion:
  - Ingesta de acciones no previstas y ruido.
- Tareas de correccion (ordenadas):
  - Separar reglas de match exacto y prefijos explicitamente permitidos.
  - Agregar pruebas allow/deny.
- Estimacion: **2-3 h**
- Criterios de aceptacion:
  - Solo acciones aprobadas son aceptadas.

### `DB-API-001` - `get_combined_events()` ignora gran parte de `EventFilter`

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:452`
  - `gitgov/gitgov-server/src/db.rs:457`
  - `gitgov/gitgov-server/src/models.rs:303`
  - `gitgov/gitgov-server/supabase_schema.sql:456`
- Descripcion tecnica:
  - `EventFilter` incluye repo/org/status/fechas, pero el wrapper solo envia `source`, `event_type`, `user_login` a la funcion SQL.
- Impacto en produccion:
  - `/logs`, `/dashboard` y `/export` devuelven datos filtrados incorrectamente.
- Tareas de correccion (ordenadas):
  - Extender wrapper/funcion SQL para soportar todos los campos.
  - Resolver `org_name`/`repo_full_name` a IDs antes de consultar.
- Estimacion: **6-10 h**
- Criterios de aceptacion:
  - Todos los campos de `EventFilter` tienen efecto comprobable.

### `DB-API-002` - SQL dinamico roto en `get_github_events()` / `get_client_events()` (placeholder de OFFSET)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:228`
  - `gitgov/gitgov-server/src/db.rs:245`
  - `gitgov/gitgov-server/src/db.rs:389`
  - `gitgov/gitgov-server/src/db.rs:409`
- Descripcion tecnica:
  - Se genera `OFFSET {n}` en vez de `OFFSET ${n}` y luego se bindea un parametro extra.
- Impacto en produccion:
  - Fallo runtime si estos metodos se usan.
- Tareas de correccion (ordenadas):
  - Corregir placeholders y agregar tests.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - Consultas ejecutan sin error con `limit/offset` y filtros.

### `OBS-BE-001` - `health_check()` reporta `pending_events` con query imposible (`client_events.status='pending'`)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:699`
  - `gitgov/gitgov-server/src/models.rs:229`
- Descripcion tecnica:
  - `client_events.status` no tiene valor `pending`; la metrica siempre sera 0.
- Impacto en produccion:
  - Observabilidad engañosa.
- Tareas de correccion (ordenadas):
  - Redefinir la metrica o removerla.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - `pending_events` representa una metrica real.

### `DB-DATA-001` - `get_compliance_dashboard()` puede fallar en orgs sin datos (NULL en `json_object_agg` + modelo sin default)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/supabase_schema.sql:1021`
  - `gitgov/gitgov-server/src/models.rs:700`
  - `gitgov/gitgov-server/src/models.rs:704`
  - `gitgov/gitgov-server/src/db.rs:994`
  - `gitgov/gitgov-server/src/db.rs:1003`
- Descripcion tecnica:
  - `json_object_agg` puede devolver `NULL` y `SignalStats.by_type` no tiene `#[serde(default)]`.
- Impacto en produccion:
  - `/compliance/{org}` puede responder 500 para orgs vacias.
- Tareas de correccion (ordenadas):
  - Agregar `COALESCE(...,'{}'::json)` y `#[serde(default)]`.
  - Probar org sin datos.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - Dashboard de compliance retorna `by_type: {}` cuando no hay senales.

### `DB-BE-001` - `update_signal_status()` incompatible con tabla append-only `noncompliance_signals`

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:839`
  - `gitgov/gitgov-server/supabase_schema.sql:647`
  - `gitgov/gitgov-server/supabase_schema.sql:650`
  - `gitgov/gitgov-server/supabase_schema.sql:211`
- Descripcion tecnica:
  - El metodo hace `UPDATE` sobre una tabla protegida por trigger append-only (`prevent_update_delete`).
- Impacto en produccion:
  - Flujo de investigacion de senales roto (errores 500).
- Tareas de correccion (ordenadas):
  - Rediseñar el flujo via `signal_decisions` (append-only) o ajustar trigger de forma consistente.
  - Eliminar mutacion directa si se mantiene append-only.
- Estimacion: **6-12 h**
- Criterios de aceptacion:
  - La actualizacion/investigacion de senales funciona sin violar append-only.

### `UX-BE-001` - Operaciones `UPDATE` retornan exito aunque no exista el registro (falta `rows_affected` check)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:845`
  - `gitgov/gitgov-server/src/db.rs:1587`
  - `gitgov/gitgov-server/src/handlers.rs:187`
- Descripcion tecnica:
  - Algunos `UPDATE` retornan `Ok(())` aunque no afecten filas.
- Impacto en produccion:
  - UX/admin engañosa y troubleshooting dificil.
- Tareas de correccion (ordenadas):
  - Verificar `rows_affected` y retornar `404/409`.
- Estimacion: **2-3 h**
- Criterios de aceptacion:
  - IDs inexistentes no devuelven exito.

### `SEC-BE-007` - Sin rate limiting / controles explicitos de volumen en endpoints de ingesta

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/main.rs:273`
  - `gitgov/gitgov-server/src/main.rs:284`
- Descripcion tecnica:
  - No hay middleware de cuota/rate limit para `/events`, `/webhooks/github`, `/audit-stream/github`.
- Impacto en produccion:
  - Riesgo de saturacion por spam o clientes defectuosos.
- Tareas de correccion (ordenadas):
  - Implementar rate limit por IP/api key y limite de tamaño de batch.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - Requests fuera de cuota reciben `429` y quedan metricados.

### `DB-OP-001` - Riesgo alto de despliegue por esquema fragmentado (`supabase_schema.sql`, `v2`, `v3`) sin migracion canonica

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/supabase_schema.sql`
  - `gitgov/gitgov-server/supabase_schema_v2.sql`
  - `gitgov/gitgov-server/supabase_schema_v3.sql`
- Descripcion tecnica:
  - El server usa columnas/funciones de migraciones v2/v3, pero no existe sistema de migracion versionado integrado.
- Impacto en produccion:
  - Despliegues con schema incompleto rompen worker/jobs/endpoints.
- Tareas de correccion (ordenadas):
  - Adoptar migraciones versionadas y chequeo de version al arranque.
- Estimacion: **8-16 h**
- Criterios de aceptacion:
  - Arranque falla rapido con mensaje claro si schema no es compatible.

---

## 2) Base de datos / Migraciones / Consistencia

### `DB-MIG-001` - `supabase_schema_v3.sql` se contradice: trigger bloquea `add_violation_decision()`

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/gitgov-server/supabase_schema_v3.sql:77`
  - `gitgov/gitgov-server/supabase_schema_v3.sql:84`
  - `gitgov/gitgov-server/supabase_schema_v3.sql:99`
  - `gitgov/gitgov-server/supabase_schema_v3.sql:127`
  - `gitgov/gitgov-server/supabase_schema_v3.sql:130`
- Descripcion tecnica:
  - La trigger `violations_no_resolution_update` prohibe cambiar `resolved/resolved_at/resolved_by`.
  - `add_violation_decision()` intenta actualizar esos campos cuando `decision_type='resolved'`.
  - El resultado esperado es excepcion en runtime para resoluciones.
- Impacto en produccion:
  - Flujo de decisiones de violacion roto para `resolved`.
- Tareas de correccion (ordenadas):
  - Elegir una sola estrategia de compatibilidad legacy (proyeccion/vista o update permitido).
  - Corregir trigger y/o funcion para que no se contradigan.
  - Test E2E de decision `resolved`.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - `add_violation_decision(...,'resolved',...)` funciona sin excepcion.

### `DB-SCHEMA-001` - `signal_decisions` definido dos veces en `supabase_schema.sql`

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/supabase_schema.sql:658`
  - `gitgov/gitgov-server/supabase_schema.sql:807`
- Descripcion tecnica:
  - La tabla y sus indices/triggers aparecen repetidos en dos bloques.
- Impacto en produccion:
  - Riesgo de drift y cambios inconsistentes.
- Tareas de correccion (ordenadas):
  - Consolidar en una unica definicion / migracion versionada.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - Existe una sola fuente de verdad para `signal_decisions`.

### `DB-PERF-001` - `validate_api_key()` hace `UPDATE` en cada request (write amplification)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:624`
  - `gitgov/gitgov-server/src/db.rs:627`
- Descripcion tecnica:
  - Cada request autenticado actualiza `last_used = NOW()`.
- Impacto en produccion:
  - Contencion/IO innecesario en hot path.
- Tareas de correccion (ordenadas):
  - Muestrear actualizacion o moverla a proceso async.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - El auth path no fuerza una escritura por request.

### `DB-PERF-002` - Batch inserts secuenciales sin transaccion/bulk insert

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/db.rs:329`
  - `gitgov/gitgov-server/src/db.rs:334`
  - `gitgov/gitgov-server/src/db.rs:1112`
  - `gitgov/gitgov-server/src/db.rs:1116`
- Descripcion tecnica:
  - Los batch inserts iteran item por item y llaman inserts individuales.
- Impacto en produccion:
  - Throughput bajo y mayor latencia en rafagas.
- Tareas de correccion (ordenadas):
  - Usar transaccion y insercion set-based (`QueryBuilder` / `UNNEST`).
- Estimacion: **6-12 h**
- Criterios de aceptacion:
  - Benchmark de batches mejora de forma medible.

---

## 3) Integracion entre modulos (Server <-> Desktop <-> Frontend)

### `INT-DESK-001` - Cierre de app desktop puede colgarse por shutdown incorrecto del worker de outbox

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/src-tauri/src/lib.rs:75`
  - `gitgov/src-tauri/src/lib.rs:77`
  - `gitgov/src-tauri/src/lib.rs:129`
  - `gitgov/src-tauri/src/lib.rs:131`
  - `gitgov/src-tauri/src/outbox/outbox.rs:462`
  - `gitgov/src-tauri/src/outbox/outbox.rs:485`
- Descripcion tecnica:
  - `lib.rs` crea una bandera `shutdown` local distinta de la usada por el worker interno del outbox.
  - `worker_handle.join()` espera a un thread que nunca recibe la señal correcta (`outbox.signal_shutdown()` no se llama).
- Impacto en produccion:
  - La app puede quedarse colgada al cerrar.
- Tareas de correccion (ordenadas):
  - Remover la bandera local y llamar `outbox.signal_shutdown()` antes de `join()`.
  - Agregar timeout/logica de escape para join.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - La app cierra sin bloquearse y el worker termina limpiamente.

### `INT-DESK-002` - Persistencia de sesion GitHub rota (`cmd_get_current_user()` siempre retorna `None`)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src-tauri/src/commands/auth_commands.rs:95`
  - `gitgov/src-tauri/src/commands/auth_commands.rs:96`
  - `gitgov/src-tauri/src/commands/auth_commands.rs:115`
  - `gitgov/src-tauri/src/commands/auth_commands.rs:123`
  - `gitgov/src/store/useAuthStore.ts:66`
- Descripcion tecnica:
  - `stored_login` esta hardcodeado a `None`.
  - `cmd_set_current_user()` es un stub no-op.
  - `checkExistingSession()` en frontend nunca recupera sesion.
- Impacto en produccion:
  - Reautenticacion obligatoria en cada reinicio.
- Tareas de correccion (ordenadas):
  - Persistir login actual y restaurarlo desde storage + keyring.
  - Implementar o eliminar `cmd_set_current_user`.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - Reiniciar la app conserva sesion valida.

### `SEC-DESK-001` - Token GitHub se guarda en archivo (violacion de requisito) y no se elimina al logout

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/src-tauri/src/github/auth.rs:169`
  - `gitgov/src-tauri/src/github/auth.rs:186`
  - `gitgov/src-tauri/src/github/auth.rs:213`
  - `gitgov/src-tauri/src/github/auth.rs:221`
  - `gitgov/src-tauri/src/github/auth.rs:238`
  - `gitgov/src-tauri/src/github/auth.rs:259`
  - `gitgov/src-tauri/src/github/auth.rs:316`
- Descripcion tecnica:
  - `save_token()` persiste en keyring **y tambien en archivo** (`*.token`).
  - `load_token()` cae a archivo si falla keyring.
  - `delete_token()` solo elimina del keyring.
- Impacto en produccion:
  - Exposicion de token OAuth en disco y logout incompleto.
- Tareas de correccion (ordenadas):
  - Eliminar fallback a archivo y limpiar archivos legacy existentes.
  - Asegurar que logout borre cualquier residuo legacy.
- Estimacion: **6-10 h**
- Criterios de aceptacion:
  - No se crean ni leen archivos `.token`; logout remueve credenciales completamente.

### `INT-DESK-003` - Tipos de eventos de ramas no compatibles con servidor; telemetria se corrompe por fallback silencioso

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src-tauri/src/commands/branch_commands.rs:58`
  - `gitgov/src-tauri/src/commands/branch_commands.rs:145`
  - `gitgov/gitgov-server/src/models.rs:210`
  - `gitgov/gitgov-server/src/models.rs:222`
- Descripcion tecnica:
  - Desktop emite `attempt_create_branch` y `branch_failed`.
  - Backend no soporta esos tipos y hace fallback a `AttemptPush`.
- Impacto en produccion:
  - Estadisticas y logs de eventos de ramas quedan mal clasificados.
- Tareas de correccion (ordenadas):
  - Unificar enums/contratos de eventos entre desktop y server.
  - Rechazar tipos desconocidos en vez de degradarlos silenciosamente.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - Los tipos emitidos por desktop se registran correctamente en server.

### `INT-DESK-004` - Configuracion de Control Plane en UI no reconfigura el outbox real (solo dashboard)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src-tauri/src/lib.rs:42`
  - `gitgov/src-tauri/src/lib.rs:55`
  - `gitgov/src/components/control_plane/ServerConfigPanel.tsx:11`
  - `gitgov/src/components/control_plane/ServerConfigPanel.tsx:12`
  - `gitgov/src/store/useControlPlaneStore.ts:78`
- Descripcion tecnica:
  - El outbox se configura solo al arrancar desde env.
  - La UI de Control Plane solo actualiza estado local para consultas (`cmd_server_*`), no el outbox.
- Impacto en produccion:
  - Falsa sensacion de conexion/configuracion; eventos pueden seguir yendo a otro server (o a ninguno).
- Tareas de correccion (ordenadas):
  - Crear comandos Tauri para actualizar config del outbox y persistirla.
  - Mostrar en UI la URL real del outbox + estado de cola.
- Estimacion: **6-12 h**
- Criterios de aceptacion:
  - Cambios de URL/API key en UI impactan el envio real de eventos del outbox.

### `INT-DESK-005` - Cliente Tauri de Control Plane usa contratos drifted (`/logs` filtros y `/events` payload legacy)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src-tauri/src/control_plane/server.rs:61`
  - `gitgov/src-tauri/src/control_plane/server.rs:67`
  - `gitgov/src-tauri/src/control_plane/server.rs:153`
  - `gitgov/src-tauri/src/commands/server_commands.rs:33`
- Descripcion tecnica:
  - `AuditFilter` del cliente usa nombres distintos a `/logs` del server (`developer_login` vs `user_login`, `action` vs `event_type`, `repo_name` vs `repo_full_name`).
  - `send_event()` usa payload legacy `EventPayload` hacia `/events`, incompatible con `ClientEventBatch`.
- Impacto en produccion:
  - Filtros ignorados silenciosamente; comando legacy roto si se usa.
- Tareas de correccion (ordenadas):
  - Unificar DTOs y eliminar/actualizar `cmd_server_send_event`.
  - Agregar tests de serializacion de query/body.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - Filtros enviados por Tauri si impactan `/logs`; no quedan endpoints llamados con payload legacy incompatible.

---

## 4) Frontend (React) - build, UX, accesibilidad, seguridad

### `FE-BUILD-001` - Frontend no compila (`npm run build`) por error de tipos en `useControlPlaneStore`

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/src/store/useControlPlaneStore.ts:117`
  - `gitgov/src/components/control_plane/ServerDashboard.tsx:5`
- Evidencia:
  - `npm run build` falla con `TS2304: Cannot find name 'AuditLogEntry'` y `TS6133` por import no usado.
- Descripcion tecnica:
  - `useControlPlaneStore` usa generic `AuditLogEntry[]` pero `serverLogs` esta tipado como `CombinedEvent[]`.
  - `ServerDashboard` tiene import `GitBranch` no usado.
- Impacto en produccion:
  - CI/build bloqueado.
- Tareas de correccion (ordenadas):
  - Cambiar generic a `CombinedEvent[]`.
  - Eliminar import no usado.
  - Agregar `typecheck` a CI.
- Estimacion: **0.5-1 h**
- Criterios de aceptacion:
  - `npm run build` pasa.

### `FE-SEC-001` - API key hardcodeada en frontend (`useControlPlaneStore`)

- Severidad: **critico**
- Prioridad: **P0**
- Modulo/archivo:
  - `gitgov/src/store/useControlPlaneStore.ts:58`
  - `gitgov/src/store/useControlPlaneStore.ts:61`
- Descripcion tecnica:
  - `DEFAULT_SERVER_CONFIG` incluye una API key UUID hardcodeada y `initFromEnv()` la usa automaticamente.
- Impacto en produccion:
  - Exposicion de credencial en codigo fuente y bundle.
- Tareas de correccion (ordenadas):
  - Remover API key del codigo.
  - Rotar/revocar la API key expuesta.
  - Configurar lectura segura desde env/storage de usuario.
- Estimacion: **1-3 h** (sin contar rotacion operativa)
- Criterios de aceptacion:
  - No hay credenciales embebidas en `src/`.
  - Clave expuesta revocada.

### `FE-QA-001` - Falta script `typecheck` requerido por proceso de calidad

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/package.json:6`
- Evidencia:
  - `npm run typecheck` retorna `Missing script: "typecheck"`.
- Descripcion tecnica:
  - El proceso documentado de linting menciona `npm run typecheck`, pero `package.json` no lo define.
- Impacto en produccion:
  - Pipeline de calidad incompleto.
- Tareas de correccion (ordenadas):
  - Agregar script `typecheck`.
  - Integrarlo en CI/pre-commit.
- Estimacion: **0.5 h**
- Criterios de aceptacion:
  - `npm run typecheck` existe y funciona.

### `FE-AUTH-001` - Estado admin nunca se establece tras login GitHub (funcionalidades admin inaccesibles)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src-tauri/src/commands/auth_commands.rs:85`
  - `gitgov/src-tauri/src/commands/auth_commands.rs:90`
  - `gitgov/src/pages/AuditPage.tsx:9`
  - `gitgov/src/components/layout/Sidebar.tsx:11`
- Descripcion tecnica:
  - Los comandos de auth siempre retornan `is_admin: false`.
  - La UI bloquea o esconde rutas admin (`/audit`).
- Impacto en produccion:
  - Admins reales no pueden usar paneles admin.
- Tareas de correccion (ordenadas):
  - Resolver rol desde policy/config o desde control plane.
  - Persistir `is_admin` en sesion restaurable.
- Estimacion: **4-8 h**
- Criterios de aceptacion:
  - Usuarios admin acceden a vistas admin; no-admin no.

### `FE-LINT-001` - `AuditLogRow` usa `Date.now()` en render y `<tr onClick>` sin semantica de teclado

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src/components/audit/AuditLogRow.tsx:16`
  - `gitgov/src/components/audit/AuditLogRow.tsx:38`
  - `gitgov/src/components/audit/AuditLogRow.tsx:40`
- Evidencia:
  - `npm run lint` reporta error `react-hooks/purity`.
- Descripcion tecnica:
  - `Date.now()` en render rompe pureza.
  - La fila clickable no es navegable por teclado.
- Impacto en produccion:
  - Falla lint/CI y problema de accesibilidad.
- Tareas de correccion (ordenadas):
  - Sacar calculo temporal del render.
  - Usar boton/role + handlers de teclado para expandir.
- Estimacion: **1-3 h**
- Criterios de aceptacion:
  - Lint pasa y el componente es operable con teclado.

### `FE-LINT-002` - `Toast.tsx` rompe regla de fast refresh y usa IDs aleatorios con colision potencial

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src/components/shared/Toast.tsx:20`
  - `gitgov/src/components/shared/Toast.tsx:23`
  - `gitgov/src/components/shared/Toast.tsx:31`
- Evidencia:
  - `npm run lint` reporta `react-refresh/only-export-components`.
- Descripcion tecnica:
  - El archivo mezcla store/helper y componentes.
  - IDs con `Math.random()` no garantizan unicidad fuerte.
- Impacto en produccion:
  - Mala DX y riesgo bajo de colision.
- Tareas de correccion (ordenadas):
  - Separar store/helper en otro modulo.
  - Usar `crypto.randomUUID()`.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - Lint sin errores de react-refresh.

### `FE-UX-001` - Nombre de repo en header se rompe en Windows (`split('/')`)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src/components/layout/Header.tsx:26`
- Descripcion tecnica:
  - El path de Windows usa `\`; `split('/')` no obtiene el nombre final correctamente.
- Impacto en produccion:
  - UI muestra ruta completa/truncada en vez del nombre del repo.
- Tareas de correccion (ordenadas):
  - Normalizar separadores o usar helper cross-platform.
- Estimacion: **0.5 h**
- Criterios de aceptacion:
  - En Windows se muestra el nombre correcto del repo.

### `FE-UX-002` - Auto-refresh de dashboard actualiza stats pero deja logs desactualizados

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src/components/control_plane/ServerDashboard.tsx:47`
  - `gitgov/src/components/control_plane/ServerDashboard.tsx:48`
- Descripcion tecnica:
  - El intervalo automatico solo llama `loadStats()`; `loadLogs()` no se refresca.
- Impacto en produccion:
  - "Eventos Recientes" puede quedar stale mientras auto-refresh esta activo.
- Tareas de correccion (ordenadas):
  - Incluir `loadLogs()` con throttling razonable.
  - Mostrar timestamp de ultima actualizacion.
- Estimacion: **1 h**
- Criterios de aceptacion:
  - Logs y stats se refrescan de forma consistente.

### `FE-A11Y-001` - Elementos icon-only sin `aria-label` explicita (Sidebar / Modal)

- Severidad: **bajo**
- Prioridad: **P3**
- Modulo/archivo:
  - `gitgov/src/components/layout/Sidebar.tsx:44`
  - `gitgov/src/components/shared/Modal.tsx:54`
- Descripcion tecnica:
  - Algunos controles icon-only dependen de `title`, sin `aria-label` explicita.
- Impacto en produccion:
  - Accesibilidad reducida.
- Tareas de correccion (ordenadas):
  - Agregar `aria-label` y validar con `axe`.
- Estimacion: **1 h**
- Criterios de aceptacion:
  - Auditoria a11y sin controles sin nombre accesible.

### `FE-UX-003` - `RepoSelector` traga errores del dialog y solo loguea en consola

- Severidad: **bajo**
- Prioridad: **P3**
- Modulo/archivo:
  - `gitgov/src/components/repo/RepoSelector.tsx:41`
  - `gitgov/src/components/repo/RepoSelector.tsx:42`
- Descripcion tecnica:
  - Fallos del dialogo no se muestran al usuario.
- Impacto en produccion:
  - Mala UX y soporte dificil.
- Tareas de correccion (ordenadas):
  - Mostrar toast/error visible.
- Estimacion: **0.5-1 h**
- Criterios de aceptacion:
  - El usuario ve mensaje accionable cuando falla el dialogo.

---

## 5) Desktop (Tauri) - Git/outbox/OAuth/control plane

### `INT-DESK-006` - `cmd_push` y otros comandos ignoran errores de `audit_db.insert()` (perdida silenciosa de auditoria local)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src-tauri/src/commands/git_commands.rs:215`
  - `gitgov/src-tauri/src/commands/git_commands.rs:265`
  - `gitgov/src-tauri/src/commands/git_commands.rs:300`
  - `gitgov/src-tauri/src/commands/branch_commands.rs:103`
  - `gitgov/src-tauri/src/commands/branch_commands.rs:137`
- Descripcion tecnica:
  - Inserciones al audit DB local se ejecutan con `let _ = ...`; las fallas se silencian.
- Impacto en produccion:
  - Huecos de auditoria local sin visibilidad.
- Tareas de correccion (ordenadas):
  - Loguear errores con `tracing::error!`.
  - Notificar a UI cuando falle persistencia local.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - Fallos de SQLite local quedan visibles y trazables.

### `PERF-DESK-001` - `trigger_flush()` crea un thread por accion aun con worker de outbox activo

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src-tauri/src/commands/git_commands.rs:21`
  - `gitgov/src-tauri/src/commands/branch_commands.rs:18`
  - `gitgov/src-tauri/src/lib.rs:77`
  - `gitgov/src-tauri/src/outbox/outbox.rs:462`
- Descripcion tecnica:
  - Existe worker background y, adicionalmente, se hace `spawn` por cada flush manual.
- Impacto en produccion:
  - Overhead y concurrencia innecesaria bajo actividad alta.
- Tareas de correccion (ordenadas):
  - Reemplazar flush inmediato por señal al worker (o una cola dedicada).
- Estimacion: **4-6 h**
- Criterios de aceptacion:
  - No se crean hilos por accion Git y el envio sigue siendo oportuno.

### `INT-DESK-007` - `cmd_push` valida ramas protegidas por igualdad exacta y puede omitir patrones configurados

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src-tauri/src/commands/git_commands.rs:190`
  - `gitgov/src-tauri/src/config/validator.rs:43`
- Descripcion tecnica:
  - `cmd_push` usa igualdad exacta para ramas protegidas mientras otras validaciones usan glob/patterns.
- Impacto en produccion:
  - Enforcement inconsistente de politica de ramas.
- Tareas de correccion (ordenadas):
  - Reutilizar el mismo motor de validacion de patrones.
- Estimacion: **2-4 h**
- Criterios de aceptacion:
  - Push y create-branch aplican la misma semantica de proteccion.

---

## 6) Frontend <-> Backend contratos / Estados / Flujos

### `FE-INT-001` - `useControlPlaneStore.loadLogs()` usa tipo incorrecto (`AuditLogEntry[]` vs `CombinedEvent[]`)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src/store/useControlPlaneStore.ts:42`
  - `gitgov/src/store/useControlPlaneStore.ts:117`
  - `gitgov/src/lib/types.ts:8`
  - `gitgov/src/lib/types.ts:21`
- Descripcion tecnica:
  - Contratos de auditoria local y control plane se mezclan; ya rompe el build.
- Impacto en produccion:
  - Build y render de dashboard de Control Plane bloqueados.
- Tareas de correccion (ordenadas):
  - Unificar tipado y separar DTOs por contexto.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - Store compila y usa `CombinedEvent[]` consistentemente.

### `FE-INT-002` - `initFromEnv()` no usa `import.meta.env` y contradice configuracion documentada

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/src/store/useControlPlaneStore.ts:72`
  - `gitgov/src/store/useControlPlaneStore.ts:74`
- Descripcion tecnica:
  - No existe lectura de `VITE_SERVER_URL`/`VITE_API_KEY`; el comentario "from env" es falso.
- Impacto en produccion:
  - Configuracion documentada no funciona.
- Tareas de correccion (ordenadas):
  - Implementar lectura real de env o corregir contrato/documentacion.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - `initFromEnv()` carga valores reales o el nombre de la funcion/comentario se alinea a la realidad.

### `FE-INT-003` - Filtros de auditoria y control plane comparten nombres ambiguos pero contratos distintos

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src/lib/types.ts:33`
  - `gitgov/src-tauri/src/control_plane/server.rs:61`
  - `gitgov/gitgov-server/src/models.rs:303`
- Descripcion tecnica:
  - Existen varios `AuditFilter/EventFilter` con campos incompatibles; el drift ya genero bugs reales.
- Impacto en produccion:
  - Riesgo alto de regresiones silenciosas en filtros.
- Tareas de correccion (ordenadas):
  - Renombrar DTOs por contexto y/o generar tipos compartidos desde schema.
- Estimacion: **3-6 h**
- Criterios de aceptacion:
  - Los filtros de cada capa tienen nombres claros y conversiones explicitas.

---

## 7) Rendimiento y escalabilidad

### `PERF-BE-001` - `/events` hace N+N lookups por batch y luego inserts secuenciales

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:752`
  - `gitgov/gitgov-server/src/handlers.rs:754`
  - `gitgov/gitgov-server/src/handlers.rs:763`
  - `gitgov/gitgov-server/src/handlers.rs:793`
  - `gitgov/gitgov-server/src/db.rs:329`
- Descripcion tecnica:
  - Por cada evento: lookup de org + repo; luego insert unitario. No hay cache por batch ni transaccion.
- Impacto en produccion:
  - Latencia y consumo de DB crecen linealmente con el batch.
- Tareas de correccion (ordenadas):
  - Cachear org/repo por batch.
  - Insercion masiva y limite de tamaño de batch.
- Estimacion: **8-16 h**
- Criterios de aceptacion:
  - Mejora de throughput medida y limites de batch aplicados.

### `PERF-BE-002` - Worker y endpoints de jobs dependen de migraciones sin schema check en arranque

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/main.rs:134`
  - `gitgov/gitgov-server/supabase_schema_v2.sql:249`
  - `gitgov/gitgov-server/supabase_schema_v2.sql:262`
  - `gitgov/gitgov-server/supabase_schema_v2.sql:287`
- Descripcion tecnica:
  - El worker arranca siempre, aunque schema pueda no tener funciones/columnas v2 requeridas.
- Impacto en produccion:
  - Errores repetitivos y degradacion del job queue en runtime.
- Tareas de correccion (ordenadas):
  - Validar schema version antes de arrancar worker.
  - Falla rapida con mensaje claro o feature-flag de worker.
- Estimacion: **3-6 h**
- Criterios de aceptacion:
  - El server detecta schema incompatible antes de procesar jobs.

---

## 8) Pruebas, edge cases y calidad de validacion

### `TEST-001` - `e2e_flow_test.sh` desactualizado: `/health` espera `"OK"` pero el server responde JSON

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/tests/e2e_flow_test.sh:26`
  - `gitgov/gitgov-server/tests/e2e_flow_test.sh:27`
  - `gitgov/gitgov-server/tests/e2e_flow_test.sh:8`
- Descripcion tecnica:
  - El test falla por contrato viejo y ademas trae API key default hardcodeada.
- Impacto en produccion:
  - Falsos negativos y confianza errada en pruebas de pipeline.
- Tareas de correccion (ordenadas):
  - Verificar `/health` por status code o JSON.
  - Exigir API key por env (sin defaults hardcodeados).
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - El script pasa contra el server actual sin hacks manuales.

### `TEST-002` - Cobertura de edge cases insuficiente / scripts no representan produccion (HMAC, auth scopes, filtros, vacios)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/tests/e2e_flow_test.sh`
  - `gitgov/gitgov-server/tests/stress_test.sh`
- Descripcion tecnica:
  - No cubren HMAC real, scoping por rol, dashboards vacios, filtros de `/logs`, ni rutas policy `owner/repo`.
  - `stress_test.sh` envia webhooks sin firma, lo que no representa produccion si `GITHUB_WEBHOOK_SECRET` esta activo.
- Impacto en produccion:
  - Bugs criticos de authz/HMAC pasan inadvertidos.
- Tareas de correccion (ordenadas):
  - Crear suite de integracion con DB efimera y fixtures firmados.
  - Agregar matriz de edge cases y pruebas de scoping.
- Estimacion: **16-32 h**
- Criterios de aceptacion:
  - La suite cubre authz, HMAC, filtros, vacios y rutas policy reales.

### `FE-QA-002` - `npm run lint` falla con 5 errores (sin gate de calidad previo al deploy)

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/src/components/audit/AuditLogRow.tsx:16`
  - `gitgov/src/components/audit/AuditLogView.tsx:11`
  - `gitgov/src/components/control_plane/ServerDashboard.tsx:5`
  - `gitgov/src/components/shared/Toast.tsx:20`
  - `gitgov/src/components/shared/Toast.tsx:31`
- Descripcion tecnica:
  - El frontend no esta lint-clean; esto baja la señal de regresiones.
- Impacto en produccion:
  - Mayor probabilidad de bugs y debt acumulada.
- Tareas de correccion (ordenadas):
  - Corregir errores actuales e integrar lint en CI como obligatorio.
- Estimacion: **1-3 h**
- Criterios de aceptacion:
  - `npm run lint` retorna exit code 0.

---

## 9) Mantenimiento / deuda tecnica

### `MAINT-BE-001` - `src/jobs.rs` contiene codigo roto/no compilable (archivo muerto)

- Severidad: **bajo**
- Prioridad: **P3**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/jobs.rs:47`
  - `gitgov/gitgov-server/src/jobs.rs:86`
  - `gitgov/gitgov-server/src/jobs.rs:103`
  - `gitgov/gitgov-server/src/jobs.rs:143`
  - `gitgov/gitgov-server/src/jobs.rs:148`
  - `gitgov/gitgov-server/src/jobs.rs:233`
- Descripcion tecnica:
  - El archivo tiene errores de sintaxis y referencias inexistentes, pero no se compila porque no esta incluido en `main.rs`.
- Impacto en produccion:
  - Deuda tecnica y riesgo de confusion en futuras refactorizaciones.
- Tareas de correccion (ordenadas):
  - Eliminarlo o reemplazarlo por una implementacion compilable y usada.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - No quedan implementaciones alternativas rotas del job queue en `src/`.

---

## Hallazgos adicionales de consistencia (recomendado corregir junto con P0/P1)

### `DATA-BE-001` - Fallback silencioso de enums (`from_str`) oculta errores y corrompe datos

- Severidad: **medio**
- Prioridad: **P2**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/models.rs:58`
  - `gitgov/gitgov-server/src/models.rs:210`
  - `gitgov/gitgov-server/src/models.rs:244`
- Descripcion tecnica:
  - Valores invalidos se transforman silenciosamente a defaults (`Developer`, `AttemptPush`, `Failed`).
- Impacto en produccion:
  - Datos corruptos y debugging dificil.
- Tareas de correccion (ordenadas):
  - Parsear a `Result` y rechazar payloads invalidos con `400`.
- Estimacion: **4-6 h**
- Criterios de aceptacion:
  - Valores desconocidos no se persisten ni se degradan silenciosamente.

### `INT-BE-005` - `create_api_key` con `org_name` inexistente genera key sin scope (fallback a `None`)

- Severidad: **alto**
- Prioridad: **P1**
- Modulo/archivo:
  - `gitgov/gitgov-server/src/handlers.rs:1120`
  - `gitgov/gitgov-server/src/handlers.rs:1122`
  - `gitgov/gitgov-server/src/db.rs:648`
- Descripcion tecnica:
  - Si `org_name` no existe y fue enviado, se termina creando una key con `org_id=None`.
- Impacto en produccion:
  - Error de configuracion con posible impacto de seguridad (scope mas amplio de lo deseado).
- Tareas de correccion (ordenadas):
  - Retornar `400` cuando `org_name` enviado no exista.
- Estimacion: **1-2 h**
- Criterios de aceptacion:
  - `org_name` invalido nunca crea una API key.

---

## Plan de remediacion recomendado (macro)

### Sprint de bloqueo (P0 - 2 a 4 dias)

1. Corregir authz y scoping en `/signals`, `/events`, `/export`, `/signals/detect`, `/governance-events`.
2. Corregir HMAC de webhooks usando body raw.
3. Resolver incompatibilidades de schema (`update_signal_status` vs append-only, `v3 add_violation_decision`).
4. Eliminar almacenamiento de tokens en archivo y limpiar tokens existentes.
5. Reparar shutdown del outbox worker.
6. Quitar API key hardcodeada y dejar frontend compilando (`npm run build`).
7. Corregir rutas/policy `owner/repo`.

### Sprint de estabilizacion (P1 - 1 semana)

1. Unificar contratos de eventos y filtros entre desktop/server/frontend.
2. Arreglar session restore y `is_admin`.
3. Hacer dedupe deterministic en audit stream.
4. COALESCE/serde defaults para dashboards de compliance.
5. Formalizar migraciones SQL versionadas.

### Sprint de hardening (P2/P3)

1. Rendimiento de batch ingest y inserts masivos.
2. Rate limiting / cuotas / body limits.
3. Lint/typecheck/CI gates.
4. A11y y UX menores.
5. Eliminar codigo muerto (`src/jobs.rs`).

## Matriz de estimacion total (aprox)

- P0 (bloqueantes): **34-66 h**
- P1 (estabilizacion): **31-62 h**
- P2/P3 (hardening/calidad): **22-47 h**

Rango total estimado: **87-175 horas** (dependiendo de refactor de contratos/migraciones y cobertura de tests automatizados).

## Checklist minimo de aceptacion antes de produccion

- [ ] `cargo check` y `cargo clippy -- -D warnings` pasan en `gitgov-server`
- [ ] `cargo check` y `cargo clippy -- -D warnings` pasan en `src-tauri`
- [ ] `npm run typecheck`, `npm run lint`, `npm run build` pasan en `gitgov`
- [ ] Webhooks GitHub con HMAC real se aceptan correctamente
- [ ] Pruebas de authz (admin/developer/scoped key) pasan
- [ ] Pruebas de export/logs/scoping pasan
- [ ] Dashboard compliance funciona con org sin datos
- [ ] No se almacenan tokens OAuth en archivos
- [ ] Cierre de desktop no se bloquea
- [ ] Migraciones SQL estan versionadas y validadas en arranque
