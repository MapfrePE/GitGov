# Docker Local (GitGov)

Setup Docker local para levantar:
- PostgreSQL (`gitgov-db`)
- GitGov Control Plane Server (`gitgov-server`)
- Jenkins (opcional, perfil `jenkins`, para pruebas V1.2-A)
- Jira Software (opcional, perfil `jira`, para pruebas V1.2-B)

No reemplaza tu app Desktop/Tauri local. La idea es correr el **server** en Docker y seguir usando GitGov Desktop como cliente.

---

## Requisitos

- Docker Desktop ejecutándose
- Puerto `3001` libre (GitGov server Docker)
- Puerto `5433` libre (Postgres Docker)

---

## Levantar stack

Desde la raíz del repo:

```bash
docker compose up --build -d
```

Ver estado:

```bash
docker compose ps
```

Logs del server:

```bash
docker compose logs -f gitgov-server
```

Logs de Postgres:

```bash
docker compose logs -f gitgov-db
```

---

## Jira (opcional, perfil `jira`)

Levantar Jira local (puede tardar varios minutos en primer arranque):

```bash
docker compose --profile jira up -d jira
```

Ver logs de Jira:

```bash
docker compose logs -f jira
```

Abrir Jira:

- URL: `http://localhost:8095`

Notas:
- Primer arranque puede tardar bastante (descarga imagen + setup interno).
- Jira requiere configuración inicial vía navegador (wizard de Atlassian).
- Para pruebas locales V1.2-B basta con usarlo como emisor de webhooks/snapshot de issues.

---

## Jenkins (opcional, perfil `jenkins`)

Levantar Jenkins local:

```bash
docker compose --profile jenkins up -d jenkins
```

Ver logs de Jenkins:

```bash
docker compose logs -f jenkins
```

Abrir Jenkins:

- URL: `http://localhost:8096`

Password inicial (admin) desde logs o archivo en el contenedor:

```bash
docker exec -it gitgov-jenkins cat /var/jenkins_home/secrets/initialAdminPassword
```

Notas:
- Primer arranque puede tardar varios minutos.
- Jenkins también requiere setup inicial (wizard).
- Luego puedes configurar webhook/pipeline hacia `GitGov` (`/integrations/jenkins`).

---

## Qué inicializa automáticamente

Al crear el volumen de Postgres por primera vez, Docker ejecuta:

1. `supabase_schema.sql`
2. `supabase_schema_v4.sql`
3. `supabase_schema_v5.sql`
4. `supabase_schema_v6.sql`

Esto deja lista la base para:
- core audit/events
- Jenkins V1.2-A
- Jira/Ticket Coverage V1.2-B preview

Importante:
- Si ya existe el volumen, los scripts **no** se vuelven a ejecutar.

---

## URL y credenciales (dev local)

### Server (Docker)
- URL: `http://localhost:3001`

### API Key admin (dev)
- `57f1ed59-371d-46ef-9fdf-508f59bc4963`

### PostgreSQL
- host: `localhost`
- port: `5433`
- db: `gitgov`
- user: `gitgov`
- password: `gitgov_dev_password`

---

## Integrar con GitGov Desktop (tu app)

En la configuración del Control Plane dentro de la app:
- URL: `http://127.0.0.1:3001` (solo si quieres probar el server Docker)
- API Key: `57f1ed59-371d-46ef-9fdf-508f59bc4963`

**Recomendado para Golden Path diario (server local):**
- URL: `http://127.0.0.1:3000`

Esto evita split-brain local si también tienes Docker/WSL levantado.

---

## Reset de base local Docker (si quieres reiniciar de cero)

Esto borra datos locales del contenedor:

```bash
docker compose down -v
docker compose up --build -d
```

---

## Probar endpoints rápidos

Health:

```bash
curl http://localhost:3001/health
```

Stats (admin):

```bash
curl -H "Authorization: Bearer 57f1ed59-371d-46ef-9fdf-508f59bc4963" \
  http://localhost:3001/stats
```

---

## Notas

- Este setup es para **desarrollo/demo local**.
- Para producción conviene:
  - secrets reales
  - TLS reverse proxy
  - Postgres gestionado o backups persistentes
  - variables en `.env`/secret manager
