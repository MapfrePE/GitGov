# GitGov V1.2-A — Backlog Técnico (Jenkins-first MVP)

**Estado:** Listo para ejecución  
**Fecha:** 2026-02-24  
**Fuente:** `docs/GITGOV_ROADMAP_V1.2.md` (V1.2-A)

---

## Objetivo de V1.2-A

Entregar trazabilidad enterprise básica `commit -> pipeline` sin romper el flujo actual de GitGov:

- Desktop detecta cambios
- Commit/Push funcionan
- Eventos llegan al Control Plane
- Dashboard sigue operativo

**Resultado visible esperado:** para un commit reciente, GitGov puede mostrar el pipeline Jenkins asociado y su estado.

---

## Alcance (incluido / excluido)

### Incluido
- `pipeline_events` schema + migración + append-only
- `POST /integrations/jenkins`
- `GET /integrations/jenkins/status` (admin)
- Correlación básica por `commit_sha`
- Widget MVP de `Pipeline Health`
- `POST /policy/check` en modo `advisory`
- Idempotencia, auth, validaciones, límites básicos
- Pruebas de integración mínimas y smoke demo

### Excluido (V1.2-A)
- Bloqueo real de pipeline por policy (`enforce`)
- Jira integration
- Correlation Engine V2 completo
- Timeline de compliance

---

## Orden de Ejecución (recomendado)

1. Infraestructura de datos (`pipeline_events`)
2. Ingesta Jenkins (`/integrations/jenkins`)
3. Idempotencia + seguridad + límites
4. Correlación básica por `commit_sha`
5. Dashboard Pipeline Health (MVP)
6. `policy/check` advisory
7. Tests E2E demo + docs cliente

---

## Épicas y Tareas

## Épica A1 — Base de Datos `pipeline_events`

**Objetivo:** Persistir eventos de pipeline como fuente append-only, indexada para correlación.

### Tareas

1. Crear migración SQL versionada para `pipeline_events`
- Tabla `pipeline_events`
- trigger append-only
- índices por `commit_sha`, `org_id+ingested_at`, `org_id+branch+ingested_at`
- **Estimación:** 4h
- **Dependencias:** ninguna

2. Añadir validación de esquema al arranque (opcional pero recomendada)
- detectar ausencia de `pipeline_events`
- log claro en startup
- **Estimación:** 2h
- **Dependencias:** tarea A1.1

3. Añadir modelos Rust (`PipelineEvent`, `PipelineStage`, payloads)
- tipos server-side con `serde`
- enums/strings validados (`success`, `failure`, `aborted`, `unstable`)
- **Estimación:** 3h
- **Dependencias:** tarea A1.1

### Criterios de aceptación (Épica A1)
- Migración corre en DB limpia y en DB existente sin romper tablas actuales.
- `UPDATE/DELETE` sobre `pipeline_events` falla por trigger append-only.
- Índices visibles en DB.

---

## Épica A2 — Ingesta Jenkins (`POST /integrations/jenkins`)

**Objetivo:** Recibir eventos de Jenkins y persistirlos con validación de payload.

### Tareas

1. Diseñar contrato de payload v1 (server canonical)
- definir campos requeridos vs opcionales
- normalizar `status`, `timestamp`, `stages`, `artifacts`
- **Estimación:** 2h
- **Dependencias:** A1.3

2. Implementar handler `POST /integrations/jenkins`
- parse de payload
- validación básica
- resolución de `org_id` por `repo_full_name`/`org` (si existe)
- insert en `pipeline_events`
- respuesta JSON consistente
- **Estimación:** 8h
- **Dependencias:** A1.1, A1.3, A2.1

3. Implementar `GET /integrations/jenkins/status` (admin)
- health endpoint de integración
- stats mínimas (`last_ingest_at`, count últimos N eventos)
- **Estimación:** 4h
- **Dependencias:** A2.2

4. Añadir rutas en `main.rs` + wiring de auth
- scoping/admin en status endpoint
- **Estimación:** 2h
- **Dependencias:** A2.2, A2.3

### Criterios de aceptación (Épica A2)
- Jenkins puede enviar payload válido y el server responde `200`.
- Eventos inválidos retornan `400` con error claro (sin filtrar secretos).
- `GET /integrations/jenkins/status` requiere admin y responde datos reales.

---

## Épica A3 — Seguridad, Idempotencia y Hardening de Integración

**Objetivo:** Evitar duplicados, abuso y payloads inseguros en endpoints nuevos.

### Tareas

1. Definir estrategia de idempotencia Jenkins
- clave recomendada: `pipeline_id + job_name + commit_sha + timestamp`
- alternativa con header `X-Request-Id`
- **Estimación:** 2h
- **Dependencias:** A2.1

2. Implementar deduplicación en DB / lógica server
- índice único o check lógico con `ON CONFLICT`
- respuesta clara para duplicados (aceptado/duplicado)
- **Estimación:** 6h
- **Dependencias:** A1.1, A3.1

3. Verificación de secreto/firma (si cliente Jenkins lo soporta)
- header secreto simple o HMAC (fase A puede usar shared secret)
- config por env
- errores `401/403` sanitizados
- **Estimación:** 6h
- **Dependencias:** A2.2

4. Rate limiting + body size limit para endpoint Jenkins
- reutilizar middleware actual de rate limiting
- límite de payload para stages/artifacts grandes
- **Estimación:** 4h
- **Dependencias:** A2.2

5. Logs y métricas de integración (sin exponer tokens)
- tracing fields útiles (`job_name`, `pipeline_id`, `commit_sha`)
- **Estimación:** 2h
- **Dependencias:** A2.2

### Criterios de aceptación (Épica A3)
- Reenvío del mismo payload no crea duplicados.
- Requests sin credencial/firma válidas son rechazadas.
- Burst razonable activa rate limiting sin afectar endpoints existentes.
- Payload excesivo retorna error controlado.

---

## Épica A4 — Correlación Básica por `commit_sha`

**Objetivo:** Unir `client/github/pipeline` a nivel mínimo para demo enterprise.

### Tareas

1. Diseñar vista/consulta de correlación básica
- por `commit_sha`
- último pipeline por commit
- estado agregado simple
- **Estimación:** 4h
- **Dependencias:** A2.2

2. Implementar query en backend
- endpoint reutilizable o extendiendo `/dashboard`
- evitar degradar `/logs`
- **Estimación:** 8h
- **Dependencias:** A4.1

3. Resolver normalización de SHA (short vs full)
- comparar por prefijo seguro o normalizar longitud
- documentar comportamiento
- **Estimación:** 4h
- **Dependencias:** A4.2

### Criterios de aceptación (Épica A4)
- Un commit reciente con `commit_sha` se correlaciona con pipeline Jenkins.
- Si no existe pipeline, el sistema responde sin error (estado vacío).
- Consultas responden con tiempos aceptables en entorno demo.

---

## Épica A5 — Dashboard MVP (Pipeline Health)

**Objetivo:** Hacer visible el valor de V1.2-A en Control Plane.

### Tareas

1. Diseñar contrato frontend-backend para métricas de pipelines
- success rate, avg duration, failed builds, repos con failures
- **Estimación:** 2h
- **Dependencias:** A4.2

2. Implementar endpoint/expansión de `/dashboard` o `/stats`
- métricas últimos 7 días
- defaults seguros (sin nulls)
- **Estimación:** 6h
- **Dependencias:** A1.1, A2.2

3. Implementar widget `Pipeline Health` en dashboard
- UI simple, consistente con diseño actual
- estados vacíos y errores
- **Estimación:** 6h
- **Dependencias:** A5.1, A5.2

4. (Opcional V1.2-A) Mostrar pipeline status por commit en `Commits Recientes`
- badge success/failure/unstable junto al commit
- **Estimación:** 6h
- **Dependencias:** A4.2, A5.3

### Criterios de aceptación (Épica A5)
- Dashboard carga sin romper widgets existentes.
- Widget muestra datos reales tras ingesta de Jenkins.
- Si no hay pipelines, muestra estado vacío claro.

---

## Épica A6 — `POST /policy/check` (Advisory)

**Objetivo:** Permitir a Jenkins consultar policy antes del pipeline sin bloquear builds aún.

### Tareas

1. Definir contrato v1 de `policy/check`
- request: repo, commit, branch
- response: `allowed`, `reasons[]`, `mode=advisory`
- **Estimación:** 3h
- **Dependencias:** ninguna (pero ideal después de A2)

2. Reusar lógica de policy existente (sin duplicar reglas)
- branch permitida
- autor/permisos (si disponible)
- bypass/drift básico
- **Estimación:** 10h
- **Dependencias:** A6.1

3. Implementar handler + auth + tiempos de respuesta
- timeout defensivo
- respuestas 200/403/500 consistentes
- **Estimación:** 8h
- **Dependencias:** A6.2

4. Modo advisory en respuesta/documentación Jenkins
- no romper pipeline por defecto
- **Estimación:** 2h
- **Dependencias:** A6.3

### Criterios de aceptación (Épica A6)
- Jenkins recibe respuesta usable en <500ms p95 (demo env).
- `policy/check` no rompe el pipeline si GitGov falla (modo advisory documentado).
- Reglas de policy reutilizan lógica existente (sin drift funcional evidente).

---

## Épica A7 — QA, Demo y Documentación de Cliente

**Objetivo:** Poder demostrar V1.2-A de punta a punta sin improvisación.

### Tareas

1. Crear script/demo reproducible de Jenkins → GitGov
- payloads de ejemplo
- cURL/Postman collection
- **Estimación:** 3h
- **Dependencias:** A2.2, A3.2

2. Tests de integración mínimos (server)
- payload válido
- duplicado
- auth inválida
- **Estimación:** 8h
- **Dependencias:** A2.2, A3.x

3. Smoke test E2E de no regresión del golden path
- Desktop commit/push
- logs/dashboard siguen bien
- **Estimación:** 4h
- **Dependencias:** todas las épicas anteriores

4. Documentación para cliente (Jenkinsfile + setup)
- variables de entorno
- secretos
- troubleshooting
- **Estimación:** 4h
- **Dependencias:** A2.2, A6.4

### Criterios de aceptación (Épica A7)
- Demo E2E documentada y repetible.
- Existe checklist de no regresión del flujo base.
- Snippet Jenkins y setup funcionan en entorno de prueba.

---

## Dependencias Críticas

- `V1.1` realmente completado (según roadmap)
- URL pública del server (o túnel estable para demo)
- Jenkins con credenciales para llamar a GitGov
- API key/admin key funcional para endpoints de admin/status
- DB con migraciones versionadas aplicables

---

## Riesgos y Mitigaciones (V1.2-A)

## Riesgo 1 — Romper el flujo actual Desktop/Control Plane
- **Mitigación:** smoke test obligatorio después de cada épica (A2, A5, A6)

## Riesgo 2 — `policy/check` se vuelve cuello de botella
- **Mitigación:** lanzar en `advisory`, timeout corto, caching posterior si hace falta

## Riesgo 3 — Duplicados o reintentos de Jenkins ensucian métricas
- **Mitigación:** idempotencia desde A3 antes de demo oficial

## Riesgo 4 — Incompatibilidad de payloads Jenkins entre clientes
- **Mitigación:** contrato v1 canónico + campos opcionales + validación clara

---

## Estimación Consolidada (V1.2-A)

| Épica | Rango |
|------|-------|
| A1 Base de datos | 9h |
| A2 Ingesta Jenkins | 16h |
| A3 Seguridad/Idempotencia | 20h |
| A4 Correlación básica | 16h |
| A5 Dashboard MVP | 14-20h |
| A6 Policy Check advisory | 23h |
| A7 QA/Docs/Demo | 19h |
| **Total estimado** | **117-123 horas** |

> Si se recorta alcance (sin `policy/check` en A y sin pipeline status por commit), V1.2-A puede bajar a ~70-90h.

---

## Recorte Recomendado para Arrancar (Sprint 1)

Si quieren empezar ya y entregar valor rápido:

### Sprint 1 (1-2 semanas)
- A1 completo
- A2 completo
- A3.1 + A3.2 (idempotencia)
- A5.1 + A5.2 + A5.3 (widget pipeline básico)
- A7.1 (demo de ingesta)

**Entregable Sprint 1:** Jenkins ingesta + dashboard pipeline health + no duplicados.

### Sprint 2
- Resto de A3 (firma, límites, logs)
- A4 correlación commit↔pipeline
- A7 tests de integración

### Sprint 3
- A6 `policy/check` advisory
- A7 docs cliente + smoke final

---

## Checklist de Arranque (Hoy)

1. Confirmar si `policy/check` entra en V1.2-A o se mueve a A+1.
2. Definir contrato v1 de payload Jenkins (campos requeridos/optional).
3. Crear migración `pipeline_events`.
4. Abrir épicas/tickets A1-A7 en tracker interno.
5. Preparar entorno demo Jenkins (o simular con cURL para sprint 1).

