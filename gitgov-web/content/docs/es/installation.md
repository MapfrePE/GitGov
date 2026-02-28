---
title: Instalación de GitGov Desktop
description: Despliega el agente de captura GitGov en tu entorno local y comienza a rastrear las operaciones de ingeniería.
order: 2
---

La aplicación GitGov Desktop es el agente de captura fundamental del ecosistema. Se ejecuta en segundo plano, monitorizando tus operaciones Git activas y proporcionando feedback en tiempo real sobre el cumplimiento de políticas.

## Requisitos del Sistema

Antes de proceder con la instalación, asegúrate de que tu estación de trabajo cumple los siguientes requisitos técnicos:

- **Sistema Operativo**: Windows 10 u 11 (64 bits).
- **Versión de Git**: 2.30 o superior (debe estar en el PATH del sistema).
- **Permisos**: Derechos de administrador para la configuración inicial.
- **Memoria**: Mínimo 2 GB de RAM (GitGov usa ~50 MB cuando está inactivo).

---

## Adquisición e Instalación

### 1. Obtener el Instalador
Ve al [Portal de Descarga de GitGov](/download) y selecciona el paquete `.exe` más reciente para Windows.

### 2. Ejecutar la Instalación
Haz doble clic en el binario descargado.

> [!IMPORTANT]
> **Aviso de Seguridad**: Durante la fase de acceso anticipado, el instalador puede activar Windows SmartScreen. Haz clic en **"Más información"** y luego en **"Ejecutar de todas formas"** para continuar. Los certificados de firma de código están en proceso.

### 3. Asistente de Instalación
Sigue las instrucciones en pantalla. GitGov se instala por defecto en `%LOCALAPPDATA%\Programs\GitGov`. Mantén esta ruta predeterminada para garantizar que las actualizaciones automáticas funcionen correctamente.

---

## Configuración Post-Instalación

Una vez instalado, GitGov se iniciará automáticamente. Completa los siguientes pasos para inicializar el agente de captura:

### A. Detección de Git
La aplicación localiza tu `git.exe` desde el PATH del sistema. Si Git está instalado pero no se detecta, verifica que `git --version` funciona en una terminal CMD o PowerShell y que Git está presente en tu variable de entorno `PATH`.

### B. Conexión al Control Plane
Proporciona la URL del servidor Control Plane de tu organización. Tu equipo de DevOps te facilitará esta dirección. La aplicación se conecta automáticamente al arrancar — no se requiere ninguna acción manual una vez guardada la URL.

---

## Qué Captura GitGov

Una vez conectado, GitGov captura los siguientes eventos Git automáticamente:

| Evento | Disparador |
|--------|------------|
| `stage_files` | Archivos añadidos al índice Git (`git add`) |
| `commit` | Se crea un nuevo commit (incluye SHA, mensaje, autor y rama) |
| `attempt_push` | Se inicia un push |
| `successful_push` | El push se completa con éxito |
| `blocked_push` | Push rechazado por política de protección de rama |
| `push_failed` | El push falla (red, rechazo remoto, etc.) |

> **Nota**: En repositorios con un gran número de archivos en stage, el evento `stage_files` captura un máximo de 500 entradas. Se establece una bandera `truncated` en los metadatos del evento cuando se alcanza este límite.

---

## Verificación Operacional

Para confirmar que el agente de captura funciona correctamente, realiza un "Push de Prueba":

1. Abre una terminal (PowerShell, Git Bash o CMD).
2. Navega a un repositorio Git.
3. Crea y confirma un cambio: `git add . && git commit -m "chore: test capture"`
4. Cambia a la interfaz de GitGov Desktop. Deberías ver los eventos `stage_files` y `commit` en el feed de **Eventos en Vivo** en milisegundos.

---

## Solución de Problemas Comunes

| Problema | Causa Probable | Resolución |
|----------|----------------|------------|
| "Git no encontrado" | Git no está en el PATH del sistema | Verifica que `git --version` funciona en CMD/PS. Ajusta el `PATH` si es necesario. |
| Tiempo de conexión agotado | Firewall o VPN bloqueando el puerto 3000 | Asegúrate de que el tráfico saliente al host del Control Plane en el puerto 3000 esté permitido. |
| Los eventos no aparecen | Control Plane no está ejecutándose | Verifica que el servidor está activo. Comprueba el indicador de estado de conexión en la app Desktop. |
| Advertencia de SmartScreen | Sin certificado de firma de código | Haz clic en "Más información" → "Ejecutar de todas formas". Es normal durante el acceso anticipado. |

## Siguiente Fase

- [**Conectar al Control Plane**](/docs/control-plane) — Finaliza la capa de sincronización.
