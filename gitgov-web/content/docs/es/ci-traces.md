---
title: Trazabilidad CI/CD
description: Cierra la brecha entre el código fuente, los artefactos de build y los despliegues en producción mediante integraciones con Jenkins, Jira y GitHub.
order: 5
---

Un gran punto ciego en la seguridad del software es el "build fantasma" — código que existe en producción sin ningún enlace verificable a un commit o desarrollador específico. GitGov cierra esta brecha integrándose directamente con tus pipelines CI/CD y herramientas de gestión de proyectos.

---

## La Cadena de Trazabilidad

GitGov establece un enlace de extremo a extremo entre tu código fuente y tus entornos de despliegue:

1. **Fase de Commit**: Los metadatos son capturados por GitGov Desktop (`commit_sha`, `branch`, `author`, `timestamp`).
2. **Fase de Build**: Tu pipeline CI publica los resultados del build al Control Plane. GitGov captura el nombre del job, estado del build, duración y resultados por etapa.
3. **Correlación**: El Control Plane empareja el commit SHA con el ID del build, creando una cadena de custodia verificable.

---

## Integraciones Soportadas

### Jenkins (V1.2-A — Disponible)
GitGov se integra con Jenkins mediante una llamada a la REST API desde tu `Jenkinsfile`. Después de cada build, un paso `curl` envía el resultado al endpoint `/integrations/jenkins` del Control Plane.

- **Metadatos capturados**: Nombre del job, commit SHA, rama, estado del build, duración del build, resultados por etapa, usuario que lo disparó y payload raw.
- **Correlación**: El Control Plane empareja automáticamente `commit_sha` con los eventos de commit existentes, creando un registro `CommitPipelineCorrelation`.
- **Análisis de fallos**: Correlaciona cambios de código específicos con regresiones de build y etapas fallidas.

### Jira (V1.2-B — Preview)
GitGov se integra con webhooks de Jira para capturar eventos de tickets y calcular la cobertura de tickets. El endpoint `/integrations/jira/ticket-coverage` reporta el porcentaje de commits en un repositorio que están vinculados a un ticket de Jira.

- **Seguimiento de cobertura**: Sabe qué porcentaje de tus commits referencian un ticket de Jira válido.
- **Correlación en lote**: El endpoint `/integrations/jira/correlate` ejecuta una pasada de correlación masiva contra todos los commits recientes.

### GitHub Webhooks
Conecta tus repositorios para recibir eventos de push, pull request y estado en tiempo real. Esto permite a GitGov verificar que cada pull request ha sido auditado y aprobado según las políticas de tu organización.

---

## Ejemplo de Configuración Jenkins

Añade un paso `post` a tu `Jenkinsfile` que llame directamente a la REST API del Control Plane:

```groovy
pipeline {
    agent any
    environment {
        GITGOV_URL = 'https://tu-control-plane'
        GITGOV_KEY = credentials('gitgov-admin-api-key')
    }
    stages {
        stage('Build') {
            steps {
                // ... tus pasos de build existentes ...
            }
        }
        stage('Test') {
            steps {
                // ... tus pasos de test existentes ...
            }
        }
        stage('Deploy') {
            steps {
                // ... tus pasos de despliegue existentes ...
            }
        }
    }
    post {
        always {
            script {
                def result = currentBuild.result ?: 'SUCCESS'
                def ts     = System.currentTimeMillis()
                sh """
                    curl -s -X POST \${GITGOV_URL}/integrations/jenkins \\
                      -H "Authorization: Bearer \${GITGOV_KEY}" \\
                      -H "Content-Type: application/json" \\
                      -d '{
                        "pipeline_id":    "\${env.BUILD_TAG}",
                        "job_name":       "\${env.JOB_NAME}",
                        "status":         "\${result.toLowerCase()}",
                        "commit_sha":     "\${env.GIT_COMMIT}",
                        "branch":         "\${env.GIT_BRANCH}",
                        "repo_full_name": "TuOrg/TuRepo",
                        "duration_ms":    \${currentBuild.duration},
                        "triggered_by":   "\${env.BUILD_USER_ID ?: 'ci'}",
                        "timestamp":      \${ts},
                        "stages": [
                          {"name": "Build",  "status": "success", "duration_ms": 134000},
                          {"name": "Test",   "status": "success", "duration_ms": 272000},
                          {"name": "Deploy", "status": "success", "duration_ms": 63000}
                        ]
                      }'
                """
            }
        }
    }
}
```

Almacena la API key como credencial de Jenkins (`gitgov-admin-api-key`) de tipo **Secret text**. El endpoint requiere un token Bearer con rol admin.

> [!IMPORTANT]
> **Garantía de Integridad**: Una vez que un build está vinculado a un commit en GitGov, el registro queda bloqueado y append-only. Cualquier intento de "re-etiquetar" un build existente a un commit diferente quedará registrado en la pista de auditoría.

---

## Integración de Verificación de Política (Advisoria)

Antes de ejecutar un build, tu pipeline de Jenkins puede consultar el Control Plane para obtener una verificación advisoria de política:

```groovy
stage('Policy Check') {
    steps {
        script {
            def response = sh(
                script: """curl -s -X POST \${GITGOV_URL}/policy/check \\
                  -H "Authorization: Bearer \${GITGOV_KEY}" \\
                  -H "Content-Type: application/json" \\
                  -d '{"repo_name": "TuOrg/TuRepo", "commit_sha": "\${env.GIT_COMMIT}", "branch": "\${env.GIT_BRANCH}", "user_login": "\${env.GIT_AUTHOR_NAME}"}'""",
                returnStdout: true
            ).trim()
            echo "GitGov policy check: ${response}"
            // Parsea la respuesta y opcionalmente falla el build ante violaciones
        }
    }
}
```

La respuesta incluye `allowed`, `reasons` y `warnings`. Este paso es actualmente **advisorio** — registra el estado de cumplimiento pero no bloquea el build a menos que añadas lógica explícita de fallo.

---

## Estableciendo Evidencia de Cumplimiento

Al usar la Trazabilidad CI, puedes generar informes automatizados para auditorías de cumplimiento:

- **Procedencia de Build**: Verificación de que un artefacto específico fue construido desde un commit específico en un servidor específico.
- **Cadenas de Aprobación**: Evidencia de que el código fue revisado y aprobado antes del merge.
- **Cobertura de Tickets**: El porcentaje de commits vinculados a un ticket de Jira rastreado (vía `/integrations/jira/ticket-coverage`).
- **Exportación de Logs de Auditoría**: Exporta informes JSON completos vía el endpoint `/export` para auditorías SOC 2 o ISO 27001.

---

## Resumen de Capacidades

| Funcionalidad | Endpoint | Estado |
|---------------|----------|--------|
| Vincular commits a builds Jenkins | `POST /integrations/jenkins` | Disponible (V1.2-A) |
| Consulta de correlación commit–pipeline | `GET /integrations/jenkins/correlations` | Disponible (V1.2-A) |
| Widget de salud del pipeline | `GET /integrations/jenkins/status` | Disponible (V1.2-A) |
| Cobertura de tickets Jira | `GET /integrations/jira/ticket-coverage` | Preview (V1.2-B) |
| Correlación Jira en lote | `POST /integrations/jira/correlate` | Preview (V1.2-B) |
| Ingestión de webhooks GitHub | `POST /webhooks/github` | Disponible |
| Exportación de logs de auditoría (JSON) | `POST /export` | Disponible |
| Verificación advisoria de política CI | `POST /policy/check` | Disponible (advisorio) |

## Fin de la Documentación Principal

- [**Volver al Inicio**](/)
- [**Contactar con Ventas para Soporte Enterprise**](/contact)
