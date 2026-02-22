# GITGOV — Roadmap Comercial y Features para Ser Vendible
## Análisis consolidado v2: Claude + GPT (con correcciones de revisión técnica)
### Estado: V1.0 IMPLEMENTADO ✓

> Este documento NO es técnico. Es estratégico. Define qué construir y en qué orden para que el producto sea vendible, no solo funcional. La versión 2 incorpora las 4 correcciones de revisión técnica de GPT para que el documento sea defendible frente a un CTO o CISO que ya conoce GitHub a fondo.

---

## ✅ IMPLEMENTADO - Paso 1: Plomería de integridad

- [x] Outbox local en desktop funcionando con JSONL + backoff exponencial
- [x] Idempotencia con event_uuid en client_events y delivery_id en github_events
- [x] Webhook ingestion verificado con HMAC secret
- [x] Health check detallado y estado de conexión visible (`GET /health/detailed`)

---

## ✅ IMPLEMENTADO - Paso 2: Correlación y Bypass Detection

- [x] Correlación client_event ↔ github_event por commit_sha
- [x] Bypass detection con tolerancia configurable
- [x] Confidence scoring (NO binario: high, medium, low)
- [x] Lenguaje orientado a evidencia (no acusaciones)
- [x] Ventana de tiempo configurable
- [x] Tolerancia de clock skew

---

## ✅ IMPLEMENTADO - Paso 3: Dashboard y Compliance

- [x] Compliance dashboard endpoint
- [x] Noncompliance signals con filtros
- [x] Export con hash SHA256
- [x] Policy history (versionado automático)

---

## Features diferenciadores - ESTADO

### 1. Correlación + Bypass Detection (V1.0) ✅ IMPLEMENTADO
Ya descrito en Corrección 3. El diferenciador más importante para el buyer de Compliance.

### 2. Policy-as-Code versionado (V1.0) ✅ IMPLEMENTADO
El `gitgov.toml` ya es versionado. Tabla `policy_history` con trigger automático. Visualización en endpoint `GET /policy/:repo/history`.

### 3. Checklist configurable antes del push (V1.0) ⏳ PENDIENTE
Feature de UX visible en demos para el buyer de Engineering. Hay que etiquetarlo correctamente como "guardrail UX", no como "security enforcement". Reduce PRs rechazados, que es un dolor medible para el Engineering Manager.

### 4. Drift Detection (V1.1) ⏳ PENDIENTE
El control plane detecta repos que se desviaron de la política. Importante: implementar con ETags y checks incrementales para no golpear los rate limits de GitHub API a escala de org.

### 5. Export PDF/Excel para auditorías (V1.1) ✅ IMPLEMENTADO
El audit log exportable con metadatos: quién exportó, cuándo, rango cubierto, hash SHA256 del contenido. Endpoint `POST /export`.

### 6. Integración Jira (V1.1) ⏳ PENDIENTE
Validación de ticket en nomenclatura de rama. El punto de GPT es correcto: la validación en desktop es UX (feedback rápido), el enforcement real va en merge gate como status check.

### 7. Cross-provider GitHub + GitLab + Bitbucket (V2.0) ⏳ PENDIENTE
Post-tracción inicial. Gran diferenciador para empresas en migración o con equipos mixtos.

### 8. Hunk staging (V2.0) ⏳ PENDIENTE
Complejo de implementar bien con libgit2. El MVP resuelve el problema real con stage por archivo completo.

---

Antes de agregar features, hay un hecho en el que GPT y yo estamos completamente de acuerdo: **el posicionamiento lo es todo.** Si GitGov se vende como "cliente de Git", pierde. Si se vende como "Git Governance Control Plane", gana. Esa distinción no es de marketing — determina qué features construyes, a quién le vendes y cuánto cobras.

El producto tiene tres capas que deben existir juntas para ser vendible:

**Capa 1 — Enforcement:** Las reglas se cumplen a través de los controles del Git host (branch protection, rulesets) orquestados por GitGov. El desktop es UX de guardrail, no la raíz de confianza del enforcement. **Esta distinción es crítica y debe estar en toda demo y en toda conversación de venta.** Si no la dices primero, el CTO te pregunta "¿y si alguien hace push desde la terminal?" y pierdes la conversación.

**Capa 2 — Evidencia:** Hay registro inmutable de todo. No editable. Auditable. Correlaciona lo que el dev intentó hacer (cliente) con lo que GitHub realmente ejecutó (webhooks).

**Capa 3 — Visibilidad:** El admin ve lo que pasa en tiempo real, con contexto, sin necesidad de saber Git. Violations, drift, bypasses, historial de políticas.

---

## Corrección 1 (GPT): Enforcement es server-side, GitGov lo orquesta

El documento anterior decía "las reglas se cumplen, no depende de la buena voluntad del dev" sin aclarar cómo. Eso es impreciso y peligroso en una venta técnica.

**La verdad completa:** Un cliente de escritorio no puede garantizar compliance porque cualquiera puede hacer `git push` desde la terminal. El enforcement real vive en GitHub mediante branch protection y rulesets, incluyendo push rulesets que pueden bloquear pushes por paths, extensiones o tamaño de archivos.

**El rol de GitGov:** No es ser el enforcer. Es ser el **control plane que estandariza, verifica y remedia** esas protecciones a escala. La diferencia es enorme:

**Nota técnica sobre fuentes de verdad (importante para el pitch enterprise):** Los webhooks de GitHub cubren bien los eventos de código (pushes, refs, commits). Para eventos de gobernanza y configuración (cambios en rulesets, branch protections modificadas, permisos cambiados), la fuente correcta es el **audit log streaming** de GitHub, que exporta estos eventos con entrega at-least-once. GitGov ingiere ambas fuentes y las normaliza en tablas separadas:

> "Webhooks para actividad de código. Audit log streaming para cambios de configuración y gobernanza. Ambos aterrizan en tablas normalizadas con contexto de política."

Esto cierra el argumento de evidencia completa sin dejar huecos que un CTO pueda explotar en una evaluación técnica.

- Sin GitGov: el admin configura branch protection repo por repo manualmente, sin visibilidad de inconsistencias.
- Con GitGov: las políticas se declaran en `gitgov.toml` versionado, el control plane las aplica a todos los repos, detecta cuando alguno se desvía, y genera evidencia del estado de cumplimiento.

**Frase para usar en demos y documentación:**

> "GitGov no reemplaza los controles de GitHub — los orquesta a escala y genera la evidencia que GitHub no guarda."

**Advertencia comercial importante — disponibilidad de push rulesets:** El posicionamiento de enforcement depende parcialmente de push rulesets de GitHub, pero estos no están disponibles universalmente. Están limitados a repos privados/internos y planes de pago específicos. Si vendes a una empresa en un plan donde no están disponibles, o con repos públicos, necesitas una historia de fallback que no dependa de este feature:

> "Sin push rulesets: branch protection + PR requerido + status checks + CODEOWNERS. GitGov sigue aportando policy-as-code versionado, drift detection, correlación de evidencia, y señales de noncompliance. El mecanismo de enforcement cambia, el valor de gobernanza no."

Tener este fallback preparado previene que un CTO cierre la conversación con "nosotros no tenemos ese plan de GitHub."

---

## Corrección 2 (GPT): El argumento de retención necesita matices

El documento anterior argumentaba "GitHub tiene 180 días de retención, nosotros guardamos más". Eso es correcto pero insuficiente frente a un enterprise admin que responda "nosotros ya hacemos streaming del audit log a S3".

**La realidad de GitHub Enterprise:**
- El audit log de orgs retiene eventos 180 días.
- Los Git events específicos tienen restricciones adicionales de retención en algunos contextos enterprise (hasta 7 días para ciertos eventos).
- GitHub Enterprise tiene audit log streaming que exporta eventos a S3/SIEM con entrega at-least-once, pero con caveats operacionales (buffering, sin garantía de orden, filtrado limitado).

**Lo que esto significa para el pitch:** Si vas contra un equipo que ya streama logs a Splunk o Elastic, "guardamos más tiempo" no cierra la venta.

**El diferenciador real de GitGov en evidencia no es retención, es:**

1. **Evidencia normalizada de gobernanza:** No solo "qué pasó" sino "qué política aplicaba en ese momento, quién la configuró, y si el evento cumplió o violó esa política." Eso no lo da el streaming de GitHub.

2. **Correlación de intención vs realidad:** "El dev intentó pushear a las 14:32 desde GitGov (client_event) y GitHub confirmó el push a las 14:33 (github_event via webhook)." O peor: "GitGov bloqueó el intento a las 14:32, pero GitHub registró un push directo a las 14:45." Eso tampoco lo da el streaming.

3. **Bypass detection como violation de primera clase:** Un evento de bypass (push directo evadiendo GitGov) es un registro explícito en la tabla de violations, no algo que hay que buscar correlacionando logs manualmente.

4. **Retención de versiones de política:** El historial de cambios de `gitgov.toml` es evidencia de gestión de cambios. Quién cambió qué regla, cuándo, en qué commit. Eso no existe en ninguna herramienta del mercado mediano.

**Frase para usar cuando el buyer dice "ya tenemos SIEM":**

> "Su SIEM tiene los eventos. GitGov tiene el contexto de gobernanza: qué política aplica, si fue un bypass, qué versión de la regla estaba vigente, y la correlación entre lo que el dev intentó y lo que GitHub ejecutó. Eso no está en su SIEM."

---

## Corrección 3 (GPT): Reorden de V1.0 — plomería antes que analytics

El roadmap anterior ponía correlación y detección de bypass en V1.0 sin haber asegurado primero que los eventos lleguen de forma confiable. Eso es un error que destruye demos.

**El problema real:** Si el desktop tiene el outbox sin flush por VPN caída, red corporativa con proxy, o firewall, el sistema no recibe el client_event. Cuando llega el github_event por webhook, el sistema lo lee como "push sin intento previo en GitGov" = BYPASS. Falso positivo. El admin ve una violation que no existió. Eso destruye la confianza en el producto en las primeras semanas de uso.

**El orden correcto de V1.0 es:**

**Paso 1 — Plomería de integridad (primero esto, nada más):**
- Outbox local en desktop funcionando con JSONL + backoff exponencial
- Idempotencia con event_uuid en client_events y delivery_id en github_events
- Webhook ingestion verificado con HMAC secret
- Health check y estado de conexión visible en la UI del desktop

**Paso 2 — Solo cuando paso 1 es confiable:**
- Correlación client_event ↔ github_event por commit_sha
- Bypass detection con tolerancia configurable (no alertar si el outbox tiene eventos pendientes para ese usuario en los últimos N minutos)

**Paso 3 — Dashboard y exports:**
- Ahora la historia es creíble porque los datos son confiables

**Paso 4 — Features de UX:**
- Checklist configurable antes del push
- Historial visual de cambios de política

La tolerancia de bypass (Paso 2) es importante: si el sistema sabe que el outbox de un dev tiene eventos pendientes sin flush, no debe generar violation por bypass hasta que el outbox se vacíe o pase un tiempo configurable (ej. 30 minutos). Sin esa lógica, los falsos positivos van a generar más ruido que valor.

**Confidence scoring en bypass detection (crítico para no hacer acusaciones falsas):**

El dashboard no debe mostrar un veredicto binario "BYPASS" sino un nivel de certeza basado en la evidencia disponible. El lenguaje importa: un CISO quiere evidencia, no acusaciones. Usar siempre terminología orientada a evidencia:

- **Señal de noncompliance (alta confianza):** GitHub registró un push directo y no existe ningún client_event del mismo actor en una ventana razonable de tiempo, con outbox vacío y conexión confirmada. Se muestra como "noncompliance signal — ruta no autorizada detectada." Solo bajo estas condiciones estrictas escala a violation formal.
- **Telemetría incompleta (baja confianza):** El outbox del dev tiene eventos pendientes sin flush, o el desktop estuvo sin conexión en esa ventana. Se muestra como "telemetría incompleta — outbox pendiente", nunca como violation ni como acusación de bypass.
- **Bypass confirmado:** Solo cuando alta confianza + revisión manual del admin. Nunca automático.

Evitar en toda la UI y documentación palabras como "bypass detectado" o "violación de seguridad" como estado automático. El lenguaje correcto es "noncompliance signal", "untrusted path detected", "missing client telemetry". La escalación a lenguaje más duro requiere intervención humana.

Las reglas de correlación deben ser explícitas en el código: ventana de tiempo configurable (default 15 minutos), correlación por commit_sha + actor_login + branch, y tolerancia de clock skew entre el timestamp del cliente y el timestamp del webhook (los relojes de máquinas no están perfectamente sincronizados).

---

## Corrección 4 (GPT): Self-host implica soporte, no solo infraestructura

El documento anterior ponía self-host en el tier Enterprise sin aclarar qué significa operacionalmente. Un comprador enterprise que escuche "self-host" asume que incluye:

- Soporte para upgrades y migraciones de schema
- Expectativas de monitoring y backup
- Posiblemente revisiones de seguridad
- En algunos casos, soporte para ambientes air-gapped

**Consecuencia en pricing:** Self-host no puede venderse a $249/mes porque el costo de soporte operacional lo hace inviable. Es un feature de Enterprise tier con precio custom precisamente porque el scope de soporte varía por cliente.

**Cómo documentarlo:**

> "Self-host es Enterprise exclusivo porque incluye soporte operacional: asistencia en upgrades, revisión de arquitectura de red, y SLA. No es solo 'una imagen Docker.'"

Esto previene que alguien en el tier Business pida self-host como si fuera un feature técnico menor.

---

## Features diferenciadores (sin cambios del análisis anterior, solo orden ajustado)

### 1. Correlación + Bypass Detection (V1.0 — después de plomería)
Ya descrito en Corrección 3. El diferenciador más importante para el buyer de Compliance.

### 2. Policy-as-Code versionado (V1.0 — gratis por diseño)
El `gitgov.toml` ya es versionado. Solo hay que visualizarlo en el dashboard como "Historial de cambios de política". No hay mucho que construir, pero tiene alto impacto en demos de Compliance.

### 3. Checklist configurable antes del push (V1.0)
Feature de UX visible en demos para el buyer de Engineering. Hay que etiquetarlo correctamente como "guardrail UX", no como "security enforcement". Reduce PRs rechazados, que es un dolor medible para el Engineering Manager.

### 4. Drift Detection (V1.1)
El control plane detecta repos que se desviaron de la política. Importante: implementar con ETags y checks incrementales para no golpear los rate limits de GitHub API a escala de org. En organizaciones con muchos repos, un polling naïve puede agotar el rate limit en minutos. La implementación correcta usa ETags para solo procesar repos que cambiaron desde el último check, y distribuye los checks en el tiempo en lugar de hacerlos todos al mismo tiempo. La remediación automática (re-aplicar ruleset) es la versión premium de este feature y debe ser opt-in — nunca automática por defecto, porque una remediación incorrecta puede bloquear el trabajo de todo un equipo.

### 5. Export PDF/Excel para auditorías (V1.1)
El audit log exportable con metadatos: quién exportó, cuándo, rango cubierto, hash SHA256 del contenido. Simple de implementar, alto valor para Compliance.

### 6. Integración Jira (V1.1)
Validación de ticket en nomenclatura de rama. El punto de GPT es correcto: la validación en desktop es UX (feedback rápido), el enforcement real va en merge gate como status check. Ambos pueden coexistir. En conversaciones de venta, llamarlo "control mapping a tickets" — ese es el lenguaje que usa el comprador enterprise con Jira.

### 7. Cross-provider GitHub + GitLab + Bitbucket (V2.0)
Post-tracción inicial. Gran diferenciador para empresas en migración o con equipos mixtos.

### 8. Hunk staging (V2.0)
Complejo de implementar bien con libgit2. El MVP resuelve el problema real con stage por archivo completo.

---

## Los dos buyers y qué necesita cada uno (sin cambios)

### Buyer A: Engineering Manager / VP Engineering / CTO
Lo que compran: reducción de fricción + visibilidad del flujo de ingeniería.
Features que cierran: checklist, drift detection, métricas de flujo, integración Jira.
Ciclo de venta: corto. Entrar por aquí.

### Buyer B: CISO / Compliance Officer
Lo que compran: evidencia inmutable + retención + control de acceso + bypass detection.
Features que cierran: append-only, correlación, export, policy versioning, bypass violations.
Ciclo de venta: largo. Expandir desde Buyer A.

**Estrategia: Land and Expand.** Entrar por Engineering Manager (decisión rápida, valor visible en días). Cuando el CISO vea los logs y pregunte "¿esto cumple con nuestra auditoría?", activar el tier de Compliance con precio mayor.

---

## Modelo de negocio (estructura sin cambios, precios como hipótesis)

GPT tiene razón: los precios son hipótesis, no compromisos. Se ajustan en las primeras 5-10 conversaciones de venta. La estructura sí es correcta.

**Starter:** Repos y devs limitados, retención 90 días, sin export, sin Jira. Precio bajo para entrada sin fricción.

**Business:** Core diferenciadores activos: drift detection, correlación, bypass detection, export PDF/Excel, integración Jira, retención 2 años.

**Enterprise:** Self-host con soporte operacional incluido, SSO/SAML, SLA, multi-provider, retención 5+ años, precio custom.

**Por qué por organización y no por usuario:** El valor es para la org, el dev no compra, y necesitas 100% de adopción para que la gobernanza funcione. Si cobras por usuario, el buyer excluye devs para ahorrar y rompe el modelo de enforcement.

---

## El pitch defensible frente a un CTO que conoce GitHub

**La objeción más común:** "GitHub ya tiene branch protection, audit log y rulesets. ¿Para qué necesito GitGov?"

**La respuesta correcta:**

"GitHub hace el enforcement — lo hace bien. El problema es que con 50 repos y 5 equipos, nadie sabe si todos los repos tienen la policy correcta aplicada hoy. Nadie puede demostrar qué política estaba vigente el día que ocurrió un incidente. Y nadie puede correlacionar 'el dev intentó hacer algo' con 'lo que GitHub realmente ejecutó'. GitGov es el control plane que gestiona eso a escala y genera la evidencia que GitHub no empaqueta para auditorías."

---

## La frase de posicionamiento (no cambió, sigue siendo la correcta)

> "GitGov es la capa de gobernanza que convierte las reglas de tu equipo de ingeniería en código versionado, enforcement orquestado, y evidencia inmutable — sin reemplazar GitHub."
