---
title: Privacidad y Responsabilidad de Señales
description: Qué datos captura GitGov, qué significan las señales legalmente y los límites de responsabilidad para las organizaciones desplegantes.
order: 6
---

GitGov se construye sobre un principio fundamental: **solo metadatos, nunca código fuente**. Esta página explica exactamente qué se recopila, qué representan las señales y los límites explícitos de la responsabilidad de GitGov cuando los datos de señales se usan para informar decisiones organizacionales.

---

## Qué Captura GitGov

GitGov Desktop captura los siguientes campos por evento Git. Nada más.

| Campo | Ejemplo | Descripción |
|-------|---------|-------------|
| `event_type` | `commit`, `push` | La operación Git que ocurrió |
| `commit_sha` | `a3f8c2e` | Identificador que enlaza el evento a un commit específico |
| `branch` | `feat/auth` | Rama destino de la operación |
| `user_login` | `alice` | Identificador del autor Git (`git config user.name`) |
| `timestamp` | ISO 8601 | Cuándo ocurrió la operación en la máquina del desarrollador |
| `file_count` | `12` | Conteo de archivos en stage — **no sus nombres, no su contenido** |
| `repo_name` | `org/repo` | Identificador del repositorio |
| `client_version` | `0.1.0` | Versión de la app Desktop para compatibilidad de protocolo |

> **Garantía absoluta:** El contenido del código fuente, el contenido de archivos, el contenido de diffs, los cuerpos de mensajes de commit, contraseñas, secretos y valores de `.env` **nunca se transmiten** y nunca abandonan la estación de trabajo del desarrollador.

---

## Señales de Incumplimiento

El Control Plane puede generar **señales** — marcadores automatizados que indican una posible desviación de las políticas de gobernanza configuradas. Ejemplos:

- Un `successful_push` a una rama listada como `protected` en `gitgov.toml` por un usuario no incluido en `admins` ni en un grupo autorizado.
- Un commit fuera del horario operacional configurado (si se define una política de ventana horaria).
- Un push sin referencia a ticket de Jira correspondiente (si la política de cobertura de tickets está activa).

### Crítico: Carácter Consultivo

> [!IMPORTANT]
> **Las señales son observaciones computacionales. No son conclusiones jurídicas, determinaciones de RRHH ni hallazgos de mala conducta.**

Una señal significa: _"una regla configurada se activó en base a los metadatos disponibles."_ No establece:

- **Intención** — el desarrollador puede haber tenido una razón legítima.
- **Negligencia** — errores de configuración o desfase de reloj pueden producir falsos positivos.
- **Culpa** — la señal no tiene conocimiento de contexto más allá de los metadatos capturados.

GitGov no ofrece **ninguna garantía** — expresa o implícita — sobre la exactitud, integridad o idoneidad de los datos de señales para ningún propósito laboral, disciplinario o legal.

---

## Límites de Responsabilidad

### La responsabilidad de GitGov termina en la señal.

| Límite | Rol de GitGov | Rol de la Organización Desplegante |
|--------|--------------|-----------------------------------|
| **Captura de datos** | Captura metadatos según el esquema anterior | Debe informar a los desarrolladores que el monitoreo está activo |
| **Generación de señales** | Marca desviaciones de política según las reglas configuradas | Responsable de la exactitud de la configuración de políticas |
| **Interpretación de señales** | Ninguna — las señales no emiten juicio | Responsable de la revisión humana antes de cualquier acción |
| **Decisiones basadas en señales** | Ninguna — GitGov no toma decisiones | Asume plena responsabilidad legal por acciones de RRHH/disciplinarias |
| **Falsos positivos** | Proporciona herramientas para revisar y descartar señales | No debe actuar sobre señales sin revisar |
| **Derechos de interesados** | Proporciona endpoint de exportación (`POST /export`) | Actúa como responsable del tratamiento; gestiona solicitudes DSAR individuales |

### Las organizaciones desplegantes deben:

1. **Informar a los empleados** de que se capturan metadatos operacionales de Git antes del despliegue.
2. **Establecer la base jurídica** (intereses legítimos, obligación legal o contrato) para el tratamiento conforme a la legislación aplicable (RGPD Art. 6).
3. **Configurar las políticas con precisión** — un `gitgov.toml` mal configurado producirá señales incorrectas.
4. **Exigir revisión humana** antes de tomar cualquier acción basada en una señal.
5. **Cumplir la legislación laboral local** — en muchas jurisdicciones, la monitorización de empleados requiere consulta con el comité de empresa, notificación individual o aprobación regulatoria.

---

## Retención de Datos e Inmutabilidad

Los registros de eventos de auditoría son **append-only**. El sistema está diseñado para evitar la manipulación de registros históricos — un requisito básico para marcos de cumplimiento como SOC 2 e ISO 27001.

- Los registros no pueden modificarse ni eliminarse a través de la API estándar.
- El periodo de retención lo configura la organización desplegante.
- GitGov no impone un periodo máximo de retención; las organizaciones deben definir sus propias políticas de retención acordes con los principios de minimización de datos del RGPD.

---

## Referencia RGPD

Para despliegues en la UE:

| Concepto RGPD | Implementación |
|--------------|----------------|
| **Responsable del tratamiento** | La organización desplegante |
| **Encargado del tratamiento** | GitGov (software + operadores) |
| **Base jurídica** | Art. 6(1)(b) contrato, 6(1)(c) obligación legal, o 6(1)(f) intereses legítimos |
| **Minimización de datos** | Solo metadatos operacionales — sin contenido, sin diffs |
| **Derecho de acceso** | `GET /logs?user_login={user}` limitado a datos propios para el rol Developer |
| **Portabilidad** | `POST /export` — exportación JSON completa de registros de eventos |
| **Supresión** | Sujeto a las obligaciones de retención de pistas de auditoría de la organización |

---

## Resumen

- GitGov captura metadatos, no código.
- Las señales marcan desviaciones de política — son entradas consultivas, no conclusiones.
- La organización desplegante es el responsable del tratamiento y asume plena responsabilidad sobre el uso que hace de las señales.
- Aplica siempre criterio humano antes de actuar sobre una señal.
- Revisa los requisitos de monitorización de empleados de tu jurisdicción antes del despliegue.

## Relacionado

- [**Política de Privacidad**](/privacy) — Términos legales para usuarios finales y organizaciones.
- [**Gobernanza y Políticas**](/docs/governance) — Cómo configurar `gitgov.toml`.
- [**Conectar al Control Plane**](/docs/control-plane) — Arquitectura de autenticación y flujo de datos.
