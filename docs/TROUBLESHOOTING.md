# GitGov - Guía de Troubleshooting

## Cómo Usar Esta Guía

Esta guía te ayuda a resolver problemas comunes. Cada sección explica:
- Qué síntomas verás
- Por qué pasa
- Cómo solucionarlo

---

## Índice

1. [Problemas de Autenticación](#problemas-de-autenticación)
2. [Problemas del Outbox](#problemas-del-outbox)
3. [Problemas de Base de Datos](#problemas-de-base-de-datos)
4. [Problemas de Serialización](#problemas-de-serialización)
5. [Problemas de Conexión](#problemas-de-conexión)
6. [Problemas de Variables de Entorno](#problemas-de-variables-de-entorno)
7. [Rate Limiting (429)](#rate-limiting-429)
8. [Integraciones Jenkins / Jira](#integraciones-jenkins--jira)
9. [Paginación: defaults y límites](#paginación-defaults-y-límites)
10. [Logs y Debugging](#logs-y-debugging)

---

## Problemas de Autenticación

### Error: 401 Unauthorized

**Qué verás:**
- En los logs: "WARN Outbox flush failed: status 401 Unauthorized"
- El dashboard no carga datos del servidor
- Los eventos no aparecen en el servidor

**Por qué pasa:**

El servidor rechazó la request porque no pudo verificar tu identidad.

**Causas más comunes:**

| Causa | Cómo verificar | Solución |
|-------|----------------|----------|
| Header incorrecto | El header debe ser `Authorization: Bearer`, NO `X-API-Key` | Corregir el header en la configuración |
| API key no existe | Verificar en la base de datos | Crear la API key nuevamente |
| API key desactivada | El campo `is_active` está en false | Reactivar la key |
| Hash incorrecto | El servidor calcula SHA256 del token recibido | Usar la key exacta, sin espacios extra |

**Pasos para diagnosticar:**

1. Verificar que el header se envía correctamente
2. Calcular el hash SHA256 de tu API key
3. Buscar ese hash en la tabla api_keys de la base de datos
4. Confirmar que is_active es true

**Solución más común:**

El error más frecuente es usar el header incorrecto. El servidor SOLO acepta:
- ✅ `Authorization: Bearer TU_API_KEY`
- ❌ `X-API-Key: TU_API_KEY` (esto NO funciona)

---

### Error: Missing Authorization header

**Qué verás:**
- Error 401 inmediato
- Mensaje: "Missing Authorization header"

**Por qué pasa:**

El middleware de autenticación no encontró ningún header de autorización en la request.

**Causas y soluciones:**

| Causa | Solución |
|-------|----------|
| El cliente no envía el header | Verificar que el código incluye el header |
| Un proxy elimina headers | Revisar configuración de nginx/cloudflare |
| CORS mal configurado | Habilitar el header Authorization en CORS |

---

## Problemas del Outbox

### Error: Outbox flush failed

**Qué verás:**
- En los logs: "WARN Outbox flush failed: status XXX"
- El dashboard del servidor no muestra tus eventos
- El archivo outbox.jsonl crece pero los eventos no llegan

**Por qué pasa:**

El worker en background intentó enviar eventos al servidor pero falló.

**Diagnóstico paso a paso:**

1. **Verificar configuración**
   - Confirmar que SERVER_URL está configurado
   - Confirmar que API_KEY está configurado
   - Verificar que no hay typos en los valores

2. **Verificar conectividad**
   - El servidor está corriendo en el puerto 3000?
   - Puedes hacer ping a la URL del servidor?
   - Hay firewall bloqueando la conexión?

3. **Verificar el archivo de eventos**
   - Existe ~/.gitgov/outbox.jsonl?
   - Tiene contenido?
   - Los eventos tienen "sent": false?

**Errores comunes y soluciones:**

| Error | Significado | Solución |
|-------|-------------|----------|
| connection refused | Servidor no está corriendo | Iniciar el servidor |
| status 401 | Autenticación falló | Ver header Authorization |
| timeout | Red muy lenta | Aumentar timeout o mejorar red |
| certificate error | HTTPS con certificado inválido | Usar HTTP en desarrollo |

---

### Error: Events not being sent

**Qué verás:**
- El dashboard no muestra nuevos eventos
- Haces commits/pushes pero no aparecen en el servidor
- No hay errores en los logs

**Por qué pasa:**

Los eventos se están generando pero no se están enviando.

**Verificar:**

1. **El worker de background está iniciado?**
   - Al iniciar la app debe llamarse a start_background_flush
   - Si no, los eventos quedan en la cola local

2. **Hay eventos en la cola?**
   - Verificar el archivo ~/.gitgov/outbox.jsonl
   - Debe tener líneas con eventos

3. **Los eventos tienen sent: false?**
   - Si todos tienen sent: true, ya se enviaron
   - El problema está en otro lado

**Solución:**

Forzar un flush manual puede ayudar a diagnosticar. Si funciona manualmente, el problema está en el worker automático.

---

### Error: El commit/push se hizo desde GitGov pero no aparece en Control Plane (localhost vs 127.0.0.1)

**Qué verás:**
- Haces commit/push desde la app GitGov y GitHub sí recibe el commit
- El dashboard de GitGov no muestra ese commit
- En otro momento sí aparecen commits normalmente

**Por qué pasa (causa real frecuente en local):**

Estás corriendo **dos servidores distintos** en `:3000` (por ejemplo, server local + Docker/WSL), y:
- el Desktop/outbox envía a `http://localhost:3000`
- el Dashboard/Control Plane consulta `http://127.0.0.1:3000`

En algunas máquinas `localhost` puede resolver por IPv6 (`::1`) y terminar pegándole a otro proceso diferente.

**Cómo verificar rápido:**

1. Consultar stats en ambas URLs:
   - `http://127.0.0.1:3000/stats`
   - `http://localhost:3000/stats`
2. Si los totales (`client_events.total`) son distintos, tienes split-brain local.

**Solución (recomendada):**

1. Usar una sola URL canónica en todo el proyecto local: `http://127.0.0.1:3000`
2. Verificar `src-tauri/.env` y configuración del Control Plane
3. Reiniciar la app Desktop para que el outbox tome la URL nueva
4. Evitar correr dos instancias de GitGov server en el mismo puerto `3000`

**Prevención:**
- Si usas Docker y server local a la vez, mueve uno a otro puerto (ej. `3001`)
- No mezclar `localhost` y `127.0.0.1` en configuración
- **Convención del proyecto:** `server local -> 127.0.0.1:3000`, `gitgov-server` Docker -> `127.0.0.1:3001`

---

## Problemas de Base de Datos

### Error: invalid type: null, expected a map

**Qué verás:**
- Crash del servidor con panic
- Error mencionando "ColumnDecode" y "invalid type: null"

**Por qué pasa:**

PostgreSQL tiene una función json_object_agg que devuelve NULL cuando no hay datos. Rust espera un objeto/mapa pero recibe null.

**Solución:**

Se necesita usar COALESCE en las queries SQL para devolver un objeto vacío en lugar de NULL cuando no hay datos.

También los structs en Rust deben tener `#[serde(default)]` en los campos que pueden estar vacíos.

---

### Error: function get_audit_stats() does not exist

**Qué verás:**
- Error al llamar al endpoint /stats
- Mensaje: "function get_audit_stats() does not exist"

**Por qué pasa:**

La función SQL no fue creada en la base de datos.

**Solución:**

Ejecutar el archivo supabase_schema.sql en el editor SQL de Supabase o con psql.

---

### Error: relation "client_events" does not exist

**Qué verás:**
- Error al insertar eventos
- Mensaje: "relation 'client_events' does not exist"

**Por qué pasa:**

La tabla no existe en la base de datos.

**Solución:**

1. Listar las tablas existentes
2. Si falta la tabla, ejecutar el schema completo
3. Verificar que te conectas a la base de datos correcta

---

## Problemas de Serialización

### Error: Serialization error / error decoding response body

**Qué verás:**
- Error al recibir respuesta del servidor
- Mensaje mencionando "Serialization" o "decoding"

**Por qué pasa:**

La estructura de datos que el cliente espera no coincide con lo que el servidor envía.

**Cómo diagnosticar:**

1. Ver la respuesta raw del servidor (sin parsear)
2. Comparar con lo que el cliente espera
3. Buscar diferencias en nombres de campos, tipos, o estructura

**Causas comunes:**

| Causa | Ejemplo | Solución |
|-------|---------|----------|
| Campo con nombre diferente | Servidor envía "pushes_today", cliente espera "pushesToday" | Sincronizar nombres |
| Campo faltante | Servidor no envía un campo que el cliente requiere | Agregar default en el cliente |
| Campo extra | Servidor envía un campo que el cliente no conoce | Ignorar campos desconocidos |
| Estructura diferente | Servidor envía objeto anidado, cliente espera plano | Cambiar estructura |

**Regla de oro:**

Las estructuras de datos deben ser IDÉNTICAS entre cliente y servidor. Cualquier diferencia causará errores de serialización.

---

### Error: missing field / unknown field

**Qué verás:**
- Error específico mencionando un campo
- "missing field: nombre_campo" o "unknown field: nombre_campo"

**Por qué pasa:**

El JSON recibido tiene más o menos campos de los esperados.

**Soluciones:**

- Para campos faltantes: usar defaults en el cliente
- Para campos unknown: el cliente debe ignorar campos extra
- Para campos con nombre diferente: usar alias

---

## Problemas de Conexión

### Error: connection refused (ECONNREFUSED)

**Qué verás:**
- Error inmediato al intentar conectar
- "connection refused" o "ECONNREFUSED"

**Por qué pasa:**

No hay nada escuchando en el puerto al que intentas conectar.

**Verificar:**

1. El servidor está corriendo?
2. Está en el puerto correcto (3000 por defecto)?
3. La URL en el .env es correcta?

**Comandos de diagnóstico:**

- Verificar qué procesos escuchan en puerto 3000
- Verificar la URL configurada en .env
- Intentar conectar manualmente con curl o browser

---

### Error: certificate verify failed

**Qué verás:**
- Error mencionando "certificate" o "SSL"
- Solo pasa con HTTPS

**Por qué pasa:**

El certificado SSL no es válido o es auto-firmado.

**Soluciones:**

- En desarrollo: usar HTTP en lugar de HTTPS
- En producción: usar certificados válidos (Let's Encrypt)
- Temporal: aceptar certificados inválidos (SOLO desarrollo)

---

## Problemas de Variables de Entorno

### El Outbox envía eventos pero el Dashboard no muestra nada (o viceversa)

**Síntomas:**
- Haces commit/push, los logs del servidor muestran que llegan eventos (status 200)
- Pero el panel "Control Plane" de la app muestra "Sin datos" o no actualiza

**Causa: Desktop App tiene DOS capas de configuración independientes**

La Desktop App tiene dos contextos de ejecución con fuentes de configuración distintas:

| Capa | Variables | Qué lee | Para qué |
|------|-----------|---------|----------|
| Rust/Tauri (backend) | `GITGOV_SERVER_URL`, `GITGOV_API_KEY` | `gitgov/.env` (lado Rust) | Outbox, git commands, envío de eventos |
| Vite/React (frontend) | `VITE_SERVER_URL`, `VITE_API_KEY` | `gitgov/.env` (lado Vite) | Dashboard UI, consultas desde el navegador |

Si configuras solo `VITE_*`, el outbox (Rust) no sabe a dónde enviar. Si configuras solo `GITGOV_*`, el dashboard (React) no sabe a dónde consultar.

**Solución: definir ambos pares en `gitgov/.env`:**

```env
# Para el frontend React (dashboard UI)
VITE_SERVER_URL=http://127.0.0.1:3000
VITE_API_KEY=tu-api-key-aqui

# Para el backend Rust (outbox, git commands)
GITGOV_SERVER_URL=http://127.0.0.1:3000
GITGOV_API_KEY=tu-api-key-aqui
```

**Diagnóstico rápido:**
- Si el servidor recibe eventos (logs muestran 200) pero dashboard no carga → falta `VITE_*`
- Si dashboard carga pero no llegan eventos al servidor → falta `GITGOV_*`
- Si nada funciona → faltan ambos pares o la URL tiene `localhost` en vez de `127.0.0.1`

---

### La API key no aparece al arrancar el servidor en Docker/CI

**Síntoma:** El servidor arranca sin errores pero nunca muestra la API key generada en los logs.

**Causa:** Por seguridad, la API key generada automáticamente solo se imprime si:
1. La salida estándar es una TTY interactiva (terminal real)
2. O se usó el flag `--print-bootstrap-key` explícitamente

En Docker/Kubernetes los logs no son TTY, así que la key no aparece.

**Solución:**
```bash
# Opción 1: Flag explícito
./gitgov-server --print-bootstrap-key

# Opción 2: Establecer la key directamente en el entorno
GITGOV_API_KEY=tu-uuid-aqui ./gitgov-server
```

Con `GITGOV_API_KEY` en el entorno, el servidor inserta esa key en la DB (si no existe) sin generar ni imprimir nada.

---

## Rate Limiting (429)

### Error: 429 Too Many Requests

**Síntoma:** El servidor responde con status 429 en lugar de procesar el request.

**Causa:** Se superó el límite de requests por segundo para ese endpoint.

**Límites por defecto:**

| Endpoint | Límite | Burst |
|----------|--------|-------|
| `/events` | 10 req/s | 20 |
| `/webhooks/github` (audit-stream) | 5 req/s | 10 |
| `/integrations/jenkins` | 5 req/s | 10 |
| `/integrations/jira` | 5 req/s | 10 |

**La clave de rate limiting** es `{IP}:{SHA256(auth_header)[0:12]}`. Cada API key distinta tiene su propio límite por IP.

**Solución para cargas altas:**

Ajustar en `gitgov/gitgov-server/.env`:
```env
GITGOV_RATE_LIMIT_EVENTS_PER_MIN=600
GITGOV_RATE_LIMIT_JENKINS_PER_MIN=300
GITGOV_RATE_LIMIT_JIRA_PER_MIN=300
GITGOV_RATE_LIMIT_ADMIN_PER_MIN=120
```

Reiniciar el servidor para que tome los nuevos valores.

---

## Integraciones Jenkins / Jira

### Jenkins: eventos llegan pero no aparecen correlaciones

**Síntomas:**
- `POST /integrations/jenkins` devuelve 200
- `GET /integrations/jenkins/correlations` devuelve array vacío

**Causas posibles:**

| Causa | Diagnóstico | Solución |
|-------|-------------|----------|
| SHA mismatch | El `commit_sha` que envía Jenkins no coincide con el de Desktop | Verificar que Jenkins reporta el SHA exacto del commit |
| Tabla vacía de commits | No hay `client_events` de tipo `commit` en el server | Hacer al menos un commit + push desde Desktop primero |
| Short vs full SHA | Jenkins envía SHA corto (7 chars), Desktop envía SHA completo | La correlación soporta prefijo, pero el SHA corto debe ser prefijo del largo |

**Verificar manualmente:**
```bash
# Ver pipeline events ingresados
curl -H "Authorization: Bearer $API_KEY" http://127.0.0.1:3000/integrations/jenkins/status

# Ver correlaciones
curl -H "Authorization: Bearer $API_KEY" "http://127.0.0.1:3000/integrations/jenkins/correlations?limit=10"
```

---

### Jira: secret incorrecto o rechazado

**Síntoma:** `POST /integrations/jira` devuelve 401.

**Causa:** Si `JIRA_WEBHOOK_SECRET` está configurado en el servidor, todos los requests deben incluir el header `x-gitgov-jira-secret` con el valor correcto.

**Solución:**
```bash
# Enviar con el header correcto
curl -X POST http://127.0.0.1:3000/integrations/jira \
  -H "Authorization: Bearer $API_KEY" \
  -H "x-gitgov-jira-secret: $JIRA_WEBHOOK_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"ticket_id": "PROJ-1", ...}'
```

Si no quieres usar secret, simplemente no configures `JIRA_WEBHOOK_SECRET` (ni `JENKINS_WEBHOOK_SECRET`).

---

### DB: supabase_schema_v5 / v6 no aplicados

**Síntoma:** `POST /integrations/jenkins` falla con error de tabla no encontrada, o el widget Pipeline Health no aparece.

**Causa:** No se ejecutaron los schemas incrementales de Jenkins/Jira.

**Solución:**
```sql
-- En Supabase SQL Editor, ejecutar en orden:
-- supabase_schema_v5.sql  (pipeline_events para Jenkins)
-- supabase_schema_v6.sql  (project_tickets + commit_ticket_correlations para Jira)
```

---

## Paginación: defaults y límites

### Error: "missing field `offset`" o "missing field `limit`"

**Qué verás:**
```
Failed to deserialize query string: missing field `offset`
```

**Causa:** Antes de la corrección (Feb 2026) los campos `offset` y `limit` eran obligatorios en la query string.

**Solución:** Ya no es necesario mandarlos. Todos los endpoints de lectura paginada usan defaults si faltan:

| Campo | Default | Máximo |
|-------|---------|--------|
| `offset` | `0` | — |
| `limit` | depende del endpoint (ver abajo) | 500 |

**Defaults por endpoint:**

| Endpoint | limit default si no se manda |
|----------|------------------------------|
| `/logs` | 100 |
| `/integrations/jenkins/correlations` | 20 |
| `/signals` | 100 |
| `/governance-events` | 100 |

**Ejemplos que ahora funcionan:**
```bash
# Antes fallaba con "missing field offset"
curl -H "Authorization: Bearer $API_KEY" "$SERVER_URL/logs?limit=50"
curl -H "Authorization: Bearer $API_KEY" "$SERVER_URL/integrations/jenkins/correlations?limit=20"
curl -H "Authorization: Bearer $API_KEY" "$SERVER_URL/signals?limit=5"

# Compatibilidad hacia atrás — mandar offset explícito sigue funcionando
curl -H "Authorization: Bearer $API_KEY" "$SERVER_URL/logs?limit=50&offset=0"
```

### Cómo verificar el contrato de paginación (smoke script)

El script `gitgov/gitgov-server/tests/smoke_contract.sh` valida que los 8 checks de contrato pasen:

```bash
cd gitgov/gitgov-server/tests
SERVER_URL=http://127.0.0.1:3000 \
API_KEY=<tu_api_key> \
bash smoke_contract.sh
```

Salida esperada: `Results: 8 passed, 0 failed`

---

## Logs y Debugging

### Cómo habilitar logs detallados

**En el servidor:**
```
RUST_LOG=debug cargo run
```

**En la desktop app:**
```
RUST_LOG=gitgov=debug npm run tauri dev
```

**Niveles de log:**

| Nivel | Cuándo usar |
|-------|-------------|
| error | Solo errores críticos |
| warn | Errores recuperables |
| info | Eventos importantes |
| debug | Todo lo que pasa |
| trace | Muy detallado |

### Qué buscar en los logs

**Errores comunes:**
- "panic" - Crash del programa
- "ERROR" - Algo falló críticamente
- "WARN" - Algo inesperado pero recuperable

**En el servidor:**
- Buscar "failed" o "error"
- Ver si hay requests con status 4xx o 5xx
- Revisar queries SQL que fallan

**En la desktop app:**
- Buscar "outbox" para ver estado de eventos
- Ver "flush" para saber si se envían eventos
- Revisar errores de serialización

### Archivos importantes

| Archivo | Qué contiene |
|---------|--------------|
| ~/.gitgov/outbox.jsonl | Eventos pendientes de enviar |
| ~/.gitgov/audit.db | Base de datos SQLite local |
| stdout/stderr | Logs del servidor |

---

## Checklist de Diagnóstico

Cuando algo no funciona, verifica en orden:

**Conexión básica:**
1. [ ] El servidor está corriendo en puerto 3000
2. [ ] `curl http://127.0.0.1:3000/health` devuelve OK
3. [ ] La API key existe en la base de datos (`api_keys.is_active = true`)
4. [ ] El header es `Authorization: Bearer TU_KEY` (no `X-API-Key`)

**Outbox (eventos no llegan al servidor):**
5. [ ] `GITGOV_SERVER_URL` y `GITGOV_API_KEY` configurados en `gitgov/.env`
6. [ ] El archivo `{data_local_dir}/gitgov/outbox.jsonl` existe y tiene eventos con `"sent": false`
7. [ ] No hay mensaje `Outbox flush failed: status 401` en los logs de la app

**Dashboard (llegan eventos pero UI no muestra):**
8. [ ] `VITE_SERVER_URL` y `VITE_API_KEY` configurados en `gitgov/.env`
9. [ ] No estás mezclando `localhost` y `127.0.0.1` (usar solo `127.0.0.1`)
10. [ ] El auto-refresh (30s) no está bloqueado por CORS o red

**Base de datos:**
11. [ ] Se ejecutaron todos los schemas v1 a v6 en orden
12. [ ] No hay panics con "invalid type: null" → revisar COALESCE en queries
13. [ ] Las estructuras `ServerStats` / `CombinedEvent` coinciden frontend ↔ backend

**Rate limiting:**
14. [ ] No hay respuestas 429 en logs → ajustar `RATE_LIMIT_*` si es necesario

---

## Comandos de Emergencia

### Reiniciar el outbox

Si el outbox está atascado:

1. Hacer backup del archivo outbox.jsonl
2. Eliminar eventos ya enviados (líneas con "sent":true)
3. Reiniciar la aplicación

### Regenerar API key

Si la API key está corrupta o perdida:

1. Desactivar la key anterior en la base de datos
2. Reiniciar el servidor (genera una nueva key automáticamente)
3. Actualizar el .env de la desktop app con la nueva key

### Limpiar datos de prueba

Solo para desarrollo - eliminar eventos de testing:
1. Conectar a la base de datos
2. Eliminar eventos con user_login de prueba
3. Verificar que el dashboard se actualiza

---

## Cuándo Pedir Ayuda

Si después de seguir esta guía el problema persiste:

1. Guardar los logs completos con RUST_LOG=debug
2. Documentar los pasos exactos que causan el error
3. Incluir versión de Rust, Node.js, y sistema operativo
4. Describir qué intentaste y qué resultados obtuviste

Consulta la documentación adicional:
- Arquitectura: [ARCHITECTURE.md](./ARCHITECTURE.md)
- Guía rápida: [QUICKSTART.md](./QUICKSTART.md)
- Para agentes IA: [AGENTS.md](../AGENTS.md)
