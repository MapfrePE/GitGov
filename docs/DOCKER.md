# Docker Local (GitGov)

Setup Docker local para levantar:
- PostgreSQL (`gitgov-db`)
- GitGov Control Plane Server (`gitgov-server`)

No reemplaza tu app Desktop/Tauri local. La idea es correr el **server** en Docker y seguir usando GitGov Desktop como cliente.

---

## Requisitos

- Docker Desktop ejecutándose
- Puerto `3000` libre (server)
- Puerto `5432` libre (Postgres)

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

### Server
- URL: `http://localhost:3000`

### API Key admin (dev)
- `57f1ed59-371d-46ef-9fdf-508f59bc4963`

### PostgreSQL
- host: `localhost`
- port: `5432`
- db: `gitgov`
- user: `gitgov`
- password: `gitgov_dev_password`

---

## Integrar con GitGov Desktop (tu app)

En la configuración del Control Plane dentro de la app:
- URL: `http://localhost:3000`
- API Key: `57f1ed59-371d-46ef-9fdf-508f59bc4963`

Esto mantiene tu flujo actual:
- cambios → commit → push → logs/dashboard

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
curl http://localhost:3000/health
```

Stats (admin):

```bash
curl -H "Authorization: Bearer 57f1ed59-371d-46ef-9fdf-508f59bc4963" \
  http://localhost:3000/stats
```

---

## Notas

- Este setup es para **desarrollo/demo local**.
- Para producción conviene:
  - secrets reales
  - TLS reverse proxy
  - Postgres gestionado o backups persistentes
  - variables en `.env`/secret manager
