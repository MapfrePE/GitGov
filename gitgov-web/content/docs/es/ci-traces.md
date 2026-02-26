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
GitGov proporciona un plugin ligero para Jenkins (y una librería compartida) que reporta automáticamente eventos de construcción al Control Plane.
- **Inyección Automática de Metadatos**: Inyecta la URL del build y el nombre del job en el flujo de eventos de GitGov.
- **Análisis de Fallos**: Correlaciona cambios específicos de código con regresiones en la construcción.

### Webhooks de GitHub
Conecte sus repositorios para recibir eventos de push y pull request en tiempo real. Esto permite a GitGov verificar que cada pull request haya sido auditado y aprobado según las políticas de su organización.

---

## Estableciendo Evidencia de Cumplimiento

Al utilizar la trazabilidad de CI, puede generar informes automatizados para auditorías de cumplimiento:

- **Procedencia del Build**: Verificación de que un artefacto específico fue construido a partir de un commit específico en un servidor específico.
- **Cadenas de Aprobación**: Evidencia de que el código fue revisado por un par autorizado antes de la integración.
- **Estado del Entorno**: Visibilidad en tiempo real de qué commit está desplegado actualmente en Dev, Staging o Producción.

---

## Ejemplo de Configuración

Para habilitar la trazabilidad de CI en su `Jenkinsfile`, simplemente añada el wrapper de GitGov:

```groovy
pipeline {
    agent any
    stages {
        stage('Audit') {
            steps {
                // Notifica a GitGov que este build está iniciando para un commit específico
                gitgovNotify(status: 'STARTING', serverUrl: 'https://gitgov.internal')
            }
        }
        // ... pasos de build ...
    }
    post {
        always {
            gitgovNotify(status: currentBuild.result)
        }
    }
}
```

> [!IMPORTANT]
> **Garantía de Integridad**: Una vez que un build se vincula a un commit en GitGov, el registro queda bloqueado. Cualquier intento de "volver a etiquetar" un build existente a un nuevo commit activará una alerta de seguridad de alta prioridad.

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
