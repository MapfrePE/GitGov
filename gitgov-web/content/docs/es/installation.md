---
title: Instalación de GitGov Desktop
description: Despliegue el agente de captura GitGov en su entorno local y comience a rastrear las operaciones de ingeniería.
order: 2
---

La aplicación GitGov Desktop es el agente de captura fundamental para el ecosistema. Se ejecuta discretamente en segundo plano, monitoreando sus directorios locales de Git y proporcionando feedback instantáneo sobre el cumplimiento de políticas.

## Requisitos Previos del Sistema

Antes de proceder con la instalación, asegúrese de que su estación de trabajo cumpla con los siguientes requisitos técnicos:

- **Sistema Operativo**: Windows 10 o 11 (64 bits).
- **Versión de Git**: 2.30 o superior (debe estar en el PATH del sistema).
- **Permisos**: Derechos de administrador para la configuración inicial e instalación del servicio en segundo plano.
- **Memoria**: Mínimo 2GB RAM (GitGov consume ~50MB en reposo).

---

## Adquisición y Despliegue

### 1. Obtener el Instalador
Diríjase al [Portal de Descargas de GitGov](/download) y seleccione el paquete `.exe` más reciente para Windows.

### 2. Ejecutar la Instalación
Haga doble clic en el binario descargado.

> [!IMPORTANT]
> **Aviso de Seguridad**: Durante la fase de acceso anticipado/desarrollo, el instalador puede activar Windows SmartScreen. Haga clic en **"Más información"** seguido de **"Ejecutar de todas formas"** para continuar. Actualmente estamos trabajando en certificados de firma EV globales.

### 3. Asistente de Instalación
Siga las instrucciones en pantalla. GitGov se instala de forma predeterminada en `%LOCALAPPDATA%/Programs/gitgov`. Se recomienda mantener esta ruta para asegurar que las actualizaciones automáticas funcionen correctamente.

---

## Configuración Post-Instalación

Una vez instalado, GitGov se iniciará automáticamente. Complete los siguientes tres pasos para inicializar el agente de captura:

### A. Descubrimiento del Entorno
La aplicación intentará localizar su ejecutable `git.exe` y cualquier identidad SSH. Si Git está instalado en una ubicación no estándar, puede sobrescribir esto manualmente en **Settings > Advanced**.

### B. Vinculación con el Control Plane
Debe proporcionar la URL del Control Plane de su organización. Si está realizando pruebas locales, el valor predeterminado es:
`http://127.0.0.1:3000`

### C. Indexación del Espacio de Trabajo
GitGov solicitará permiso para buscar repositorios Git en su disco. Puede elegir entre:
- **Auto-Detectar**: Escanea carpetas de desarrollo comunes (ej. `C:/Users/PC/Desktop`, `C:/GitHub`).
- **Selección Manual**: Especifique directorios individuales para rastrear.

---

## Verificación Operativa

Para confirmar que el agente de captura funciona correctamente, realice un "Push Canario":

1. Abra su terminal preferida (PS, Git Bash, CMD).
2. Navegue a un repositorio rastreado.
3. Cree un commit: `git commit -m "chore: test capture"`
4. Cambie a la UI de GitGov Desktop. Debería ver una nueva entrada en el feed de **Live Events** en milisegundos.

## Solución de Problemas Comunes

| Problema | Causa Potencial | Resolución |
|-------|-----------------|------------|
| "Git no encontrado" | Git no está en el PATH | Verifique que `git --version` funcione. Ajuste las variables de entorno si es necesario. |
| Tiempo de espera agotado | Interferencia de VPN o Firewall | Asegúrese de que el puerto 3000 esté en la lista blanca de entrada/salida. |
| Alto uso de CPU | Escaneo de disco agresivo | Excluya `node_modules` o carpetas de build en **Settings > Ignored Paths**. |

## Siguiente Fase

- [**Conectar al Control Plane**](/docs/control-plane) — Finalice la capa de sincronización.
