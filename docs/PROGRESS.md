# GitGov - Registro de Progreso

## Documentación del Proyecto

| Documento | Propósito |
|-----------|-----------|
| [AGENTS.md](../AGENTS.md) | Instrucciones para agentes de IA |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Arquitectura del sistema explicada |
| [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) | Guía de solución de problemas |
| [QUICKSTART.md](./QUICKSTART.md) | Guía de inicio rápido |

---

## Estado Actual: Sistema Funcional

### Qué funciona hoy

La versión actual de GitGov tiene todas las funcionalidades básicas operativas:

**Desktop App**
- Inicia correctamente y muestra el dashboard principal
- Conecta con GitHub vía OAuth
- Permite hacer commits y pushes
- Registra eventos en el outbox local
- Envía eventos al servidor cuando hay conexión

**Control Plane Server**
- Corre en localhost:3000
- Recibe y almacena eventos de las desktop apps
- Autentica requests con API keys
- Proporciona endpoints para dashboards y estadísticas

**Pipeline de Eventos**
- Los eventos fluyen desde Desktop → Server → PostgreSQL → Dashboard
- La deduplicación funciona (event_uuid único)
- Los eventos se muestran en tiempo real

### Visualización del Dashboard

El dashboard muestra:

```
┌────────────────────────────────────────────────────────────────────┐
│  Conectado al Control Plane                                        │
│  URL del servidor: http://localhost:3000                           │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌───────────┐ │
│  │ Total GitHub │ │ Pushes Hoy   │ │ Bloqueados   │ │Devs Activ │ │
│  │      0       │ │      0       │ │      0       │ │     1     │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └───────────┘ │
│                                                                    │
│  Tasa de Éxito: 100.0%          │  Eventos Cliente por Estado     │
│  Repos Activos: 0               │  ┌─────────────────────────┐    │
│                                 │  │ success: 25             │    │
│                                 │  └─────────────────────────┘    │
│                                                                    │
│  Eventos Recientes:                                                │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │ Hora              │ Usuario   │ Tipo            │ Estado     ││
│  ├────────────────────────────────────────────────────────────────┤│
│  │ 22/2/2026 5:45:41 │ MapfrePE  │ successful_push │ success    ││
│  │ 22/2/2026 5:45:41 │ MapfrePE  │ attempt_push    │ success    ││
│  │ 22/2/2026 5:45:13 │ MapfrePE  │ commit          │ success    ││
│  │ 22/2/2026 5:44:43 │ MapfrePE  │ stage_files     │ success    ││
│  └────────────────────────────────────────────────────────────────┘│
└────────────────────────────────────────────────────────────────────┘
```

---

## Historia del Proyecto

### Fase 1: Sincronización Control Plane (22 de Febrero, 2026)

**El problema:** La desktop app no podía comunicarse con el servidor. Los eventos no llegaban y el dashboard permanecía vacío.

**Los bugs encontrados y resueltos:**

**Bug 1 - Panic en get_stats()**

El servidor crasheaba cuando intentaba obtener estadísticas. Resulta que PostgreSQL devuelve NULL cuando una función de agregación no tiene datos, pero Rust esperaba un objeto vacío.

La solución fue doble: modificar las queries SQL para usar COALESCE (que devuelve un valor por defecto cuando hay NULL), y agregar atributos en Rust para que los campos HashMap tengan valores default.

**Bug 2 - Serialización ServerStats**

El cliente y el servidor tenían estructuras de datos diferentes. El cliente esperaba campos planos, el servidor enviaba objetos anidados.

Se sincronizaron las estructuras en ambos lados para que coincidan exactamente.

**Bug 3 - Serialización CombinedEvent**

Similar al anterior. El endpoint /logs enviaba eventos en un formato que el cliente no esperaba.

Se agregó el tipo CombinedEvent en el cliente y se actualizó el frontend.

**Bug 4 - 401 Unauthorized**

El outbox enviaba eventos pero el servidor los rechazaba. El problema: el header de autenticación era incorrecto.

El servidor esperaba `Authorization: Bearer`, pero el outbox enviaba `X-API-Key`. Se corrigió en dos lugares del código.

**Resultado:** El pipeline completo funciona. Los eventos fluyen desde la desktop app hasta el dashboard.

---

### Fase 2: Pipeline de Eventos End-to-End (22 de Febrero, 2026)

**El logro:** El sistema ahora registra correctamente todos los eventos desde el desktop hasta el Control Plane.

**Cómo funciona el flujo:**

1. El usuario hace push en la desktop app
2. La app registra "attempt_push" en el outbox local
3. Ejecuta el push real a GitHub
4. Si tiene éxito, registra "successful_push" en el outbox
5. El worker de background envía los eventos al servidor
6. El servidor los guarda en PostgreSQL
7. El dashboard muestra los eventos en tiempo real

**Tipos de eventos registrados:**

| Evento | Cuándo se genera |
|--------|------------------|
| attempt_push | Antes de cada push |
| successful_push | Push completado |
| blocked_push | Push a rama protegida |
| push_failed | Push falló |
| commit | Commit creado |
| stage_files | Archivos agregados al staging |
| create_branch | Rama creada |
| blocked_branch | Creación de rama bloqueada |

---

### Fase 3: Production Hardening (21 de Febrero, 2026)

**El objetivo:** Preparar el sistema para producción con mejoras de robustez.

**Mejoras implementadas:**

**Job Queue Production-Grade**

El sistema de jobs en background tenía varios problemas de concurrencia que se resolvieron:

- **Race conditions:** Se implementó `FOR UPDATE SKIP LOCKED` para que múltiples workers no tomen el mismo job
- **Explosión de jobs:** Se agregó deduplicación con índice único
- **Reintentos infinitos:** Backoff exponencial con máximo de intentos y dead-letter queue
- **Reset peligroso:** Solo se pueden resetear jobs que realmente están atascados

**Cursor Incremental Seguro**

El cursor que marca qué eventos ya se procesaron usaba `created_at`, que es el tiempo del evento en GitHub. Pero los eventos pueden llegar tarde (retries, backlogs).

Se agregó un campo `ingested_at` que es el tiempo cuando el evento llegó al servidor. El cursor ahora usa este campo.

**Append-Only Triggers**

Se verificó que todas las tablas de auditoría son append-only:
- github_events: 100% inmutable
- client_events: 100% inmutable
- violations: Solo se puede cambiar el estado de resolución
- noncompliance_signals: 100% inmutable
- governance_events: 100% inmutable

**Job Metrics Endpoint**

Se agregó `/jobs/metrics` para ver el estado del queue:
- Cuántos jobs pending
- Cuántos running
- Cuántos dead
- Tiempos promedio

**Seguridad del Bootstrap**

El servidor imprimía la API key de bootstrap en los logs, lo cual es un problema en Docker/Kubernetes donde los logs son visibles.

Se implementó:
- Flag `--print-bootstrap-key` para explícitamente mostrar la key
- Detección de TTY para solo mostrar en terminal interactiva
- En Docker (sin TTY), la key no aparece en logs

**Stress Tests**

Se creó una suite de tests de stress:
- Idempotencia de webhooks
- Deduplicación de jobs
- Reset de jobs atascados
- Múltiples organizaciones
- Alto volumen de webhooks

---

### Fase 4: Audit Stream Endpoint (21 de Febrero, 2026)

**El objetivo:** Recibir eventos de gobernanza desde GitHub.

**Qué se implementó:**

Un nuevo endpoint `/audit-stream/github` que recibe batches de audit logs de GitHub. Estos logs incluyen:

- Cambios en branch protection
- Modificaciones de rulesets
- Cambios de permisos
- Cambios de acceso de teams

Se creó una nueva tabla `governance_events` para almacenar estos eventos, también append-only.

---

### Fase 5: Autenticación y Correlación (21 de Febrero, 2026)

**Middleware de Autenticación**

Se implementó un sistema completo de autenticación con roles:

- **admin:** Acceso total
- **developer:** Solo puede ver sus propios eventos

Los endpoints están protegidos según el nivel requerido:
- `/stats`, `/dashboard`: Solo admin
- `/logs`: Admin ve todo, developer solo sus eventos
- `/events`: Cualquier usuario autenticado
- `/webhooks/github`: Valida firma HMAC (sin JWT)

**Correlación y Confidence Scoring**

El sistema de detección de violaciones ahora es más sofisticado:

- **confidence = 'high':** Señal clara de bypass
- **confidence = 'low':** Telemetría incompleta, necesita investigación

No se muestra "BYPASS DETECTADO" automáticamente. Solo cuando un humano lo confirma.

**Violation Decisions**

Se separó la resolución de violaciones en una tabla separada:

Los tipos de decisión:
- acknowledged: Alguien vio la violación
- false_positive: No era una violación real
- resolved: Se resolvió el problema
- escalated: Se escaló a nivel superior
- dismissed: Se decidió ignorar
- wont_fix: Se decidió no arreglar

Esto crea un historial completo de cada violación.

---

## Qué Falta por Hacer

### Prioridad Alta

| Componente | Qué falta |
|------------|-----------|
| Webhooks GitHub | Recibir y procesar eventos push de GitHub |
| Correlation Engine | Correlacionar client_events con github_events |
| Drift Detection | Detectar cuando la configuración difiere de la política |

### Prioridad Media

| Componente | Qué falta |
|------------|-----------|
| Tests automatizados | Expandir cobertura de tests |
| Documentación | Agregar más ejemplos y casos de uso |
| Performance | Optimizar queries para organizaciones grandes |

---

## Build Status

Los builds compilan con warnings menores (variables no usadas, código muerto), sin errores.

- Desktop (Tauri): Compila correctamente
- Server (Axum): Compila correctamente
- Clippy: Solo warnings de estilo, sin errores

---

## Archivos Clave del Proyecto

| Ubicación | Qué hace |
|-----------|----------|
| gitgov/src-tauri/src/outbox/ | Cola de eventos offline |
| gitgov/src-tauri/src/commands/ | Operaciones Git |
| gitgov-server/src/auth.rs | Middleware de autenticación |
| gitgov-server/src/handlers.rs | Handlers HTTP |
| gitgov-server/src/models.rs | Estructuras de datos |
| gitgov-server/supabase_schema.sql | Schema de la base de datos |

---

## Próximos Pasos

1. **Configurar webhooks de GitHub** en los repositorios
2. **Implementar correlation engine** para detectar bypasses
3. **Agregar drift detection** para validación de políticas
4. **Expandir tests** para mayor cobertura
5. **Deploy a producción** cuando esté listo
