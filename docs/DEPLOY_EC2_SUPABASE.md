# Deploy Backend en AWS EC2 (Supabase)

Guía operativa mínima para el despliegue actual de `gitgov-server` en AWS usando:

- `EC2 Ubuntu 22.04`
- `Nginx` como reverse proxy
- `systemd` para el backend
- `Supabase` como PostgreSQL remoto (sin RDS)

## Alcance

Este documento cubre solo el **backend** (`gitgov-server`) en AWS.

No cubre:

- Deploy del Desktop App (Tauri) a AWS (no aplica; se distribuye como instalador)
- Dominio + HTTPS (pendiente, se hará después)
- Webhooks Jira/GitHub en producción (pendiente hasta tener HTTPS)

## Decisiones operativas (importantes)

- **No usar RDS por ahora**: la base de datos de producción actual está en **Supabase**.
- **No “subir” la app Desktop a AWS**: Tauri se distribuye como instalador y se conecta al backend por URL.
- **EC2 + Nginx + systemd** es la ruta actual (rápida y mantenible) para el backend.
- **GitHub/Jira webhooks** se activan cuando exista **URL pública estable con HTTPS** (dominio + certbot).

## Estado actual (validado)

- EC2 creada y accesible por SSH
- Elastic IP asignada
- Security Group con `22` (IP del operador), `80`, `443`
- `gitgov-server` desplegado y corriendo como `systemd`
- `Nginx` proxy hacia `127.0.0.1:3000`
- Backend conectado a Supabase
- Endpoints validados:
  - `GET /health` local y público
  - `GET /stats` con `Authorization: Bearer ...`

## URLs actuales (sin dominio)

- Público (HTTP): `http://3.143.150.199`
- Health público: `http://3.143.150.199/health`

Nota:
- Se usa HTTP temporalmente para pruebas.
- Para Jira/GitHub webhooks se recomienda **dominio + HTTPS**.

## Estructura en EC2

- Binario: `/opt/gitgov/bin/gitgov-server`
- Env de producción: `/opt/gitgov/config/gitgov-server.env`
- Servicio systemd: `/etc/systemd/system/gitgov-server.service`
- Nginx site: `/etc/nginx/sites-available/gitgov`

## Variables de entorno requeridas (backend)

Archivo: `/opt/gitgov/config/gitgov-server.env`

Variables mínimas:

- `DATABASE_URL` (Supabase Postgres, con `sslmode=require`)
- `GITGOV_JWT_SECRET`
- `GITGOV_API_KEY`
- `GITGOV_SERVER_ADDR=0.0.0.0:3000` (actual)
- `RUST_LOG=info`
- `GITHUB_WEBHOOK_SECRET`
- `JENKINS_WEBHOOK_SECRET`
- `JIRA_WEBHOOK_SECRET`

Notas:

- Si `JENKINS_WEBHOOK_SECRET` / `JIRA_WEBHOOK_SECRET` no están definidos, los endpoints pueden operar en modo compatible (solo API key admin), pero se recomienda activarlos antes de exponer webhooks reales.
- Rotar `GITGOV_API_KEY` y cualquier token expuesto antes de abrir acceso externo.

Seguridad:

- No guardar este archivo en Git
- Permisos recomendados: `root:gitgov` + `640`

## Operación (EC2)

### Backend

- Estado:
  - `sudo systemctl status gitgov-server --no-pager`
- Reiniciar:
  - `sudo systemctl restart gitgov-server`
- Logs:
  - `sudo journalctl -u gitgov-server -f`

### Nginx

- Estado:
  - `sudo systemctl status nginx --no-pager`
- Validar config:
  - `sudo nginx -t`
- Reiniciar:
  - `sudo systemctl restart nginx`

## Validación rápida

### Desde EC2

- Health backend:
  - `curl http://127.0.0.1:3000/health`
- Health vía Nginx:
  - `curl http://127.0.0.1/health`

### Desde equipo local

- Health público:
  - `curl http://3.143.150.199/health`
- Stats con auth:
  - `curl -H "Authorization: Bearer <API_KEY>" http://3.143.150.199/stats`

Importante:
- El servidor **solo acepta** `Authorization: Bearer`
- No usar `X-API-Key`

## Siguiente paso (sin dominio, para avanzar hoy)

1. Apuntar Desktop App a `http://3.143.150.199`
2. Validar Golden Path (stage -> commit -> push -> logs/stats)
3. Cambiar Jenkins `gitgov-url` a `http://3.143.150.199`
4. Validar `/integrations/jenkins/status`

## Orden de validación recomendado (post-deploy)

1. **Smoke tests backend**: `/health`, `/stats` (Bearer), logs del servicio.
2. **Golden Path Desktop**: stage -> commit -> push -> ver logs/commits en Control Plane.
3. **Jenkins**: `/integrations/jenkins` + Pipeline Health/correlaciones.
4. **Jira/GitHub webhooks**: dejar para **dominio + HTTPS**.

## Pendiente (mañana)

1. Dominio (A record a `3.143.150.199`)
2. HTTPS con `certbot` + Nginx
3. Configurar webhooks:
   - GitHub -> `/webhooks/github`
   - Jira -> `/integrations/jira`

## Nota de seguridad

Si una API key de producción fue compartida en chat/capturas, **rotarla**:

1. Generar nueva key válida
2. Actualizar `GITGOV_API_KEY` en EC2
3. Reiniciar `gitgov-server`
4. Actualizar Desktop/Jenkins
