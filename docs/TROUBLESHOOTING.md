# GitGov - Guía de Troubleshooting

## Índice

1. [Problemas de Autenticación](#problemas-de-autenticación)
2. [Problemas del Outbox](#problemas-del-outbox)
3. [Problemas de Base de Datos](#problemas-de-base-de-datos)
4. [Problemas de Serialización](#problemas-de-serialización)
5. [Problemas de Conexión](#problemas-de-conexión)
6. [Logs y Debugging](#logs-y-debugging)

---

## Problemas de Autenticación

### Error: `401 Unauthorized`

**Síntomas:**
```
WARN Outbox flush failed: status 401 Unauthorized
```

**Causas posibles:**

| Causa | Solución |
|-------|----------|
| Header incorrecto | Usar `Authorization: Bearer {key}`, NO `X-API-Key` |
| API key no existe en DB | Verificar con SQL: `SELECT * FROM api_keys WHERE key_hash = '...'` |
| API key desactivada | `is_active = false` en tabla `api_keys` |
| Hash incorrecto | El servidor hace SHA256 del token recibido |

**Verificación paso a paso:**

```bash
# 1. Verificar que el header es correcto
curl -v -H "Authorization: Bearer $API_KEY" http://localhost:3000/stats

# 2. Calcular hash de la API key
echo -n "57f1ed59-371d-46ef-9fdf-508f59bc4963" | sha256sum

# 3. Verificar en la base de datos
psql -c "SELECT * FROM api_keys WHERE key_hash = '<hash_calculado>'"
```

**Código correcto:**

```rust
// ✅ CORRECTO
request.header("Authorization", format!("Bearer {}", api_key))

// ❌ INCORRECTO
request.header("X-API-Key", api_key)
```

---

### Error: `Missing Authorization header`

**Causa:** El middleware no encuentra el header.

**Verificar:**
1. El cliente está enviando el header
2. No hay proxy/cors eliminando headers
3. El header está bien formateado

**Debug:**
```rust
// Agregar en auth.rs temporalmente
tracing::debug!("Headers: {:?}", req.headers());
```

---

## Problemas del Outbox

### Error: `Outbox flush failed`

**Síntomas:**
```
WARN Outbox flush failed: status 401 Unauthorized
WARN Outbox flush network error: connection refused
```

**Diagnóstico:**

```bash
# Verificar configuración
cat gitgov/.env | grep -E "SERVER_URL|API_KEY"

# Verificar que el servidor está corriendo
curl http://localhost:3000/health

# Ver archivo de eventos pendientes
cat ~/.gitgov/outbox.jsonl
```

**Causas comunes:**

| Error | Causa | Solución |
|-------|-------|----------|
| `connection refused` | Servidor no corre | Iniciar con `cargo run` |
| `status 401` | Auth incorrecta | Ver header Authorization |
| `timeout` | Red lenta | Aumentar timeout en código |
| `certificate error` | HTTPS local | Usar HTTP en desarrollo |

**Verificar estado del outbox:**

```rust
// En código Rust
let pending = outbox.get_pending_count();
tracing::info!("Pending events: {}", pending);
```

---

### Error: `Events not being sent`

**Síntomas:** Dashboard no muestra nuevos eventos.

**Verificar:**

1. **Background worker iniciado:**
```rust
// En lib.rs, verificar que se llama:
outbox.start_background_flush(60);
```

2. **Eventos en cola:**
```bash
# Ver archivo JSONL
wc -l ~/.gitgov/outbox.jsonl
```

3. **Trigger manual:**
```rust
// Forzar flush
trigger_flush(&outbox);
```

---

## Problemas de Base de Datos

### Error: `invalid type: null, expected a map`

**Síntomas:**
```
thread 'tokio-runtime-worker' panicked:
ColumnDecode { index: "\"stats\"", source: Error("invalid type: null, expected a map") }
```

**Causa:** `json_object_agg()` devuelve NULL cuando no hay filas.

**Solución en SQL:**
```sql
-- Agregar COALESCE
'by_type', COALESCE(
    (SELECT json_object_agg(event_type, cnt) FROM (...) t),
    '{}'::json
)
```

**Solución en Rust:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubEventStats {
    pub total: i64,
    pub today: i64,
    pub pushes_today: i64,
    #[serde(default)]  // <-- Importante
    pub by_type: HashMap<String, i64>,
}
```

---

### Error: `function get_audit_stats() does not exist`

**Causa:** La función no fue creada en la base de datos.

**Solución:**
```bash
# Ejecutar schema en Supabase SQL Editor
# O usando psql:
psql -f gitgov/gitgov-server/supabase_schema.sql
```

---

### Error: `relation "client_events" does not exist`

**Causa:** Tabla no creada.

**Solución:**
```sql
-- Verificar tablas existentes
SELECT tablename FROM pg_tables WHERE schemaname = 'public';

-- Crear tabla si falta
CREATE TABLE client_events (
    id UUID PRIMARY KEY,
    event_uuid TEXT UNIQUE NOT NULL,
    event_type TEXT NOT NULL,
    user_login TEXT NOT NULL,
    branch TEXT,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Problemas de Serialización

### Error: `Serialization error: error decoding response body`

**Causa:** Structs no coinciden entre cliente y servidor.

**Diagnóstico:**

```bash
# Ver respuesta del servidor
curl -H "Authorization: Bearer $API_KEY" http://localhost:3000/stats | jq

# Comparar con struct en Rust
```

**Verificar structs:**

```rust
// Servidor (gitgov-server/src/models.rs)
pub struct ServerStats {
    pub github_events: GitHubEventStats,
    pub client_events: ClientEventStats,
    // ...
}

// Cliente (src-tauri/src/control_plane/server.rs)
pub struct ServerStats {
    pub github_events: GitHubEventStats,
    pub client_events: ClientEventStats,
    // ...
}
```

**Ambos deben ser IDÉNTICOS.**

---

### Error: `missing field` o `unknown field`

**Causa:** Campos con nombres diferentes o faltantes.

**Solución:**
- Usar `#[serde(rename = "campo_name")]` para renombrar
- Usar `#[serde(default)]` para campos opcionales
- Usar `#[serde(skip_serializing_if = "Option::is_none")]` para omitir nulls

---

## Problemas de Conexión

### Error: `connection refused` (ECONNREFUSED)

**Causa:** Servidor no está corriendo o puerto incorrecto.

**Verificar:**
```bash
# Verificar que el servidor corre
lsof -i :3000
# o
netstat -an | grep 3000

# Verificar URL en .env
cat gitgov/.env | grep SERVER_URL
```

---

### Error: `certificate verify failed`

**Causa:** HTTPS con certificado auto-firmado.

**Solución en desarrollo:**
- Usar HTTP en lugar de HTTPS
- O agregar certificado a trust store

**Solución en código:**
```rust
// Solo para desarrollo - NO usar en producción
let client = reqwest::Client::builder()
    .danger_accept_invalid_certs(true)
    .build()?;
```

---

## Logs y Debugging

### Habilitar logs detallados

**Rust (Server):**
```bash
RUST_LOG=debug cargo run
```

**Rust (Desktop):**
```bash
RUST_LOG=gitgov=debug npm run tauri dev
```

### Logs útiles

**Ver eventos recibidos:**
```sql
SELECT 
    event_type, 
    user_login, 
    branch, 
    status, 
    created_at 
FROM client_events 
ORDER BY created_at DESC 
LIMIT 20;
```

**Ver estadísticas actuales:**
```sql
SELECT get_audit_stats();
```

**Ver eventos combinados:**
```sql
SELECT * FROM get_combined_events(100, 0);
```

### Archivos de log

| Ubicación | Contenido |
|-----------|-----------|
| `~/.gitgov/outbox.jsonl` | Eventos pendientes |
| `~/.gitgov/audit.db` | SQLite local de auditoría |
| stdout | Logs del servidor |

---

## Comandos de Emergencia

### Reiniciar outbox

```bash
# Backup
cp ~/.gitgov/outbox.jsonl ~/.gitgov/outbox.jsonl.bak

# Limpiar eventos enviados
# (Editar archivo y eliminar líneas con "sent":true)
```

### Regenerar API key

```sql
-- Desactivar key anterior
UPDATE api_keys SET is_active = false WHERE client_id = 'gitgov-desktop';

-- Crear nueva (desde el servidor, se hace automáticamente al iniciar)
-- O insertar manualmente:
INSERT INTO api_keys (key_hash, client_id, role, is_active)
VALUES ('<sha256_hash>', 'gitgov-desktop', 'admin', true);
```

### Limpiar eventos de prueba

```sql
-- Solo para desarrollo
DELETE FROM client_events WHERE user_login = 'test_user';
DELETE FROM github_events WHERE actor_login = 'test_user';
```

---

## Checklist de Diagnóstico

Cuando algo no funciona:

1. [ ] Servidor corre en puerto 3000
2. [ ] Health check devuelve OK
3. [ ] API key existe en base de datos
4. [ ] Header Authorization: Bearer se envía
5. [ ] Structs cliente/servidor coinciden
6. [ ] No hay NULLs en json_object_agg
7. [ ] Background worker del outbox iniciado
8. [ ] Archivo outbox.jsonl existe
9. [ ] Logs no muestran errores
