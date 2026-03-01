# GitGov — Roadmap Unificado

> Documento consolidado: visión, posicionamiento, estado de cada versión y demos.
> Última actualización: 2026-02-28

---

## Visión

GitGov es un **Git Governance Control Plane** que convierte las reglas de un equipo de ingeniería en código versionado, enforcement orquestado y evidencia inmutable — sin reemplazar GitHub.

### El problema

1. El desarrollador no puede elegir qué enviar — al hacer push sube todo, incluyendo archivos de debug y configuraciones locales.
2. No hay visibilidad — el arquitecto, PM y admin no saben quién subió qué, cuándo, ni si siguió las reglas.

### La solución

Una aplicación de escritorio + control plane que reemplaza el flujo de git directo con un workflow controlado: roles, nomenclatura de ramas forzada, staging selectivo y logs de auditoría inmutables.

### Ciclo de trazabilidad completo (V1.2+)

```
Jira Ticket (intención)
      ↓
Git Commit (implementación)
      ↓
Jenkins Pipeline (resultado)
      ↓
GitGov Audit Trail (evidencia)
```

---

## Posicionamiento

**Las tres capas del producto:**

| Capa | Qué hace | Diferenciador |
|------|----------|---------------|
| **Enforcement** | Orquesta branch protection, rulesets y status checks de GitHub a escala | GitGov no reemplaza los controles de GitHub — los orquesta y genera evidencia |
| **Evidencia** | Registro inmutable, correlación intención vs realidad, bypass detection | Contexto de gobernanza que no existe en SIEM ni en audit log de GitHub |
| **Visibilidad** | Dashboard en tiempo real, violations, drift, historial de políticas | Admin ve todo sin necesidad de saber Git |

**Frase clave para demos:**
> "GitGov no reemplaza los controles de GitHub — los orquesta a escala y genera la evidencia que GitHub no guarda."

**Dos buyers:**
- **Buyer A (Engineering Manager/CTO):** Reducción de fricción + visibilidad. Ciclo de venta corto.
- **Buyer B (CISO/Compliance):** Evidencia inmutable + bypass detection. Ciclo de venta largo. Se expande desde Buyer A (Land & Expand).

---

## V1.0 — Core Platform ✅ COMPLETADO

### Implementado
- [x] Desktop App: Tauri v2 + React 19 + Tailwind v4 + Zustand v5
- [x] Control Plane Server: Axum + Rust + PostgreSQL (Supabase)
- [x] GitHub OAuth + API Keys (Bearer auth, SHA256 hash)
- [x] Outbox offline con JSONL + backoff exponencial
- [x] Idempotencia con `event_uuid` / `delivery_id`
- [x] Webhook ingestion con HMAC
- [x] Health check detallado (`/health/detailed`)
- [x] Correlación `client_event ↔ github_event` por `commit_sha`
- [x] Bypass detection con confidence scoring (high/medium/low)
- [x] Compliance dashboard + noncompliance signals
- [x] Export con hash SHA256
- [x] Policy-as-code versionado (`gitgov.toml` + `policy_history`)
- [x] Roles: Admin, Architect, Developer, PM
- [x] Job queue con retry y dead letter
- [x] Web App marketing/docs (Next.js 14, Vercel)

### Features diferenciadores entregados
1. **Correlación + Bypass Detection** — Señales de noncompliance con lenguaje de evidencia, no acusaciones
2. **Policy-as-Code versionado** — `gitgov.toml` con historial automático
3. **Export auditable** — Metadatos: quién exportó, cuándo, rango, SHA256

### Pendiente (baja prioridad, post-tracción)
- [ ] Checklist configurable pre-push (guardrail UX)
- [ ] Drift Detection con ETags incrementales
- [ ] Hunk staging (complejo con libgit2, MVP usa stage por archivo)

---

## V1.2-A — Jenkins MVP ✅ COMPLETADO

**Objetivo:** Trazabilidad enterprise `commit → pipeline` sin romper el golden path.

### Implementado
- [x] `pipeline_events` schema + trigger append-only + índices
- [x] `POST /integrations/jenkins` — ingesta con validación
- [x] `GET /integrations/jenkins/status` — health check (admin)
- [x] `GET /integrations/jenkins/correlations` — correlación commit↔pipeline
- [x] Idempotencia + deduplicación por `pipeline_id + job_name + commit_sha`
- [x] Verificación de secreto Jenkins (`x-gitgov-jenkins-secret`, opcional)
- [x] Rate limiting + body size limit
- [x] Correlación básica por `commit_sha`
- [x] Widget Pipeline Health (7 días) en Dashboard
- [x] Badge `ci:<status>` en Commits Recientes
- [x] `POST /policy/check` en modo advisory
- [x] Tests de integración + smoke demo

### Endpoints

| Endpoint | Método | Auth | Propósito |
|----------|--------|------|-----------|
| `/integrations/jenkins` | POST | Bearer (admin) | Pipeline events |
| `/integrations/jenkins/status` | GET | Bearer (admin) | Health check |
| `/integrations/jenkins/correlations` | GET | Bearer (admin) | Correlaciones commit↔pipeline |
| `/policy/check` | POST | Bearer (admin) | Policy check advisory |

### Jenkinsfile snippet

```groovy
pipeline {
    agent any
    environment {
        GITGOV_URL = credentials('gitgov-server-url')
        GITGOV_KEY = credentials('gitgov-api-key')
    }
    stages {
        stage('GitGov Policy Check') {
            steps {
                script {
                    def response = sh(
                        script: """
                            curl -s -o /dev/null -w "%{http_code}" \
                            -X POST ${GITGOV_URL}/policy/check \
                            -H "Authorization: Bearer ${GITGOV_KEY}" \
                            -H "Content-Type: application/json" \
                            -d '{"repo": "${env.GIT_URL}", "commit": "${env.GIT_COMMIT}", "branch": "${env.GIT_BRANCH}"}'
                        """,
                        returnStdout: true
                    ).trim()
                    if (response != '200') {
                        error("GitGov policy check failed.")
                    }
                }
            }
        }
        stage('Build') { steps { sh 'cargo build --release' } }
        stage('Test') { steps { sh 'cargo test' } }
        stage('Deploy') { steps { sh './deploy.sh' } }
    }
    post {
        always {
            script {
                sh """
                    curl -s -X POST ${GITGOV_URL}/integrations/jenkins \
                    -H "Authorization: Bearer ${GITGOV_KEY}" \
                    -H "Content-Type: application/json" \
                    -d '{
                        "pipeline_id": "${env.BUILD_ID}",
                        "job_name": "${env.JOB_NAME}",
                        "status": "${currentBuild.result ?: 'success'}",
                        "commit_sha": "${env.GIT_COMMIT}",
                        "branch": "${env.GIT_BRANCH}",
                        "duration_ms": ${currentBuild.duration},
                        "triggered_by": "${env.BUILD_USER_ID ?: 'automated'}"
                    }'
                """
            }
        }
    }
}
```

---

## V1.2-B — Jira + Ticket Coverage 🟡 PREVIEW

**Objetivo:** Responder "¿este commit tiene justificación?" con cobertura de tickets.

### Implementado (preview)
- [x] `project_tickets` + `commit_ticket_correlations` schema
- [x] `POST /integrations/jira` — webhook ingesta
- [x] `GET /integrations/jira/status` — health check
- [x] `POST /integrations/jira/correlate` — correlación batch commit↔ticket
- [x] `GET /integrations/jira/ticket-coverage` — cobertura de tickets
- [x] `GET /integrations/jira/tickets/{id}` — detalle de ticket
- [x] Verificación de secreto Jira (`x-gitgov-jira-secret`, opcional)
- [x] Extracción de tickets desde branch/commit message (regex `[A-Z]+-\d+`)
- [x] Badge de ticket en Commits Recientes

### Pendiente
- [ ] Configuración `integrations.jira` en `gitgov.toml`
- [ ] Enforcement en modo `warn` (sin bloquear commits)
- [ ] Widget Ticket Coverage en Dashboard
- [ ] Webhook bidireccional (mover ticket a Done al mergear PR)

### Endpoints

| Endpoint | Método | Auth | Propósito |
|----------|--------|------|-----------|
| `/integrations/jira` | POST | Bearer (admin) | Webhook Jira |
| `/integrations/jira/status` | GET | Bearer (admin) | Health check |
| `/integrations/jira/correlate` | POST | Bearer (admin) | Correlación batch |
| `/integrations/jira/ticket-coverage` | GET | Bearer (admin) | Cobertura de tickets |
| `/integrations/jira/tickets/{id}` | GET | Bearer (admin) | Detalle de ticket |

---

## V1.2-C — Correlation Engine V2 + Compliance Signals ⏳ PENDIENTE

**Objetivo:** Cerrar el ciclo intención ↔ implementación ↔ resultado.

### Planificado
- [ ] Query/servicio de correlación V2 (ticket → commits → pipelines)
- [ ] Nuevos governance signals:
  - `done_not_deployed` — Ticket cerrado, código nunca en producción (Alta)
  - `stale_in_progress` — Ticket activo sin commits en 3+ días (Media)
  - `commit_no_ticket` — Commit sin ticket en rama protegida (Media)
  - `pipeline_failure_streak` — 3+ pipelines fallidos consecutivos (Alta)
  - `ticket_no_coverage` — Ticket done sin commits rastreables (Alta)
- [ ] Timeline de compliance (mensual)
- [ ] Optimización de performance (índices, materialización/cache)
- [ ] `policy/check` en modo bloqueante opcional por org/branch

### Estimación
~60-120 horas según scope final.

---

## V1.3 — AI Governance Insights 🔮 FUTURO

> Después de tener tracción con clientes. Primero el mercado valida, después la IA.

- Detección de patrones anómalos con ML (developer que siempre bypasea los viernes)
- Predicción de riesgo de un commit antes del merge
- Sugerencias automáticas de políticas basadas en historial
- Resumen semanal de governance generado con IA

---

## V2.0 — Multi-Provider 🔮 FUTURO

- Cross-provider: GitHub + GitLab + Bitbucket
- Gran diferenciador para empresas en migración o con equipos mixtos

---

## Modelo de Negocio

| Tier | Features | Target |
|------|----------|--------|
| **Starter** | Repos/devs limitados, retención 90 días, sin export/Jira | Entrada sin fricción |
| **Business** | Drift detection, correlación, bypass detection, export, Jira, retención 2 años | Engineering teams |
| **Enterprise** | Self-host + soporte operacional, SSO/SAML, SLA, multi-provider, retención 5+ años, precio custom | CISO/Compliance |

> Self-host es Enterprise exclusivo porque incluye soporte operacional: asistencia en upgrades, revisión de arquitectura de red, y SLA. No es solo "una imagen Docker."

> Precio por organización (no por usuario) porque se necesita 100% de adopción para que la gobernanza funcione.

---

## Demo V1.2-A (Jenkins MVP)

### Prerrequisitos
- Server con migración `supabase_schema_v5.sql` aplicada
- API key admin funcional
- Desktop App actualizada

### Paso 1 — Verificar Golden Path
```bash
cd gitgov/gitgov-server/tests && ./e2e_flow_test.sh
```

### Paso 2 — Commit desde Desktop
Editar archivo → commit → push desde la app. Verificar en Dashboard que aparece la fila.

### Paso 3 — Enviar pipeline event (simulando Jenkins)
```bash
curl -X POST http://localhost:3000/integrations/jenkins \
  -H "Authorization: Bearer TU_API_KEY_ADMIN" \
  -H "Content-Type: application/json" \
  -d '{
    "pipeline_id": "demo-build-001",
    "job_name": "gitgov/main",
    "status": "success",
    "commit_sha": "REEMPLAZAR_SHA",
    "branch": "main",
    "repo_full_name": "MapfrePE/GitGov",
    "duration_ms": 42000,
    "triggered_by": "jenkins",
    "stages": [
      { "name": "Build", "status": "success", "duration_ms": 12000 },
      { "name": "Test", "status": "success", "duration_ms": 24000 },
      { "name": "Deploy", "status": "success", "duration_ms": 6000 }
    ],
    "artifacts": ["gitgov-server-v1.2.0.tar.gz"],
    "timestamp": 1771900000000
  }'
```

### Paso 4 — Verificar correlación
```bash
curl -H "Authorization: Bearer TU_API_KEY_ADMIN" \
  "http://localhost:3000/integrations/jenkins/correlations?limit=10"
```

### Paso 5 — Verificar Dashboard
- Commits Recientes: badge `ci:success`
- Pipeline Health (7 días): success rate, avg duration, failures

### Paso 6 — Policy check advisory
```bash
curl -X POST http://localhost:3000/policy/check \
  -H "Authorization: Bearer TU_API_KEY_ADMIN" \
  -H "Content-Type: application/json" \
  -d '{"repo": "MapfrePE/GitGov", "commit": "REEMPLAZAR_SHA", "branch": "main"}'
```

### Script automático
```bash
cd gitgov/gitgov-server/tests
API_KEY="TU_API_KEY_ADMIN" ./jenkins_integration_test.sh
```

---

## Demo V1.2-B Preview (Jira)

### Prerrequisito adicional
- Migración `supabase_schema_v6.sql` aplicada

### Ingesta webhook Jira
```bash
curl -X POST http://localhost:3000/integrations/jira \
  -H "Authorization: Bearer TU_API_KEY_ADMIN" \
  -H "Content-Type: application/json" \
  -d '{
    "webhookEvent": "jira:issue_updated",
    "timestamp": 1771900000000,
    "issue": {
      "key": "PROJ-123",
      "self": "https://example.atlassian.net/rest/api/2/issue/10001",
      "fields": {
        "summary": "Pipeline de demo listo",
        "status": { "name": "In Progress" },
        "issuetype": { "name": "Task" },
        "priority": { "name": "Medium" },
        "assignee": { "displayName": "MapfrePE" },
        "reporter": { "displayName": "MapfrePE" },
        "created": "2026-02-24T20:10:00.000+0000",
        "updated": "2026-02-24T22:10:00.000+0000"
      }
    }
  }'
```

### Correlación batch
```bash
curl -X POST http://localhost:3000/integrations/jira/correlate \
  -H "Authorization: Bearer TU_API_KEY_ADMIN" \
  -H "Content-Type: application/json" \
  -d '{"hours": 72, "limit": 500, "repo_full_name": "MapfrePE/GitGov"}'
```

### Ticket Coverage
```bash
curl -H "Authorization: Bearer TU_API_KEY_ADMIN" \
  "http://localhost:3000/integrations/jira/ticket-coverage?hours=72&repo_full_name=MapfrePE/GitGov"
```

### Script automático
```bash
cd gitgov/gitgov-server/tests
API_KEY="TU_API_KEY_ADMIN" ./jira_integration_test.sh
```

---

## Troubleshooting de Demos

| Síntoma | Causa | Solución |
|---------|-------|----------|
| No aparece badge `ci:*` | SHA no coincide | Usar SHA completo, verificar `/integrations/jenkins/correlations` |
| `401` en Dashboard | Header incorrecto | Verificar `Authorization: Bearer`, revisar `GOLDEN_PATH_CHECKLIST.md` |
| `duplicate=true` al primer intento | Campos repetidos | Cambiar `pipeline_id` o `timestamp` |
| Jira devuelve `401` | Falta secreto | Si `JIRA_WEBHOOK_SECRET` está configurado, enviar `x-gitgov-jira-secret` |
| `correlations_created = 0` | Sin ticket en commit | Verificar que branch/message contiene `PROJ-123`, ampliar `hours` |

---

*Documento consolidado de: GITGOV_PLAN_CLAUDE_CODE.md, GITGOV_ROADMAP_COMERCIAL_v2.md, GITGOV_ROADMAP_V1.2.md, BACKLOG_V1.2-A.md, V1.2-A_DEMO.md*
*Fecha de consolidación: 2026-02-28*
