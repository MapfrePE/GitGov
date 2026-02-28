---
title: Conectar al Control Plane
description: Configura la capa de sincronización entre tu agente de captura local y el servidor de gobernanza central.
order: 3
---

El Control Plane es el corazón del ecosistema GitGov. Actúa como punto de ingestión centralizado y seguro para los eventos capturados por todos los agentes Desktop de tu organización. Una vez conectado, habilita monitorización en tiempo real, auditoría y aplicación global de políticas.

---

## Autenticación

GitGov Desktop se comunica con el Control Plane mediante una REST API autenticada con un **token Bearer**. El token proviene de una API key que tu administrador genera a través del endpoint `/api-keys` del Control Plane.

```
Authorization: Bearer <api-key>
```

> [!IMPORTANT]
> Usa siempre el formato de cabecera `Authorization: Bearer`. La cabecera `X-API-Key` **no está soportada** y resultará en una respuesta `401 Unauthorized`.

---

## Fundamentos de Conexión

GitGov Desktop se comunica mediante una REST API de alto rendimiento. En entornos de producción, esta conexión debe protegerse con TLS (HTTPS).

La aplicación Desktop se conecta automáticamente al arrancar usando la URL del servidor configurada por tu administrador. Tu equipo de DevOps te proporcionará la dirección correcta del servidor para tu organización.

---

## Flujo de Configuración

### 1. Verificar Estado del Servidor
Asegúrate de que el servicio del Control Plane está activo y accesible. Tu administrador puede confirmar mediante el endpoint de salud:

```bash
curl http://tu-control-plane/health
# Esperado: {"status":"ok", ...}
```

### 2. Autenticación en Desktop
1. Inicia **GitGov Desktop**.
2. Ve a **Configuración > Sync y Control Plane**.
3. Ingresa la **URL del Servidor** proporcionada por tu equipo de DevOps.
4. Ingresa tu **Token API**. La aplicación verificará la conexión de inmediato.

### 3. Handshake de Conexión
GitGov realiza un health check ligero para verificar latencia y compatibilidad de protocolo. Un indicador verde confirma una conexión exitosa.

---

## Comportamiento de Sincronización

| Comportamiento | Detalles |
|----------------|---------|
| **Despacho de Eventos** | Los eventos se despachan al Control Plane a medida que ocurren, en lotes mediante el endpoint `/events`. |
| **Actualización del Dashboard** | El dashboard del Control Plane se actualiza automáticamente cada **30 segundos**. |
| **Buffer Offline** | Cuando el servidor no está disponible, la bandeja de salida local encola los eventos en un archivo JSONL en disco. |
| **Reintentos** | Los despachos fallidos usan backoff exponencial, con un máximo de **32×** el intervalo base. Los eventos nunca se pierden. |
| **Límite de Tasa** | Por defecto: **240 eventos/minuto** por API key. Configurable mediante `GITGOV_RATE_LIMIT_EVENTS_PER_MIN`. |

---

## Control de Acceso Basado en Roles

El Control Plane aplica control de acceso basado en roles en todos los endpoints autenticados:

| Rol | Acceso |
|-----|--------|
| **Admin** | Acceso completo — estadísticas, dashboard, integraciones, gestión de políticas, todos los eventos |
| **Developer** | Acceso limitado — solo ve sus propios eventos en `/logs` |
| **Architect** | Reservado para futuras restricciones de rol |
| **PM** | Reservado para futuras restricciones de rol |

Las API keys llevan asignado un rol. Asegúrate de que los desarrolladores tengan keys con rol `Developer` y que el equipo de DevOps/seguridad tenga rol `Admin`.

---

## Privacidad de Datos

GitGov solo sincroniza metadatos: tipo de evento, commit SHA, nombre de rama, login del autor, timestamp y conteo de archivos. **El contenido del código fuente nunca abandona la estación de trabajo del desarrollador.** Los contenidos de diffs y archivos no se transmiten.

---

## Requisitos de Red

- **Protocolo**: HTTP/1.1 o HTTP/2.
- **Puerto**: Por defecto `3000` (configurable mediante `GITGOV_SERVER_ADDR` en el lado del servidor).
- **Firewall**: Permite el tráfico saliente desde las estaciones de trabajo al host del Control Plane en el puerto configurado.
- **Producción**: Se recomienda encarecidamente TLS (HTTPS). HTTP solo se admite para evaluación local.

## Siguiente Fase

- [**Configurar Políticas de Gobernanza**](/docs/governance) — Define las reglas del camino.
