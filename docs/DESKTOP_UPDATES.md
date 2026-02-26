# Desktop Updates (Tauri 2) - GitGov

Guía operativa para actualizaciones in-app del Desktop (`GitGov`) usando `tauri-plugin-updater` con **full updates** (sin deltas) y distribución por **S3 + CloudFront**.

## Objetivo

- Notificar al usuario cuando existe una nueva versión
- Descargar e instalar update desde la app
- Mantener fallback de descarga manual
- Preparar base para `stable` / `beta` y políticas de compatibilidad

## Estado actual (implementado)

- `tauri-plugin-updater` integrado en Desktop (`src-tauri`)
- UI en `Configuración > Actualizaciones Desktop`
- `Buscar actualizaciones` manual
- Auto-check al iniciar (con throttling local ~6h)
- Changelog simple (campo `body` del manifest)
- Fallback manual (URL configurable)

## Importante (requisito para producción)

El updater **no funcionará** hasta configurar en `tauri.conf.json`:

- `plugins.updater.endpoints`
- `plugins.updater.pubkey`

Además debes firmar el update con la clave del updater de Tauri.

## Arquitectura recomendada (AWS)

- **S3**: almacenar artefactos y manifests
- **CloudFront**: servir con HTTPS y CDN
- Canales:
  - `stable`
  - `beta` (posterior)

> Nota (Fase 2): el Desktop puede enviar el header `x-gitgov-update-channel` (`stable`/`beta`) al verificar/descargar updates. Para que esto cambie el canal real, el endpoint configurado en `plugins.updater.endpoints` debe **respetar ese header** (por ejemplo con un proxy/CloudFront Function) o debes usar builds con endpoint distinto por canal.

### Estructura sugerida

```txt
s3://gitgov-downloads/desktop/
  stable/
    latest.json
    GitGov_0.1.1_x64-setup.exe
    GitGov_0.1.1_x64-setup.exe.sig
  beta/
    latest.json
    GitGov_0.1.2-beta.1_x64-setup.exe
    GitGov_0.1.2-beta.1_x64-setup.exe.sig
```

CloudFront (ejemplo):

- `https://downloads.gitgov.com/desktop/stable/latest.json`
- `https://downloads.gitgov.com/desktop/stable/GitGov_0.1.1_x64-setup.exe`

## Manifest de updater (Tauri)

El updater de Tauri espera un JSON firmado con metadatos de la release.

### Ejemplo (Windows)

Ver también: `docs/examples/desktop-updater/latest.stable.windows.json`

Campos principales:

- `version`: versión nueva
- `notes`: changelog corto (opcional)
- `pub_date`: fecha
- `platforms."windows-x86_64".url`: URL del instalador
- `platforms."windows-x86_64".signature`: firma `.sig` del artefacto

## Configuración `tauri.conf.json` (ejemplo)

**No pegues valores dummy en producción**. Usa tus endpoints reales y `pubkey` del updater.

Ver snippet listo: `docs/examples/desktop-updater/tauri.updater.config.snippet.json`

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://downloads.gitgov.com/desktop/stable/latest.json"
      ],
      "pubkey": "TU_PUBLIC_KEY_DEL_UPDATER"
    }
  }
}
```

### ¿Por qué hay placeholders y qué va ahí?

Porque esos valores **son específicos de tu infraestructura de releases** y no deben hardcodearse como ejemplo “real” en el repo.

- `https://downloads.gitgov.com/desktop/stable/latest.json`
  - Es **tu endpoint real de updates** (manifiesto del canal).
  - En tu caso debe apuntar a **CloudFront/S3** (o el dominio que uses para descargas).
  - Ejemplos válidos:
    - `https://downloads.gitgov.com/desktop/stable/latest.json`
    - `https://downloads.gitgov.com/desktop/beta/latest.json`

- `REEMPLAZAR_CON_PUBLIC_KEY_DEL_UPDATER`
  - Es la **clave pública del updater de Tauri** (no la privada).
  - Va embebida en la app para verificar que el `latest.json` / artefacto fue firmado por ustedes.
  - Se obtiene cuando generas el par de claves del updater (ver sección de firma).

### ¿Qué son las claves de firma del updater?

El updater de Tauri usa un **par de claves** (asimétricas):

- **Clave privada (secreta)**
  - La usa tu proceso de release para **firmar** cada update (`.sig`)
  - Debe vivir solo en:
    - tu máquina de release segura, o
    - secrets del pipeline CI/CD
  - **Nunca** se commitea ni se distribuye

- **Clave pública**
  - Se coloca en `tauri.conf.json` (`plugins.updater.pubkey`)
  - La app la usa para **verificar** la firma del update antes de instalar
  - Sí puede estar en el repo (no es secreta)

### Importante: esto NO es lo mismo que code signing de Windows

Son dos firmas distintas:

- **Firma del updater (Tauri)**: valida la autenticidad del update dentro de la app
- **Code signing de Windows**: mejora SmartScreen / confianza del instalador

Lo correcto es usar **ambas** en producción.

## Release flow (full update)

1. Incrementar versión en `gitgov/src-tauri/tauri.conf.json`
2. Build release del Desktop (`tauri build`)
3. Generar firma del updater (`.sig`) con la clave privada del updater
4. Crear/actualizar `latest.json` del canal (`stable`)
5. Subir `.exe`, `.sig` y `latest.json` a S3
6. Invalidar CloudFront (si aplica)
7. Probar desde una versión anterior instalada

## Comandos de firma (CLI Tauri)

### 1) Generar par de claves del updater (una sola vez)

Desde `gitgov/`:

```powershell
npx tauri signer generate --ci -p "TU_PASSWORD_FUERTE" --write-keys .\secrets\tauri-updater.key
```

Resultado esperado:

- clave privada (archivo local)
- clave pública (copiarla y guardarla en `tauri.conf.json` -> `plugins.updater.pubkey`)

**No commitear** la clave privada.

Si ya generaste una clave sin password (válida para pruebas), puedes regenerarla de forma segura:

```powershell
npx tauri signer generate --ci -p "TU_PASSWORD_FUERTE" --write-keys .\secrets\tauri-updater.key --force
```

### 2) Firmar el instalador `.exe`

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = ".\secrets\tauri-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "TU_PASSWORD"
npx tauri signer sign .\src-tauri\target\release\bundle\nsis\GitGov_0.1.1_x64-setup.exe
```

Esto genera un `.sig` (usar ese contenido en `latest.json` o como archivo separado según tu flujo).

## Scripts helper (PowerShell)

### Generar manifest `latest.json`

Script:

- `scripts/release/desktop-updater/New-TauriUpdaterManifest.ps1`

Ejemplo:

```powershell
.\scripts\release\desktop-updater\New-TauriUpdaterManifest.ps1 `
  -Version "0.1.1" `
  -Url "https://downloads.gitgov.com/desktop/stable/GitGov_0.1.1_x64-setup.exe" `
  -Signature "PEGA_AQUI_LA_FIRMA" `
  -Notes "Fixes de rendimiento y limpieza de repos caóticos" `
  -OutputPath ".\release\desktop\stable\latest.json"
```

### Publicar a S3 (+ invalidar CloudFront opcional)

Script:

- `scripts/release/desktop-updater/Publish-DesktopUpdateAws.ps1`

Ejemplo:

```powershell
.\scripts\release\desktop-updater\Publish-DesktopUpdateAws.ps1 `
  -ExePath ".\src-tauri\target\release\bundle\nsis\GitGov_0.1.1_x64-setup.exe" `
  -SigPath ".\release\desktop\stable\GitGov_0.1.1_x64-setup.exe.sig" `
  -ManifestPath ".\release\desktop\stable\latest.json" `
  -Bucket "gitgov-downloads" `
  -Channel "stable" `
  -CloudFrontDistributionId "E123ABC456DEF"
```

### Generar snippet real de `plugins.updater` (sin placeholders manuales)

Script:

- `scripts/release/desktop-updater/New-TauriUpdaterConfigSnippet.ps1`

Ejemplo:

```powershell
.\scripts\release\desktop-updater\New-TauriUpdaterConfigSnippet.ps1 `
  -Channel "stable" `
  -BaseUrl "https://downloads.gitgov.com/desktop" `
  -PubKey "PEGA_AQUI_LA_PUBLIC_KEY_REAL" `
  -OutputPath ".\release\desktop\tauri.updater.stable.json"
```

Eso te genera un JSON listo para copiar el bloque `plugins.updater` a `gitgov/src-tauri/tauri.conf.json`.

## Firma del updater (Tauri)

Debes generar y guardar las claves del updater de forma segura.

- Clave privada: solo en pipeline/entorno de release
- Clave pública (`pubkey`): en `tauri.conf.json`

Recomendado:

- Local dev: clave privada fuera del repo (ej. `C:\secure\gitgov\tauri-updater.key`)
- CI/CD: secrets del pipeline (no archivos versionados)

No reutilizar la clave de code-signing de Windows como sustituto de la firma del updater de Tauri.

## Code signing de Windows (separado, pero recomendado)

Esto mejora SmartScreen y confianza del instalador:

- Firmar `GitGov_*.exe` / instalador con certificado de code signing
- Mantener timestamping

## Fallback manual (recomendado)

Mantener un link visible a descarga manual siempre:

- Web pública (`gitgov-web`) página `/download`
- o URL directa de CloudFront

Esto cubre:

- updater no configurado
- error de firma
- error de red / proxy corporativo

## Problemas comunes

### “Updater no configurado”

Causa:

- Falta `plugins.updater` en `tauri.conf.json`
- Falta `endpoints`
- Falta `pubkey`

### “No se pudo verificar / instalar”

Revisar:

- URL `latest.json` accesible
- `signature` coincide con el artefacto
- `pubkey` correcto
- HTTPS / proxy corporativo

### Usuario no ve notificación

Revisar:

- Está en Desktop (Tauri), no en navegador
- Auto-check tiene throttling (~6h)
- Probar `Buscar actualizaciones` manual en Settings

## Próximas fases (roadmap)

### Fase 2

- Canales `beta/stable`
- Telemetría de éxito/fallo del updater
- Reintento de descarga

### Fase 3

- `min_supported_version` desde backend
- Forced updates (solo críticos)
- Delatas (si de verdad se necesitan)
