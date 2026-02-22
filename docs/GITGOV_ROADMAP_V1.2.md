# GitGov V1.2 — Enterprise Integrations Roadmap
**Estado:** Planificación  
**Prerequisito:** V1.1 completado (GitHub App instalable, TTL por job type, violation_decisions)  
**Objetivo:** Convertir GitGov de herramienta de governance de Git a plataforma de trazabilidad completa: intención → implementación → resultado

---

## Contexto Estratégico

GitGov V1.0 responde: *"¿qué pasó en el código?"*  
GitGov V1.2 responde: *"¿por qué pasó, quién lo pidió, y funcionó en producción?"*

Esto cierra el ciclo completo que ninguna herramienta del mercado LATAM tiene integrado:

```
Jira Ticket (intención)
      ↓
Git Commit (implementación)
      ↓
Jenkins Pipeline (resultado)
      ↓
GitGov Audit Trail (evidencia)
```

Para un auditor, un CISO, o un cliente enterprise, esto es oro.

---

## V1.2.1 — Jenkins Integration

### Objetivo
Registrar cada pipeline de CI/CD como evento de auditoría y correlacionarlo con los commits que lo dispararon.

### Flujo
```
Dev hace push → GitGov registra client_event
GitHub recibe push → dispara webhook → GitGov registra github_event
Jenkins detecta push → ejecuta pipeline
Jenkins notifica a GitGov via webhook → GitGov registra pipeline_event
GitGov correlaciona: commit_sha une los tres eventos
```

### Endpoints nuevos

```
POST /integrations/jenkins          → recibe pipeline events
GET  /integrations/jenkins/status   → health check (admin)
```

### Payload que Jenkins envía a GitGov

```json
{
  "pipeline_id": "build-123",
  "job_name": "gitgov/main",
  "status": "success",
  "commit_sha": "abc123def456",
  "branch": "main",
  "repo_full_name": "MapfrePE/GitGov",
  "duration_ms": 45000,
  "triggered_by": "MapfrePE",
  "stages": [
    { "name": "Build", "status": "success", "duration_ms": 12000 },
    { "name": "Test", "status": "success", "duration_ms": 28000 },
    { "name": "Deploy", "status": "success", "duration_ms": 5000 }
  ],
  "artifacts": ["gitgov-server-v1.2.0.tar.gz"],
  "timestamp": 1740000000000
}
```

### Schema SQL

```sql
CREATE TABLE pipeline_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES orgs(id),
    pipeline_id TEXT NOT NULL,
    job_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('success', 'failure', 'aborted', 'unstable')),
    commit_sha TEXT,
    branch TEXT,
    repo_full_name TEXT,
    duration_ms BIGINT,
    triggered_by TEXT,
    stages JSONB DEFAULT '[]',
    artifacts JSONB DEFAULT '[]',
    ingested_at TIMESTAMPTZ DEFAULT NOW()
);

-- Trigger append-only
CREATE OR REPLACE FUNCTION pipeline_events_append_only()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' OR TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'pipeline_events is append-only';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE TRIGGER pipeline_events_immutable
    BEFORE UPDATE OR DELETE ON pipeline_events
    FOR EACH ROW EXECUTE FUNCTION pipeline_events_append_only();

-- Índices para correlación
CREATE INDEX idx_pipeline_events_commit ON pipeline_events(commit_sha);
CREATE INDEX idx_pipeline_events_org ON pipeline_events(org_id, ingested_at DESC);
CREATE INDEX idx_pipeline_events_branch ON pipeline_events(org_id, branch, ingested_at DESC);
```

### Jenkinsfile snippet para clientes

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
                        error("GitGov policy check failed. Commit ${env.GIT_COMMIT} does not meet governance requirements.")
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

### Endpoint nuevo: POST /policy/check

Este endpoint es crítico — es el que Jenkins llama antes de ejecutar el pipeline. Verifica:
- El commit está en una rama permitida según gitgov.toml
- El autor del commit tiene permisos para esa rama
- No hay bypass detectado para ese commit
- La rama no está en drift de políticas

Retorna 200 si todo está bien, 403 si hay violación de política.

---

## V1.2.2 — Jira Integration

### Objetivo
Correlacionar cada commit con el ticket de Jira que lo justifica. Sin ticket = commit sin justificación = riesgo de governance.

### Flujo
```
PM crea ticket JIRA-123 en Jira
Dev crea rama: feat/JIRA-123-implementar-login
Dev hace commits con mensaje: "feat: JIRA-123 agregar autenticación OAuth"
GitGov extrae JIRA-123 del branch name y del commit message
GitGov consulta Jira API para validar que el ticket existe y está activo
GitGov registra la correlación ticket↔commit
Cuando el PR se mergea, GitGov notifica a Jira para mover el ticket a Done
```

### Endpoints nuevos

```
POST /integrations/jira/webhook     → recibe eventos de Jira (ticket changes)
GET  /integrations/jira/tickets     → lista correlaciones ticket↔commit (admin)
GET  /compliance/:org/coverage      → % de commits con ticket asociado
```

### Schema SQL

```sql
CREATE TABLE project_tickets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES orgs(id),
    ticket_id TEXT NOT NULL,           -- "JIRA-123"
    ticket_url TEXT,
    title TEXT,
    status TEXT,                       -- "todo", "in_progress", "in_review", "done"
    assignee TEXT,
    reporter TEXT,
    priority TEXT,
    ticket_type TEXT,                  -- "story", "bug", "task", "epic"
    related_commits TEXT[] DEFAULT '{}',
    related_prs TEXT[] DEFAULT '{}',
    related_branches TEXT[] DEFAULT '{}',
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    ingested_at TIMESTAMPTZ DEFAULT NOW()
);

-- Correlación commits↔tickets (append-only, no updates)
CREATE TABLE commit_ticket_correlations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID REFERENCES orgs(id),
    commit_sha TEXT NOT NULL,
    ticket_id TEXT NOT NULL,
    correlation_source TEXT NOT NULL CHECK (
        correlation_source IN ('branch_name', 'commit_message', 'pr_title', 'manual')
    ),
    confidence FLOAT DEFAULT 1.0,     -- 1.0 = exacto, 0.5 = heurístico
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_commit_ticket_unique 
    ON commit_ticket_correlations(commit_sha, ticket_id);
CREATE INDEX idx_correlations_ticket 
    ON commit_ticket_correlations(org_id, ticket_id);
```

### Lógica de extracción de tickets

```rust
// En el servidor, al recibir un github_event de tipo push:
pub fn extract_ticket_ids(branch: &str, message: &str) -> Vec<String> {
    let patterns = [
        r"([A-Z]+-\d+)",           // JIRA-123, ABC-456
        r"#(\d+)",                  // GitHub issues: #123
        r"GH-(\d+)",               // GitHub issues alternativo
    ];
    
    let mut tickets = Vec::new();
    for pattern in &patterns {
        let re = Regex::new(pattern).unwrap();
        for cap in re.captures_iter(branch) {
            tickets.push(cap[1].to_string());
        }
        for cap in re.captures_iter(message) {
            tickets.push(cap[1].to_string());
        }
    }
    
    tickets.dedup();
    tickets
}
```

### Compliance coverage report

Un endpoint nuevo que responde a la pregunta que todo auditor hace:

```json
{
  "org": "MapfrePE",
  "period": "last_30_days",
  "total_commits": 450,
  "commits_with_ticket": 387,
  "coverage_percentage": 86.0,
  "commits_without_ticket": [
    {
      "commit_sha": "abc123",
      "author": "dev1",
      "branch": "main",
      "message": "fix typo",
      "timestamp": "2026-02-22T10:00:00Z"
    }
  ],
  "tickets_without_commits": [
    { "ticket_id": "JIRA-456", "title": "Implementar 2FA", "status": "in_progress" }
  ]
}
```

Esto es un reporte de compliance que los clientes enterprise van a querer exportar cada mes.

---

## V1.2.3 — Correlation Engine V2

### Objetivo
Cerrar el ciclo completo intención↔implementación↔resultado en un solo query.

### La query maestra

```sql
-- Para un org dado, mostrar el estado completo de cada ticket
SELECT 
    pt.ticket_id,
    pt.title,
    pt.status as jira_status,
    pt.assignee,
    array_agg(DISTINCT ctc.commit_sha) as commits,
    array_agg(DISTINCT ge.ref_name) as branches,
    COUNT(DISTINCT pe.id) as pipeline_runs,
    SUM(CASE WHEN pe.status = 'success' THEN 1 ELSE 0 END) as successful_deploys,
    MAX(pe.ingested_at) as last_deploy,
    -- Signal: ticket done pero sin pipeline success = código no deployado
    CASE 
        WHEN pt.status = 'done' AND SUM(CASE WHEN pe.status = 'success' THEN 1 ELSE 0 END) = 0
        THEN 'done_not_deployed'
        -- Signal: ticket in_progress pero sin commits en 3 días = dev bloqueado
        WHEN pt.status = 'in_progress' AND MAX(ge.created_at) < NOW() - INTERVAL '3 days'
        THEN 'stale_in_progress'
        ELSE 'ok'
    END as governance_signal
FROM project_tickets pt
LEFT JOIN commit_ticket_correlations ctc ON ctc.ticket_id = pt.ticket_id
LEFT JOIN github_events ge ON ge.after_sha = ctc.commit_sha
LEFT JOIN pipeline_events pe ON pe.commit_sha = ctc.commit_sha
WHERE pt.org_id = $1
GROUP BY pt.id, pt.ticket_id, pt.title, pt.status, pt.assignee
ORDER BY pt.updated_at DESC;
```

### Nuevos governance signals

Además de los signals actuales (bypass detection), V1.2 agrega:

| Signal | Descripción | Severidad |
|--------|-------------|-----------|
| `done_not_deployed` | Ticket cerrado pero código nunca llegó a producción | Alta |
| `stale_in_progress` | Ticket activo sin commits en 3+ días | Media |
| `commit_no_ticket` | Commit sin ticket asociado en rama protegida | Media |
| `pipeline_failure_streak` | 3+ pipelines fallidos consecutivos en mismo commit | Alta |
| `ticket_no_coverage` | Ticket done sin commits rastreables | Alta |

---

## V1.2.4 — Dashboard Updates

### Nuevos widgets para el Control Plane

**Widget 1: Pipeline Health**
```
┌─────────────────────────────────────┐
│ Pipeline Health (últimos 7 días)    │
│                                     │
│ Success Rate: 94.2%  ████████████░  │
│ Avg Duration: 3m 42s                │
│ Failed Builds: 3                    │
│ Repos con failures: 1               │
└─────────────────────────────────────┘
```

**Widget 2: Ticket Coverage**
```
┌─────────────────────────────────────┐
│ Cobertura de Tickets                │
│                                     │
│ Commits con ticket: 86%  ████████░░ │
│ Sin ticket (rama protegida): 12     │
│ Tickets sin commits: 4              │
└─────────────────────────────────────┘
```

**Widget 3: Compliance Timeline**
```
┌─────────────────────────────────────┐
│ Timeline de Compliance              │
│ Feb  ████████████████████  96%      │
│ Ene  ██████████████████░░  89%      │
│ Dic  ████████████████████  98%      │
└─────────────────────────────────────┘
```

---

## V1.2.5 — Configuración en gitgov.toml

```toml
[integrations.jenkins]
enabled = true
# Webhook secret para validar requests de Jenkins
webhook_secret = "env:JENKINS_WEBHOOK_SECRET"
# Si true, GitGov puede fallar el pipeline via policy check
enforce_policy = true
# Ramas donde se requiere policy check antes de deploy
enforce_on_branches = ["main", "production"]

[integrations.jira]
enabled = true
# URL de tu instancia Jira
url = "https://tu-empresa.atlassian.net"
# Token de API de Jira
api_token = "env:JIRA_API_TOKEN"
email = "admin@tu-empresa.com"
# Proyectos de Jira a monitorear
projects = ["GITGOV", "BACKEND", "FRONTEND"]

[integrations.jira.enforcement]
# Requerir ticket en commits a ramas protegidas
require_ticket_on_protected = true
# Patrones válidos de ticket en branch name o commit message
ticket_patterns = ["[A-Z]+-\\d+", "#\\d+"]
# Severity del signal si no hay ticket
no_ticket_severity = "medium"
```

---

## Prerequisitos Técnicos para V1.2

Antes de implementar V1.2, V1.1 debe estar completo:

**De V1.1:**
- GitHub App instalable (para que clients puedan onboardear sin configuración manual)
- TTL por job type (para que los jobs de correlación no hagan timeout)
- violation_decisions tabla (para el flujo completo de investigación)
- Heartbeat para jobs largos (la correlación V2 puede ser lenta)

**Infraestructura nueva:**
- URL pública permanente para el servidor (Railway/Render/AWS EC2)
- Credenciales de Jira API en .env
- Jenkins accesible desde internet para recibir notificaciones (o webhook relay)

---

## Estimación de Esfuerzo

| Feature | Complejidad | Tiempo estimado |
|---------|-------------|-----------------|
| pipeline_events schema + trigger | Baja | 2 horas |
| POST /integrations/jenkins | Media | 4 horas |
| POST /policy/check endpoint | Alta | 8 horas |
| Jenkinsfile snippet + docs | Baja | 2 horas |
| project_tickets schema | Baja | 2 horas |
| Jira webhook handler | Media | 6 horas |
| Ticket extraction logic | Media | 4 horas |
| Correlation Engine V2 query | Alta | 8 horas |
| Nuevos governance signals | Media | 6 horas |
| Dashboard widgets | Media | 8 horas |
| gitgov.toml integrations section | Baja | 2 horas |
| **Total** | | **~52 horas** |

---

## Argumento de Venta para Clientes Enterprise

Con V1.2, GitGov puede responder en tiempo real:

> *"El ticket JIRA-456 fue aprobado por el PM el 15 de febrero. El developer MapfrePE hizo 3 commits en la rama feat/JIRA-456 entre el 16 y el 18. El pipeline de Jenkins deployó exitosamente a producción el 19 a las 14:32. El código estuvo en producción 4 horas después del merge. Todo esto sin intervención manual, sin hojas de Excel, sin reuniones de seguimiento."*

Eso vale dinero para cualquier empresa con más de 10 developers.

---

## Siguiente Versión: V1.3 (Preview)

Después de V1.2, el paso natural es **V1.3: AI Governance Insights**:

- Detección de patrones anómalos con ML (developer que siempre bypasea los viernes)
- Predicción de riesgo de un commit antes del merge
- Sugerencias automáticas de políticas basadas en el historial de la org
- Resumen semanal de governance generado con IA

Pero eso es después de tener 10 clientes pagando. Primero el mercado valida, después la IA.

---

*Documento creado: 22 Feb 2026*  
*Autor: GitGov Architecture Team*  
*Versión: 1.0*
