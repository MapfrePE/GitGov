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
6. [Logs y Debugging](#logs-y-debugging)

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

1. [ ] El servidor está corriendo en puerto 3000
2. [ ] El health check devuelve OK
3. [ ] La API key existe en la base de datos
4. [ ] El header Authorization: Bearer se envía correctamente
5. [ ] Las estructuras de datos coinciden entre cliente y servidor
6. [ ] No hay NULLs donde se esperan objetos
7. [ ] El worker del outbox está iniciado
8. [ ] El archivo outbox.jsonl existe
9. [ ] Los logs no muestran errores

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
