---
title: Conectar al Control Plane
description: Configure la capa de sincronización entre su agente de captura local y el servidor de gobernanza central.
order: 3
---

El Control Plane es el corazón del ecosistema GitGov. Actúa como un punto de ingesta centralizado y seguro para los eventos capturados por todos los agentes Desktop en su organización. Una vez conectado, permite el monitoreo en tiempo real, el registro de auditoría y la aplicación de políticas globales.

---

## Fundamentos de la Conexión

GitGov Desktop se comunica con el Control Plane a través de una API REST de alto rendimiento. Para entornos de producción, esta conexión suele estar protegida mediante TLS (HTTPS) y un token de API rotativo.

### Endpoint Estándar
Por defecto, durante el desarrollo o la evaluación local, el Control Plane escucha en:
`http://127.0.0.1:3000`

---

## Flujo de Configuración

Siga estos pasos para establecer un enlace seguro:

### 1. Verificación del Estado del Servidor
Asegúrese de que el servicio del Control Plane esté activo. Si está ejecutando el servidor manualmente:
1. Abra una terminal en el directorio `gitgov-server`.
2. Verifique que Rust esté inicializado.
3. Inicie el servicio: `cargo run`.

### 2. Autenticación en Desktop
1. Inicie **GitGov Desktop**.
2. Navegue a **Settings > Sync & Control Plane**.
3. Ingrese la **URL del Servidor** proporcionada por su equipo de DevOps.
4. Ingrese su **Security Token** (si su organización lo requiere).

### 3. Prueba de Conexión
Haga clic en el botón **"Test Connection"**. GitGov realizará una verificación de salud ligera para validar la latencia y la compatibilidad de protocolos.

---

## Ajustes Avanzados de Sincronización

Puede ajustar cómo se envían los datos al servidor para equilibrar la visibilidad en tiempo real y la carga de red.

| Ajuste | Recomendación | Descripción |
|---------|----------------|-------------|
| **Intervalo de Sinc** | 5s - 15s | Frecuencia de envío. Menor para entornos de alta seguridad. |
| **Tamaño de Lote** | 100 eventos | Evita que envíos grandes saturen el ancho de banda local. |
| **Buffer Offline** | Activado | Almacena eventos localmente si el servidor no está disponible. |
| **Lógica de Reintento** | Exponencial | Reintenta envíos fallidos automáticamente con retrasos crecientes. |

> [!IMPORTANT]
> **Privacidad de Datos**: GitGov solo sincroniza metadatos (hashes, marcas de tiempo, nombres de ramas y resúmenes de diff). Su código fuente real (el contenido de los archivos) **nunca** sale de su estación de trabajo a menos que se configure específicamente para auditorías de seguridad profundas.

---

## Requisitos de Red

Para asegurar una sincronización estable, su entorno de red debe permitir:
- **Protocolo**: HTTP/1.1 o HTTP/2.
- **Puerto**: Predeterminado 3000 (Personalizable mediante la variable de entorno `GITGOV_SERVER_ADDR`).
- **Lista Blanca de Dominios**: Asegúrese de que su firewall local permita tráfico saliente al dominio del Control Plane.

## Siguiente Fase

- [**Configurar Políticas de Gobernanza**](/docs/governance) — Aprenda a establecer las reglas del camino.
