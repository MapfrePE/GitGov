# GitGov — Deployment Guide

> Guía unificada: Docker local, AWS EC2, Enterprise (instaladores/GPO) y Desktop Updates.
> Última actualización: 2026-03-14

---

## 1. Docker Local (desarrollo/demo)

Setup Docker local para levantar:
- PostgreSQL (`gitgov-db`)
- GitGov Control Plane Server (`gitgov-server`)
- Jenkins (opcional, perfil `jenkins`)
- Jira Software (opcional, perfil `jira`)

No reemplaza tu app Desktop/Tauri local. La idea es correr el **server** en Docker y seguir usando GitGov Desktop como cliente.

### Requisitos

- Docker Desktop ejecutándose
- Puerto `3001` libre (GitGov server Docker)
- Puerto `5433` libre (Postgres Docker)

### Levantar stack

```bash
# Desde la raíz del repo
docker compose up --build -d

# Ver estado
docker compose ps

# Logs
docker compose logs -f gitgov-server
docker compose logs -f gitgov-db
```

### Jenkins (opcional)

```bash
docker compose --profile jenkins up -d jenkins
docker compose logs -f jenkins
# URL: http://localhost:8096
# Password inicial:
docker exec -it gitgov-jenkins cat /var/jenkins_home/secrets/initialAdminPassword
```

### Jira (opcional)

```bash
docker compose --profile jira up -d jira
docker compose logs -f jira
# URL: http://localhost:8095
```

### Qué inicializa automáticamente

Al crear el volumen de Postgres por primera vez, Docker ejecuta:
1. `gitgov/gitgov-server/supabase_schema.sql`
2. `gitgov/gitgov-server/supabase/supabase_schema_v4.sql`
3. `gitgov/gitgov-server/supabase/supabase_schema_v5.sql`
4. `gitgov/gitgov-server/supabase/supabase_schema_v6.sql`

Si ya existe el volumen, los scripts **no** se vuelven a ejecutar.

### URLs y credenciales (dev local)

| Recurso | Valor |
|---------|-------|
| Server Docker | `http://127.0.0.1:3001` |
| API Key admin (dev) | `<YOUR_API_KEY>` |
| PostgreSQL host | `localhost:5433` |
| PostgreSQL db/user | `gitgov` / `gitgov` |
| PostgreSQL password | `gitgov_dev_password` |

### Integrar con Desktop App

En la configuración del Control Plane:
- URL: `http://127.0.0.1:3001` (server Docker)
- API Key: `<YOUR_API_KEY>`

> **Golden Path diario (server local nativo):** usar `http://127.0.0.1:3000` para evitar split-brain.

### Reset de base local

```bash
docker compose down -v
docker compose up --build -d
```

### Probar endpoints

```bash
curl http://127.0.0.1:3001/health
curl -H "Authorization: Bearer <YOUR_API_KEY>" http://127.0.0.1:3001/stats
```

### Migraciones adicionales recomendadas (governance/drift v2)

El bootstrap Docker ejecuta automáticamente `supabase_schema.sql` + `v4..v6`.
Para usar toda la superficie reciente (drift audit + policy requests + timeline compliance), aplicar también `v7..v18` una vez:

```bash
# Desde la raíz del repo
for v in 7 8 9 10 11 12 13 14 15 16 17 18; do
  cat "gitgov/gitgov-server/supabase/supabase_schema_v${v}.sql" \
    | docker exec -i gitgov-db psql -U gitgov -d gitgov
done
```

---

## 2. AWS EC2 + Supabase (producción actual)

### Arquitectura

- EC2 Ubuntu 22.04
- Nginx como reverse proxy
- systemd para el backend
- Supabase como PostgreSQL remoto (sin RDS)

### Perfil objetivo 250 simultáneos (sin tocar UI)

Topología recomendada:
- **3 instancias** `gitgov-server` (mismo build) detrás de un balanceador L7 (Nginx upstream o ALB).
- URL pública única para Desktop/dashboard.
- Supabase PostgreSQL compartido.

Contrato operativo:
- No cambiar contratos HTTP de `/events`, `/logs`, `/stats`, `/chat/ask`, `/sse`.
- Mantener Golden Path: Desktop -> `/events` -> DB -> Dashboard.

Ejemplo Nginx upstream (3 nodos backend):

```nginx
upstream gitgov_backend {
    least_conn;
    server 127.0.0.1:3000 max_fails=3 fail_timeout=10s;
    server 127.0.0.1:3002 max_fails=3 fail_timeout=10s;
    server 127.0.0.1:3003 max_fails=3 fail_timeout=10s;
}

server {
    listen 80;
    server_name _;

    location / {
        proxy_pass http://gitgov_backend;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Connection "";
        proxy_buffering off; # SSE
        proxy_read_timeout 75s;
    }
}
```

Variables recomendadas por instancia (perfil inicial 250):

```env
GITGOV_DB_MAX_CONNECTIONS=30
GITGOV_DB_MIN_CONNECTIONS=6
GITGOV_DB_ACQUIRE_TIMEOUT_SECS=12
GITGOV_RATE_LIMIT_EVENTS_PER_MIN=1500
GITGOV_RATE_LIMIT_ADMIN_PER_MIN=240
GITGOV_RATE_LIMIT_LOGS_PER_MIN=900
GITGOV_RATE_LIMIT_STATS_PER_MIN=900
GITGOV_RATE_LIMIT_CHAT_PER_MIN=180
GITGOV_CHAT_LLM_MAX_CONCURRENCY=8
GITGOV_CHAT_LLM_QUEUE_TIMEOUT_MS=1200
GITGOV_CHAT_LLM_TIMEOUT_MS=12000
GITGOV_ORG_LOOKUP_CACHE_TTL_MS=30000
GITGOV_REPO_LOOKUP_CACHE_TTL_MS=30000
GITGOV_REPO_UPSERT_MIN_INTERVAL_MS=30000
GITGOV_CACHE_INVALIDATION_MIN_INTERVAL_MS=120
GITGOV_STATS_CACHE_INVALIDATION_MIN_INTERVAL_MS=5000
GITGOV_LOGS_CACHE_INVALIDATION_MIN_INTERVAL_MS=500
GITGOV_CLIENT_SESSION_UPSERT_MIN_INTERVAL_MS=15000
GITGOV_SSE_MAX_CONNECTIONS=120
GITGOV_SSE_DISTRIBUTED_ENABLED=true
GITGOV_SSE_DISTRIBUTED_CHANNEL=gitgov_sse_events
GITGOV_RATE_LIMIT_DISTRIBUTED_DB=true
GITGOV_RATE_LIMIT_DISTRIBUTED_PRUNE_INTERVAL_SECS=60
GITGOV_RATE_LIMIT_DISTRIBUTED_RETENTION_SECS=3600
GITGOV_OUTBOX_SERVER_LEASE_ENABLED=true
GITGOV_OUTBOX_SERVER_LEASE_TTL_MS=2000
```

Notas:
- `GITGOV_SSE_DISTRIBUTED_ENABLED=true` habilita fan-out cross-node en `/sse` vía PostgreSQL `NOTIFY`.
- `GITGOV_RATE_LIMIT_DISTRIBUTED_DB=true` evita inconsistencia de cuotas cuando hay múltiples nodos.
- `GITGOV_ORG_LOOKUP_CACHE_TTL_MS` y `GITGOV_REPO_LOOKUP_CACHE_TTL_MS` reducen round-trips de lookups repetidos en `/events`.
- `GITGOV_REPO_UPSERT_MIN_INTERVAL_MS` mueve alta cardinalidad de `upsert_repo` fuera del camino síncrono de `/events` (debounced background).
- `GITGOV_CACHE_INVALIDATION_MIN_INTERVAL_MS` reduce churn de lock/cache en ráfagas de `/events`.
- `GITGOV_CLIENT_SESSION_UPSERT_MIN_INTERVAL_MS` evita escrituras redundantes a `client_sessions` por cada request.
- Política de degradación: bajo presión se degrada primero `chat` (`429`), preservando `/events` `/logs` `/stats`.

### Perfil productivo validado (single-node, 2026-03-15)

Configuración actualmente validada en EC2 `t3.small` con PostgreSQL local:

```env
DATABASE_URL=postgresql://gitgov:<password>@127.0.0.1:5432/gitgov
GITGOV_DB_MAX_CONNECTIONS=60
GITGOV_DB_MIN_CONNECTIONS=10
GITGOV_RATE_LIMIT_EVENTS_PER_MIN=60000
GITGOV_RATE_LIMIT_ADMIN_PER_MIN=3000
GITGOV_RATE_LIMIT_LOGS_PER_MIN=6000
GITGOV_RATE_LIMIT_STATS_PER_MIN=6000
GITGOV_RATE_LIMIT_CHAT_PER_MIN=6000
GITGOV_STATS_CACHE_TTL_MS=15000
GITGOV_LOGS_CACHE_TTL_MS=10000
GITGOV_AUTH_CACHE_TTL_SECS=120
GITGOV_CACHE_INVALIDATION_MIN_INTERVAL_MS=5000
GITGOV_STATS_CACHE_INVALIDATION_MIN_INTERVAL_MS=15000
GITGOV_LOGS_CACHE_INVALIDATION_MIN_INTERVAL_MS=500
GITGOV_CORS_ALLOW_ANY=true
```

Resultados certificados con esa configuración:
- Stress (`think_ms=120`): pasa `250` usuarios simultáneos en pruebas de 60s.
- Realista (`think_ms=2000`): pasa `100` usuarios simultáneos con amplio margen.

### Decisiones operativas

- **No usar RDS por ahora**: DB en Supabase.
- **No subir Desktop a AWS**: Tauri se distribuye como instalador.
- **EC2 + Nginx + systemd**: ruta actual para el backend.
- **Webhooks**: se activan cuando exista URL pública con HTTPS (dominio + certbot).

### Estado actual (validado)

- EC2 creada y accesible por SSH
- Elastic IP asignada
- Security Group: `22` (IP operador), `80`, `443`
- `gitgov-server` corriendo como systemd
- Nginx proxy hacia `127.0.0.1:3000`
- Endpoints validados: `/health`, `/stats` con Bearer
- Fuente de despliegue activa en EC2: `/home/ubuntu/GitGov-deploy` (alineada a `origin/main`)
- Repo legacy archivado para evitar drift operativo: `/home/ubuntu/GitGov-legacy-20260315-074028`

### URLs actuales (sin dominio)

- Público (HTTP): `http://3.143.150.199`
- Health: `http://3.143.150.199/health`

### Estructura en EC2

| Path | Propósito |
|------|-----------|
| `/opt/gitgov/bin/gitgov-server` | Binario |
| `/opt/gitgov/config/gitgov-server.env` | Variables de entorno |
| `/etc/systemd/system/gitgov-server.service` | Servicio systemd |
| `/etc/nginx/sites-available/gitgov` | Nginx site |

### Variables de entorno requeridas

Archivo: `/opt/gitgov/config/gitgov-server.env`

- `DATABASE_URL` — PostgreSQL (local `127.0.0.1` o remoto con SSL según topología)
- `GITGOV_JWT_SECRET`
- `GITGOV_API_KEY`
- `GITGOV_SERVER_ADDR=0.0.0.0:3000`
- `RUST_LOG=info`
- `GITHUB_WEBHOOK_SECRET`
- `JENKINS_WEBHOOK_SECRET` (opcional)
- `JIRA_WEBHOOK_SECRET` (opcional)
- `GITGOV_ALERT_WEBHOOK_URL` (opcional, alertas genéricas)
- `GITGOV_DRIFT_ALERT_WEBHOOK_URLS` (opcional, webhooks dedicados para drift crítico)
- `GITGOV_POLICY_CHECK_BLOCK_SCOPES` (opcional, CSV `org:branch_glob`, activa `409` en `/policy/check` cuando `allowed=false`)

> Permisos recomendados del archivo: `root:gitgov` + `640`. No guardar en Git.

### Operación

```bash
# Backend
sudo systemctl status gitgov-server --no-pager
sudo systemctl restart gitgov-server
sudo journalctl -u gitgov-server -f

# Nginx
sudo systemctl status nginx --no-pager
sudo nginx -t
sudo systemctl restart nginx
```

### Validación rápida

```bash
# Desde EC2
curl http://127.0.0.1:3000/health
curl http://127.0.0.1/health

# Desde equipo local
curl http://3.143.150.199/health
curl -H "Authorization: Bearer <API_KEY>" http://3.143.150.199/stats
```

### Orden de validación post-deploy

1. Smoke tests: `/health`, `/stats` (Bearer), logs del servicio
2. Golden Path Desktop: stage → commit → push → logs/commits
3. Jenkins: `/integrations/jenkins` + Pipeline Health
4. Jira/GitHub webhooks: después de dominio + HTTPS

### Gate de capacidad 250 simultáneos (runtime)

Precondición:
- Gate 0 en verde (`make smoke` + checklist Golden Path).

Gate 1 (single-node hardening):

```bash
cd gitgov/gitgov-server
python tests/perf_baseline_control_plane.py --server-url http://127.0.0.1:3000 --out-json tests/artifacts/perf_gate1.json
python tests/chat_capacity_test.py --server-url http://127.0.0.1:3000 --scenario mixed --out-json tests/artifacts/chat_gate1.json
```

Gate 2 (3 nodos + limiter distribuido + SSE distribuido):

```bash
cd gitgov/gitgov-server
make capacity-mixed \
  SERVER_URL=http://127.0.0.1:3000 \
  API_KEYS_FILE=tests/api_keys.txt \
  CAPACITY_USERS=250 \
  CAPACITY_DURATION_SEC=1200 \
  CAPACITY_OUT=tests/artifacts/capacity_mixed_250_gate2.json

make capacity-soak \
  SERVER_URL=http://127.0.0.1:3000 \
  API_KEYS_FILE=tests/api_keys.txt \
  CAPACITY_USERS=250 \
  CAPACITY_SOAK_DURATION_SEC=3600 \
  CAPACITY_SOAK_OUT=tests/artifacts/capacity_mixed_250_soak.json
```

Criterios de salida obligatorios:
- Core (`/events`, `/logs`, `/stats`):
  - `401 = 0`
  - `5xx = 0`
  - `429 < 2%`
  - `p95 < 800ms`
  - `p99 < 1500ms`
- Chat (`/chat/ask`):
  - `5xx = 0`
  - puede degradar en `429` sin afectar SLO del core.

Rollout sugerido:
1. Canary 10% por 30 min.
2. 50% por 60 min.
3. 100% si se cumplen SLO/gates.

Rollback inmediato:
- Volver a 1 nodo backend.
- Desactivar limiter distribuido:
  - `GITGOV_RATE_LIMIT_DISTRIBUTED_DB=false`
- Mantener URL canónica de Desktop (`http://127.0.0.1:3000` en local).

### Runbook post-deploy (governance v2)

Validar en este orden para confirmar que policy workflow, drift audit y export de compliance siguen operativos:

```bash
# 1) Health + stats (admin key)
curl http://127.0.0.1:3000/health
curl -H "Authorization: Bearer <ADMIN_API_KEY>" http://127.0.0.1:3000/stats

# 2) Crear policy change request (developer o admin)
curl -X POST "http://127.0.0.1:3000/policy/<owner>/<repo>/requests" \
  -H "Authorization: Bearer <DEV_OR_ADMIN_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"config":{"branches":{"protected":["main"],"patterns":["feat/*"]},"rules":{"require_pull_request":true},"enforcement":{"pull_requests":"warn","commits":"off","branches":"warn","traceability":"off"}},"reason":"post-deploy check"}'

# 3) Aprobar/rechazar request (admin)
curl -X POST "http://127.0.0.1:3000/policy/requests/<REQUEST_ID>/approve" \
  -H "Authorization: Bearer <ADMIN_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"note":"post-deploy approval check"}'

# 4) Ingesta drift snapshot crítica (auth)
curl -X POST "http://127.0.0.1:3000/policy/drift-events" \
  -H "Authorization: Bearer <ADMIN_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"action":"drift_snapshot","repo_name":"<owner>/<repo>","result":"observed","metadata":{"drift_count":2,"critical_count":1}}'

# 5) Export compliance v2 (admin)
curl -X POST "http://127.0.0.1:3000/export" \
  -H "Authorization: Bearer <ADMIN_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{"export_type":"events_csv"}'
```

Resultado esperado:
- Paso 2: `{"accepted":true,"status":"pending",...}`
- Paso 3: `{"status":"approved",...}` o `{"status":"rejected",...}`
- Paso 4: `{"accepted":true,...}`
- Paso 5: CSV con filas `policy_drift` y `policy_change_request` cuando existen datos.

### Pendiente

1. Dominio (A record a `3.143.150.199`)
2. HTTPS con `certbot` + Nginx
3. Configurar webhooks GitHub/Jira

### Nota de seguridad

Si una API key fue compartida en chat/capturas, **rotarla**:
1. Generar nueva key
2. Actualizar `GITGOV_API_KEY` en EC2
3. Reiniciar `gitgov-server`
4. Actualizar Desktop/Jenkins

---

## 3. Enterprise Desktop Deployment

### Prerequisites

- Network access to Control Plane server (HTTP/HTTPS, default port 3000)
- API key issued by GitGov admin
- Platform requirements:
  - Windows 10/11 x64 (+ .NET Framework 4.7.2+)
  - macOS 12+ (Apple Silicon / Intel)
  - Linux x64 (glibc-based distro)

### Installer Options

| Format | File | Use case |
|--------|------|----------|
| NSIS (`.exe`) | `GitGov_x.x.x_x64-setup.exe` | Silent install via GPO / Intune / SCCM |
| MSI (`.msi`) | `GitGov_x.x.x_x64_en-US.msi` | Group Policy Software Installation |

Both installers are code-signed. Verify SHA256 hashes from the release page.

### Silent Installation (NSIS)

```
GitGov_x.x.x_x64-setup.exe /S /D=C:\Program Files\GitGov
```

| Flag | Description |
|------|-------------|
| `/S` | Silent mode — no UI |
| `/D=<path>` | Installation directory (must be last, no quotes) |

Uninstall:
```
"C:\Program Files\GitGov\Uninstall GitGov.exe" /S
```

### MSI via Group Policy

```
msiexec /i GitGov_x.x.x_x64_en-US.msi /quiet /norestart INSTALLDIR="C:\Program Files\GitGov"
```

Assign to GPO: Computer Configuration > Software Settings > Software Installation.

### Microsoft Intune

1. Package with `IntuneWinAppUtil.exe`:
   ```
   IntuneWinAppUtil.exe -c . -s GitGov_x.x.x_x64-setup.exe -o ./output
   ```
2. In Intune > Apps > Windows > Add Win32:
   - **Install:** `GitGov_x.x.x_x64-setup.exe /S`
   - **Uninstall:** `"C:\Program Files\GitGov\Uninstall GitGov.exe" /S`
   - **Detection:** File exists `C:\Program Files\GitGov\GitGov.exe`
   - **Return codes:** 0 = success, 1641/3010 = success (reboot)

### Pre-configuring Server Connection

Set machine-wide environment variables:

| Variable | Example | Description |
|----------|---------|-------------|
| `GITGOV_SERVER_URL` | `http://192.168.1.50:3000` | Control Plane URL |
| `GITGOV_API_KEY` | `57f1ed59-...` | API key from admin |

**Via Group Policy:**
```
Computer Configuration > Preferences > Windows Settings > Environment
```

**Via PowerShell (Intune):**
```powershell
[System.Environment]::SetEnvironmentVariable("GITGOV_SERVER_URL", "http://192.168.1.50:3000", "Machine")
[System.Environment]::SetEnvironmentVariable("GITGOV_API_KEY", "your-api-key-here", "Machine")
```

**Via SCCM:**
```
cmd.exe /c setx GITGOV_SERVER_URL "http://192.168.1.50:3000" /M
cmd.exe /c setx GITGOV_API_KEY "your-api-key-here" /M
```

> Fallback: the app also reads from `%APPDATA%\..\Local\gitgov\.env`.

### Verifying Installation

```powershell
Test-Path "C:\Program Files\GitGov\GitGov.exe"
(Get-Item "C:\Program Files\GitGov\GitGov.exe").VersionInfo.ProductVersion
[System.Environment]::GetEnvironmentVariable("GITGOV_SERVER_URL", "Machine")
```

### SHA256 Hash Verification

```powershell
Get-FileHash .\GitGov_x.x.x_x64-setup.exe -Algorithm SHA256
```

Generate `.sha256` file:
```powershell
.\scripts\generate_sha256.ps1 -InstallerPath ".\gitgov\src-tauri\target\release\bundle\nsis\GitGov_x.x.x_x64-setup.exe"
```

Upload both `.exe` and `.sha256` as GitHub Release assets. Set hash in Vercel:
```
NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM=sha256:<hex>
```

### Code Signing

Verify signature:
```powershell
Get-AuthenticodeSignature .\GitGov_x.x.x_x64-setup.exe | Select-Object -Property Status, SignerCertificate
```

**CI secrets required for signed releases:**

| Secret | Description |
|--------|-------------|
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` blob |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for `.pfx` |
| `WINDOWS_CERTIFICATE_THUMBPRINT` | Cert thumbprint for Tauri signing |
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri updater signing private key |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for updater key |

**CI builds** (`build-signed.yml`) on `v*` tag pushes:
- Windows: NSIS + MSI + `.sha256`
- macOS: DMG + `.sha256`
- Linux: AppImage + DEB + `.sha256`

**Local signed build:**
```powershell
.\scripts\build_signed_windows.ps1 -RepoRoot . -PfxPath "C:\secrets\gitgov-codesign.pfx" -PfxPassword "<password>"
```

### Firewall / Proxy

| Destination | Port | Protocol | Purpose |
|-------------|------|----------|---------|
| Control Plane server | 3000 (or configured) | HTTP/HTTPS | Events + dashboard |
| `downloads.gitgov.com` | 443 | HTTPS | Auto-update checks |

If using a proxy, set `HTTP_PROXY` / `HTTPS_PROXY` environment variables.

### Offboarding a Developer

1. **Revoke API key** from dashboard (immediate effect — 401 on next sync)
2. **Uninstall** via Intune/SCCM/GPO
3. Audit history remains intact and immutable

### Compliance Export

1. Open **Control Plane** tab in Desktop
2. Connect with Admin API key
3. **Export Historial de Auditoría** → select range → Exportar JSON
4. Creates immutable log entry in `export_logs` table

---

## 4. Desktop Updates (Tauri Updater)

Actualizaciones in-app usando `tauri-plugin-updater` con full updates (sin deltas) y distribución por S3 + CloudFront.

### Estado actual (implementado)

- `tauri-plugin-updater` integrado en Desktop
- UI en `Configuración > Actualizaciones Desktop`
- `Buscar actualizaciones` manual
- Auto-check al iniciar (throttling ~6h)
- Changelog simple (campo `body` del manifest)
- Fallback de descarga manual

### Requisito para producción

El updater **no funcionará** hasta configurar en `tauri.conf.json`:
- `plugins.updater.endpoints`
- `plugins.updater.pubkey`

Y firmar el update con la clave del updater de Tauri.

### Arquitectura (AWS)

- **S3**: almacenar artefactos y manifests
- **CloudFront**: servir con HTTPS y CDN
- Canales: `stable` (y `beta` posterior)

```
s3://gitgov-downloads/desktop/
  stable/
    latest.json
    GitGov_0.1.1_x64-setup.exe
    GitGov_0.1.1_x64-setup.exe.sig
  beta/
    latest.json
    ...
```

CloudFront URL: `https://downloads.gitgov.com/desktop/stable/latest.json`

### Configuración `tauri.conf.json`

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

> Ver snippet listo: `docs/examples/desktop-updater/tauri.updater.config.snippet.json`

### Claves de firma del updater

El updater usa un par de claves asimétricas:
- **Clave privada (secreta)**: firma cada update. Solo en máquina de release o CI secrets. Nunca se commitea.
- **Clave pública**: en `tauri.conf.json`. Verifica firma antes de instalar. No es secreta.

> Esto NO es lo mismo que code signing de Windows. Son dos firmas distintas. Usar **ambas** en producción.

### Generar claves (una sola vez)

```powershell
npx tauri signer generate --ci -p "TU_PASSWORD_FUERTE" --write-keys .\secrets\tauri-updater.key
```

Copiar la clave pública a `tauri.conf.json` → `plugins.updater.pubkey`.

### Firmar instalador

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = ".\secrets\tauri-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "TU_PASSWORD"
npx tauri signer sign .\src-tauri\target\release\bundle\nsis\GitGov_0.1.1_x64-setup.exe
```

### Release flow

1. Incrementar versión en `tauri.conf.json`
2. Build release (`tauri build`)
3. Generar firma (`.sig`)
4. Crear/actualizar `latest.json`
5. Subir `.exe`, `.sig` y `latest.json` a S3
6. Invalidar CloudFront (si aplica)
7. Probar desde versión anterior

### Scripts helper

```powershell
# Generar manifest
.\scripts\release\desktop-updater\New-TauriUpdaterManifest.ps1 `
  -Version "0.1.1" `
  -Url "https://downloads.gitgov.com/desktop/stable/GitGov_0.1.1_x64-setup.exe" `
  -Signature "FIRMA" `
  -Notes "Changelog" `
  -OutputPath ".\release\desktop\stable\latest.json"

# Publicar a S3
.\scripts\release\desktop-updater\Publish-DesktopUpdateAws.ps1 `
  -ExePath ".\src-tauri\target\release\bundle\nsis\GitGov_0.1.1_x64-setup.exe" `
  -SigPath ".\release\desktop\stable\GitGov_0.1.1_x64-setup.exe.sig" `
  -ManifestPath ".\release\desktop\stable\latest.json" `
  -Bucket "gitgov-downloads" `
  -Channel "stable" `
  -CloudFrontDistributionId "E123ABC456DEF"

# Generar snippet de config
.\scripts\release\desktop-updater\New-TauriUpdaterConfigSnippet.ps1 `
  -Channel "stable" `
  -BaseUrl "https://downloads.gitgov.com/desktop" `
  -PubKey "PUBLIC_KEY" `
  -OutputPath ".\release\desktop\tauri.updater.stable.json"
```

### Disable auto-updates (air-gapped)

Block `downloads.gitgov.com` at the firewall. The app continues functioning; only update notifications are suppressed.

### Troubleshooting

| Síntoma | Causa | Solución |
|---------|-------|----------|
| "Updater no configurado" | Falta `plugins.updater`, `endpoints` o `pubkey` en `tauri.conf.json` | Configurar los campos |
| "No se pudo verificar/instalar" | URL inaccesible, firma incorrecta o pubkey mal | Verificar URL, signature y pubkey |
| Usuario no ve notificación | Throttling ~6h o no está en Desktop | Probar `Buscar actualizaciones` manual |

### Próximas fases

- **Fase 2:** Canales beta/stable, telemetría de updater, reintento de descarga
- **Fase 3:** `min_supported_version` desde backend, forced updates (solo críticos)

---

## Support

- Documentation: `docs/` directory
- Issues: https://github.com/MapfrePE/GitGov/issues
- Health check: `GET http://<server>:3000/health`

---

*Documento consolidado de: DEPLOY_EC2_SUPABASE.md, DOCKER.md, ENTERPRISE_DEPLOY.md, DESKTOP_UPDATES.md*
*Fecha de consolidación: 2026-03-14*
