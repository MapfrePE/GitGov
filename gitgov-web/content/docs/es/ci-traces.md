---
title: Trazabilidad CI/CD
description: Cierre la brecha entre el código fuente, los artefactos de construcción y los despliegues en producción.
order: 5
---

Un punto ciego importante en la seguridad del software es el "build fantasma": código que existe en producción pero que no tiene un vínculo verificable con un commit o desarrollador específico. GitGov cierra esta brecha integrándose directamente con sus pipelines de CI/CD.

---

## La Cadena de Trazabilidad

GitGov establece un vínculo bidireccional entre su código fuente y sus entornos de despliegue:

1. **Fase de Commit**: Los metadatos son capturados por GitGov Desktop.
2. **Fase de Build**: La integración de CI de GitGov captura el ID de Build, el Entorno y el estado de éxito.
3. **Correlación**: El Control Plane vincula el Hash del Commit con el ID del Build.

---

## Integraciones Soportadas

### Integración con Jenkins
GitGov se integra con Jenkins mediante una llamada REST API desde tu `Jenkinsfile`. Después de cada build, un paso `curl` envía el resultado al endpoint `/integrations/jenkins` del Control Plane.
- **Inyección Automática de Metadatos**: Se capturan nombre del job, commit SHA, rama, duración del build y resultados por stage.
- **Análisis de Fallos**: Correlaciona cambios específicos de código con regresiones en la construcción y stages fallidos.

### Webhooks de GitHub
Conecta tus repositorios para recibir eventos de push, pull requests y reviews en tiempo real. Esto permite a GitGov verificar que cada pull request haya sido auditado y aprobado según las políticas de tu organización.

---

## Estableciendo Evidencia de Cumplimiento

Al utilizar la trazabilidad de CI, puede generar informes automatizados para auditorías de cumplimiento:

- **Procedencia del Build**: Verificación de que un artefacto específico fue construido a partir de un commit específico en un servidor específico.
- **Cadenas de Aprobación**: Evidencia de que el código fue revisado por un par autorizado antes de la integración.
- **Estado del Entorno**: Visibilidad en tiempo real de qué commit está desplegado actualmente en Dev, Staging o Producción.

---

## Ejemplo de Configuración

Agrega un paso `post` a tu `Jenkinsfile` que llame directamente a la API REST del Control Plane:

```groovy
pipeline {
    agent any
    environment {
        GITGOV_URL    = 'http://tu-control-plane:3000'
        GITGOV_KEY    = credentials('gitgov-admin-api-key')
    }
    stages {
        stage('Build') {
            steps {
                // ... tus pasos de build existentes ...
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
                        "pipeline_id": "\${env.BUILD_TAG}",
                        "job_name": "\${env.JOB_NAME}",
                        "status": "\${result.toLowerCase()}",
                        "commit_sha": "\${env.GIT_COMMIT}",
                        "branch": "\${env.GIT_BRANCH}",
                        "repo_full_name": "TuOrg/TuRepo",
                        "duration_ms": \${currentBuild.duration},
                        "triggered_by": "\${env.BUILD_USER_ID ?: 'ci'}",
                        "timestamp": \${ts}
                      }'
                """
            }
        }
    }
}
```

Guarda la API key como credencial Jenkins (`gitgov-admin-api-key`) de tipo **Secret text**. El endpoint requiere un Bearer token con rol admin.

> [!IMPORTANT]
> **Garantía de Integridad**: Una vez que un build se vincula a un commit en GitGov, el registro queda bloqueado. Cualquier intento de "re-etiquetar" un build existente a un nuevo commit quedará registrado en el trail de auditoría.

---

## Resumen de Capacidades

| Función | Descripción |
|---------|-------------|
| **Vincular Commits a Builds** | Sepa exactamente qué build produjo un artefacto específico. |
| **Exportación de Logs** | Exporte informes completos en CSV/JSON para auditorías SOC2 o ISO. |
| **Detección de Drift** | Identifique si el código en producción difiere del último build gobernado. |

## Fin de la Documentación Principal

- [**Regresar al Inicio**](/)
- [**Contactar Soporte para Empresas**](/contact)
