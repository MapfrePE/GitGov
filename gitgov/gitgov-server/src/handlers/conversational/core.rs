// ============================================================================
// CONVERSATIONAL CHAT — POST /chat/ask  (admin-only MVP)
// ============================================================================

const CHAT_SYSTEM_PROMPT: &str = "Eres GitGov Assistant, el asistente inteligente integrado en la plataforma GitGov.\n\
\n\
== IDENTIDAD ==\n\
Eres un asistente amigable, experto y orientado al equipo de ingeniería. Tu misión es ayudar a developers, tech leads y managers a sacar el máximo provecho de GitGov: entender métricas, resolver problemas, configurar integraciones y tomar decisiones de gobernanza informadas.\n\
Puedes conversar de forma natural. Si alguien te saluda, responde cálidamente. Si alguien hace una pregunta off-topic breve, puedes responderla brevemente y luego ofrecer ayuda con GitGov. No seas rígido ni robótico.\n\
\n\
== TEMAS EN LOS QUE ERES EXPERTO ==\n\
- Gobernanza Git: commits, pushes, branch protection, políticas, flujos de trabajo\n\
- Control Plane: endpoints, métricas, logs, dashboard, stats, jobs queue\n\
- Integraciones: GitHub (webhooks, OAuth), Jenkins (pipelines, correlaciones), Jira (tickets, cobertura), GitHub Actions\n\
- Compliance y auditoría: señales, violaciones, decisiones, retención de datos, audit trail\n\
- Configuración: API keys, roles, variables de entorno, rate limits, timezones\n\
- Desktop App: stage/commit/push, outbox offline, sync de eventos\n\
- Troubleshooting: errores 401/404/429, split-brain, deserialización, deployments\n\
- Roadmap y capacidades futuras del producto\n\
- Onboarding de organizaciones y equipos\n\
- Políticas de repositorio (gitgov.toml)\n\
- Exportación de datos y gestión de retención\n\
\n\
== REGLAS DE RESPUESTA ==\n\
- Usa EXCLUSIVAMENTE los datos/contexto provistos por el backend en el campo <data>. NO inventes datos numéricos ni eventos.\n\
- Regla no negociable: si la pregunta pide logs/eventos, responde solo con datos exactos y verificables; si no hay datos suficientes, usa status=\"insufficient_data\" (nunca inventes).\n\
- Si <data>.mode = \"project_knowledge\", prioriza los snippets incluidos para responder de forma accionable y técnica.\n\
- Si <data>.mode = \"sql_result\", interpreta los datos y explícalos en lenguaje natural.\n\
- Si los datos están vacíos o son insuficientes para responder la pregunta específica, usa status=\"insufficient_data\".\n\
- Si la capacidad solicitada no existe aún en el sistema, usa status=\"feature_not_available\".\n\
- Nunca reveles ni reconstruyas secretos (API keys, tokens, JWT, hashes). Si te lo piden, rechaza y explica la política de seguridad.\n\
- Nunca prometas que puedes mostrar el valor de una API key de un usuario. Solo puedes hablar de estado/rol cuando exista capacidad segura para ello.\n\
- Si puedes responder con los datos/contexto disponibles, usa status=\"ok\".\n\
- Para saludos, preguntas generales o conversación normal sin necesidad de datos: usa status=\"ok\" y responde naturalmente.\n\
- Responde SIEMPRE en el idioma del usuario (detecta automáticamente español, inglés u otro).\n\
- Sé claro, concreto y útil. Usa pasos numerados cuando la respuesta implique acciones. Usa ejemplos cuando sea apropiado.\n\
- Cuando respondas sobre configuración o troubleshooting, incluye el comando o endpoint exacto si lo conoces.\n\
- Cuando detectes que el usuario podría beneficiarse de una funcionalidad que no conoce, menciónala proactivamente.\n\
\n\
== PERSONALIDAD ==\n\
- Cálido pero profesional. No formal en exceso.\n\
- Directo: ve al punto, sin rodeos innecesarios.\n\
- Empático: si alguien está frustrado con un error, reconócelo antes de dar la solución.\n\
- Orientado al equipo: piensa en el impacto para el equipo, no solo para el usuario individual.\n\
- Si no sabes algo con certeza, dilo. No adivines datos del sistema.\n\
\n\
== FORMATO JSON OBLIGATORIO ==\n\
Tu respuesta DEBE ser JSON válido con este esquema exacto (sin texto fuera del JSON):\n\
{\"status\":\"ok\"|\"insufficient_data\"|\"feature_not_available\"|\"error\",\
\"answer\":\"<respuesta en lenguaje natural, markdown permitido>\",\
\"missing_capability\":\"<string descriptivo o null>\",\
\"can_report_feature\":true|false,\"data_refs\":[\"<strings con refs opcionales>\"]}\n\
\n\
Reglas por status:\n\
- ok: answer tiene la respuesta completa, can_report_feature=false, missing_capability=null.\n\
- feature_not_available: answer explica qué falta y por qué es útil, can_report_feature=true, missing_capability describe la capacidad en una frase corta.\n\
- insufficient_data: answer explica qué datos faltan o por qué no se puede responder, can_report_feature=false.\n\
- error: answer describe el problema técnico encontrado, can_report_feature=false.\n\
\n\
NUNCA devuelvas texto fuera del JSON. Nunca incluyas markdown code fences alrededor del JSON.";

const PROJECT_KNOWLEDGE_BASE: &[(&str, &[&str], &str)] = &[
    // ── PRODUCTO / VISIÓN ──────────────────────────────────────────────────
    (
        "Qué es GitGov",
        &["que es gitgov", "para que sirve", "producto", "herramienta", "plataforma", "vision", "mision"],
        "GitGov es una plataforma de gobernanza de Git distribuida. Permite a equipos de ingeniería auditar su flujo de trabajo (commits, pushes, ramas), correlacionar código con tickets y pipelines CI, detectar incumplimientos de políticas y tomar decisiones de governance con trazabilidad completa. Ideal para equipos que necesitan cumplimiento, visibilidad y control sin frenar la velocidad de desarrollo.",
    ),
    (
        "Componentes del sistema",
        &["arquitectura", "componentes", "partes", "desktop", "tauri", "axum", "supabase", "postgres", "backend", "frontend"],
        "GitGov tiene 4 componentes: (1) Desktop App — Tauri v2 + React 19 + Tailwind v4 + Zustand v5, donde el developer hace commits y pushes; (2) Control Plane — servidor Axum en Rust que recibe eventos, guarda en PostgreSQL (Supabase) y expone API REST; (3) Integraciones — GitHub webhooks, Jenkins pipelines, Jira tickets; (4) Web App — Next.js 14 en git-gov.vercel.app para marketing y documentación.",
    ),
    (
        "Desktop App funcionalidades",
        &["desktop", "app", "cliente", "tauri", "staging", "commit desde app", "push desde app", "interfaz"],
        "La Desktop App permite: ver archivos cambiados en el repo, hacer stage selectivo, escribir y hacer commit, hacer push a la rama actual, ver el dashboard del Control Plane y configurar conexión al servidor. Opera offline con outbox: si no hay conexión, los eventos se encolan y se envían al reconectarse.",
    ),
    (
        "Web App (marketing/docs)",
        &["web app", "website", "vercel", "marketing", "documentacion", "git-gov.vercel.app", "landing", "nextjs"],
        "La Web App en git-gov.vercel.app (Next.js 14) es la cara pública del producto: landing page, documentación en markdown, internacionalización EN/ES. No requiere autenticación. Se despliega automáticamente en Vercel desde la rama main.",
    ),
    // ── AUTENTICACIÓN ─────────────────────────────────────────────────────
    (
        "Auth y headers",
        &["auth", "autenticacion", "api key", "bearer", "401", "token", "x-api-key", "header", "unauthorized"],
        "Toda autenticación usa el header: `Authorization: Bearer <api_key>`. NUNCA usar `X-API-Key` (devolverá 401). El servidor calcula SHA256 del token y lo busca en la tabla api_keys. Si ves 401, verifica: (1) el header es `Authorization: Bearer ...`, (2) la API key está vigente (no revocada), (3) la key tiene el rol correcto para ese endpoint.",
    ),
    (
        "Roles y permisos",
        &["roles", "admin", "developer", "architect", "pm", "scope", "visibilidad", "permisos", "acceso"],
        "GitGov tiene 4 roles: Admin (acceso total: stats, dashboard, todas las integraciones, gestión de keys), Architect (reservado para futuras restricciones), Developer (solo ve sus propios eventos en /logs), PM (reservado). El rol se asigna al crear la API key. Admin ve toda la organización; Developer ve solo su propio contexto.",
    ),
    (
        "Crear y gestionar API keys",
        &["api keys", "crear key", "nueva key", "revocar key", "rotar", "listar keys", "emitir", "issue key"],
        "Para crear una API key: POST /api-keys con Bearer de admin, body: {\"name\":\"nombre\",\"role\":\"developer\"}. La respuesta incluye la key en texto plano (solo se muestra una vez). Para revocar: usar el panel de Settings > API Keys o endpoint POST /api-keys/{id}/revoke. Nunca exponer keys en logs, UI pública ni commits.",
    ),
    (
        "GitHub OAuth en Desktop",
        &["oauth", "device flow", "github login", "conectar github", "token github", "autenticar github"],
        "La Desktop App usa GitHub Device Flow para autenticación: el usuario ve un código, lo ingresa en github.com/login/device, y el token queda guardado en el keyring del sistema operativo (nunca en archivo plano). Este token permite ver repos, ramas y hacer push autenticado.",
    ),
    // ── CONTROL PLANE / ENDPOINTS ─────────────────────────────────────────
    (
        "Endpoints principales",
        &["endpoints", "api", "rutas", "health", "stats", "logs", "dashboard", "events"],
        "Endpoints clave del Control Plane: GET /health (sin auth, básico), GET /health/detailed (con latencia DB y uptime), POST /events (ingesta batch de eventos, Bearer auth), GET /logs (eventos combinados, dev ve solo propios), GET /stats (admin, estadísticas globales + pipeline 7d), GET /dashboard (admin, datos dashboard completo).",
    ),
    (
        "Endpoint /events (ingesta)",
        &["ingesta", "enviar eventos", "batch", "event_uuid", "events endpoint", "post events"],
        "POST /events acepta un batch de eventos del cliente. Body: {\"events\":[{\"event_uuid\":\"uuid\",\"event_type\":\"commit\",\"user_login\":\"...\",\"files\":[],\"status\":\"success\",\"timestamp\":0}],\"client_version\":\"1.0\"}. Devuelve: {\"accepted\":[...],\"duplicates\":[...],\"errors\":[...]}. Deduplicación por event_uuid.",
    ),
    (
        "Endpoint /logs",
        &["logs", "ver eventos", "historial", "actividad", "filtrar logs", "paginacion"],
        "GET /logs devuelve eventos combinados con paginación. Preferir keyset con before_created_at + before_id. Params opcionales: limit, before_created_at, before_id, offset (compatibilidad legada, deprecado), event_type, user_login, repo, start_ts, end_ts. Admin ve todos; Developer ve solo sus propios eventos. Response: {\"events\":[...],\"stale\"?:bool,\"deprecations\"?:string[]}.",
    ),
    (
        "Endpoint /stats",
        &["stats", "estadisticas", "metricas", "total commits", "total pushes", "kpis"],
        "GET /stats (admin) devuelve ServerStats: total_events, total_commits, total_pushes, blocked_pushes, active_developers, active_repos, pipeline_health (7d), commits_by_day, top_repos, top_developers. Útil para dashboards de management.",
    ),
    (
        "Endpoint /dashboard",
        &["dashboard", "panel", "vista", "resumen", "admin dashboard"],
        "GET /dashboard (admin) devuelve datos consolidados del dashboard: commits recientes, pipelines, señales activas, correlaciones CI/Jira. Es el endpoint que alimenta la vista principal del dashboard en la Desktop App.",
    ),
    (
        "Jobs queue y métricas",
        &["jobs", "queue", "cola", "job queue", "dead jobs", "retry job", "jobs metrics"],
        "GET /jobs/metrics (admin) devuelve métricas del job worker: pending, running, completed, failed. GET /jobs/dead lista jobs muertos. POST /jobs/{job_id}/retry reintenta un job muerto. El worker tiene TTL de 300s, polling cada 5s y backoff exponencial (base 10s, máx x32).",
    ),
    (
        "Endpoint /compliance/{org}",
        &["compliance", "cumplimiento", "org compliance", "dashboard compliance", "reporte"],
        "GET /compliance/{org_name} (admin) devuelve dashboard de compliance de la organización: porcentaje de commits con ticket, pushes bloqueados, señales activas, violaciones pendientes. Permite una vista ejecutiva del estado de gobernanza.",
    ),
    (
        "Endpoint /export",
        &["export", "exportar", "exportacion", "descargar datos", "audit export"],
        "POST /export (Bearer auth) exporta eventos en formato estructurado. Crea un registro de auditoría del export (quién exportó, cuándo, qué rango). Útil para compliance reporting externo o integración con SIEM.",
    ),
    (
        "Endpoint /governance-events",
        &["governance events", "branch protection events", "governance", "eventos governance"],
        "GET /governance-events devuelve eventos de governance: cambios de branch protection, modificaciones de política, overrides administrativos. Permite auditar quién cambió qué configuración de gobernanza.",
    ),
    // ── INTEGRACIONES ────────────────────────────────────────────────────
    (
        "Integración GitHub webhooks",
        &["github", "webhook", "push github", "hmac", "firma", "x-hub-signature", "github webhook", "delivery"],
        "GitHub se integra via POST /webhooks/github. Requiere configurar GITHUB_WEBHOOK_SECRET en el servidor y en la config del webhook de GitHub. La firma HMAC se valida con X-Hub-Signature-256. X-GitHub-Delivery se usa para idempotencia. Soporta eventos: push, create (ramas/tags).",
    ),
    (
        "Configurar webhook GitHub",
        &["configurar github", "setup github", "webhook url", "github setup", "como integrar github"],
        "Para configurar GitHub webhook: (1) En GitHub repo/org > Settings > Webhooks > Add webhook, (2) URL: https://tu-servidor/webhooks/github, (3) Content type: application/json, (4) Secret: mismo valor que GITHUB_WEBHOOK_SECRET en el servidor, (5) Eventos: push, branch_or_tag_creation. El servidor responde 200 si la firma es válida.",
    ),
    (
        "Integración Jenkins",
        &["jenkins", "pipeline", "ci jenkins", "build jenkins", "jenkins webhook", "jenkins status", "jenkins correlacion"],
        "Jenkins ingesta por POST /integrations/jenkins con Bearer auth. Si JENKINS_WEBHOOK_SECRET está configurado, se exige header x-gitgov-jenkins-secret. Estado de integración: GET /integrations/jenkins/status. Correlaciones commit↔pipeline: GET /integrations/jenkins/correlations. Policy check advisory: POST /policy/check.",
    ),
    (
        "Configurar Jenkins",
        &["configurar jenkins", "setup jenkins", "jenkins setup", "como integrar jenkins", "jenkins gitgov"],
        "Para integrar Jenkins con GitGov: (1) En Jenkins, configurar un post-build action o pipeline step con HTTP POST a https://tu-servidor/integrations/jenkins, (2) Header: Authorization: Bearer <admin_api_key>, (3) Opcionalmente: header x-gitgov-jenkins-secret si JENKINS_WEBHOOK_SECRET está configurado en el servidor, (4) Body: {\"job_name\":\"...\",\"build_number\":N,\"status\":\"SUCCESS\"|\"FAILURE\",\"commit_sha\":\"...\",\"branch\":\"...\",\"duration_ms\":N}.",
    ),
    (
        "Jenkins correlaciones",
        &["jenkins correlaciones", "commit pipeline", "ci correlation", "build commit", "correlacion jenkins"],
        "GitGov correlaciona automáticamente commits con builds de Jenkins usando el commit SHA. GET /integrations/jenkins/correlations devuelve pares commit↔pipeline con estado del build. Visible en la tabla de commits del dashboard como badges de CI (verde/rojo).",
    ),
    (
        "Integración Jira",
        &["jira", "ticket jira", "issue jira", "jira webhook", "jira status", "jira correlate"],
        "Jira ingesta por POST /integrations/jira con Bearer auth. Si JIRA_WEBHOOK_SECRET está configurado, se exige header x-gitgov-jira-secret. Estado: GET /integrations/jira/status. Correlación batch: POST /integrations/jira/correlate. Cobertura: GET /integrations/jira/ticket-coverage. Detalle de ticket: GET /integrations/jira/tickets/{id}.",
    ),
    (
        "Configurar Jira",
        &["configurar jira", "setup jira", "jira setup", "como integrar jira", "jira gitgov"],
        "Para integrar Jira con GitGov: (1) En Jira: Project Settings > Webhooks > Create webhook, (2) URL: https://tu-servidor/integrations/jira, (3) Eventos: issue updated, issue created, comment created, (4) Header adicional (si se configura JIRA_WEBHOOK_SECRET): x-gitgov-jira-secret: <secreto>. GitGov también puede recibir eventos de Jira via Automation rules que hacen HTTP POST.",
    ),
    (
        "Cobertura de tickets Jira",
        &["ticket coverage", "cobertura tickets", "commits sin ticket", "ticket huerfano", "orphan commit", "sin ticket"],
        "GET /integrations/jira/ticket-coverage devuelve: porcentaje de commits con ticket asociado, lista de commits sin ticket (huérfanos), y tickets sin commits (tickets vacíos). Esto alimenta la métrica de compliance 'cobertura de tickets' visible en el dashboard.",
    ),
    (
        "Correlación commit-ticket",
        &["correlacion commit ticket", "commit jira", "vincular ticket", "ticket commit", "coverage", "batch correlate"],
        "POST /integrations/jira/correlate dispara correlación batch: busca en mensajes de commit referencias a tickets (ej. PROJ-123, ABC-456) y crea registros de correlación. También detecta commits sin ticket para reporting. Los badges de ticket aparecen en la tabla de commits del dashboard.",
    ),
    (
        "GitHub Actions",
        &["github actions", "actions", "workflow", "gha", "actions ci"],
        "GitGov no tiene endpoint dedicado para GitHub Actions aún. La trazabilidad CI está implementada vía Jenkins. Para usar GitHub Actions: opción A — crear un workflow step que envíe eventos al endpoint de Jenkins con el formato compatible; opción B — solicitar la capacidad nativa como feature request. Roadmap: V1.2-C incluye mejoras al correlation engine.",
    ),
    // ── GOLDEN PATH / FLUJO ───────────────────────────────────────────────
    (
        "Golden Path (flujo completo)",
        &["golden path", "flujo completo", "flujo base", "flujo events", "como funciona el flujo", "pipeline datos"],
        "El Golden Path es el flujo sagrado de GitGov: (1) Developer abre Desktop App y ve archivos cambiados, (2) Hace stage de los archivos deseados, (3) Escribe mensaje de commit y hace commit, (4) Hace push a la rama remota, (5) Desktop envía eventos [stage_files, commit, attempt_push, successful_push/blocked_push] a POST /events del Control Plane, (6) Control Plane guarda en PostgreSQL con deduplicación, (7) Dashboard muestra los datos sin errores 401.",
    ),
    (
        "Tipos de eventos",
        &["tipos eventos", "event_type", "stage_files", "commit event", "push event", "blocked_push", "successful_push", "attempt_push"],
        "Tipos de eventos que registra GitGov: stage_files (archivos marcados para commit), commit (commit realizado con SHA, mensaje, archivos), attempt_push (intento de push iniciado), successful_push (push completado exitosamente, rama destino), blocked_push (push rechazado por política o protección de rama). Cada evento lleva event_uuid único para deduplicación.",
    ),
    (
        "Outbox y modo offline",
        &["outbox", "retry", "backoff", "offline", "sin conexion", "cola eventos", "reintentos", "flush"],
        "Si el servidor no está disponible, la Desktop App encola los eventos en un outbox local. El worker de background intenta flush periódicamente con backoff exponencial. Al recuperar la conexión, los eventos se envían en orden. Garantiza que ningún evento se pierda por desconexión temporal. El outbox usa el mismo formato que /events.",
    ),
    (
        "Branch protection y push bloqueado",
        &["rama protegida", "protected branch", "push bloqueado", "blocked", "main bloqueado", "politica push"],
        "Cuando un push viola una política o rama protegida (ej. push directo a main sin PR), se registra un evento blocked_push. Estos eventos: (1) aparecen en el log de actividad, (2) incrementan la métrica de blocked_pushes en /stats, (3) pueden generar una señal de no-compliance si la política lo configura. El developer ve el bloqueo en la Desktop App.",
    ),
    // ── POLÍTICAS ─────────────────────────────────────────────────────────
    (
        "Sistema de políticas (gitgov.toml)",
        &["politica", "policy", "gitgov.toml", "reglas repo", "configuracion repo", "policy check"],
        "Cada repositorio puede tener un archivo gitgov.toml que define políticas: ramas protegidas, requisito de ticket en commit, revisores obligatorios, etc. GET /policy/{repo_name} devuelve la política actual. POST /policy/check hace validación advisory (usado por Jenkins). PUT /policy/{repo_name}/override aplica un override administrativo.",
    ),
    (
        "Historial de políticas",
        &["historial politica", "policy history", "cambios politica", "audit politica"],
        "GET /policy/{repo_name}/history devuelve el historial de cambios de política del repositorio: quién cambió qué, cuándo, y el valor anterior vs nuevo. Permite auditar la evolución de las reglas de gobernanza.",
    ),
    (
        "Policy check en Jenkins",
        &["policy check jenkins", "advisory jenkins", "check antes deploy", "policy advisory"],
        "POST /policy/check es un endpoint advisory para Jenkins: recibe {\"repo\":\"...\",\"branch\":\"...\",\"commit_sha\":\"...\",\"actor\":\"...\"} y devuelve si el commit cumple con las políticas del repo. No bloquea el pipeline (es advisory), pero el resultado puede usarse en gates del pipeline de Jenkins.",
    ),
    // ── COMPLIANCE Y SEÑALES ─────────────────────────────────────────────
    (
        "Señales de no-compliance",
        &["signals", "señales", "noncompliance", "no compliance", "alertas", "incumplimiento"],
        "Las señales son indicadores de posible incumplimiento detectados automáticamente. GET /signals lista señales activas. POST /signals/{signal_id} actualiza una señal. POST /signals/{signal_id}/confirm (admin) confirma que se detectó un bypass real. POST /signals/detect/{org_name} dispara detección manual de señales para la org.",
    ),
    (
        "Violaciones y decisiones",
        &["violations", "violaciones", "decisions", "decisiones", "historial decisiones", "resolver violacion"],
        "Cuando una señal se confirma, puede generar una violación. GET /violations/{id}/decisions lista el historial de decisiones sobre esa violación. POST /violations/{id}/decisions (admin) añade una decisión: {\"decision\":\"waived\"|\"escalated\"|\"resolved\",\"reason\":\"...\",\"actor\":\"...\"}. Permite trazabilidad de quién tomó qué acción y por qué.",
    ),
    (
        "Audit stream GitHub",
        &["audit stream", "github audit", "audit log", "github audit log", "org audit"],
        "POST /audit-stream/github (admin) ingesta el audit log stream de GitHub Enterprise o la API de audit log de GitHub. Permite correlacionar acciones en GitHub (creación de repos, cambios de permisos, etc.) con actividad de desarrollo en GitGov.",
    ),
    // ── CONFIGURACIÓN / DEPLOYMENT ────────────────────────────────────────
    (
        "Variables de entorno del servidor",
        &["variables entorno", "env", "configuracion servidor", "env vars", "secrets", "database url"],
        "Variables clave del servidor (.env): DATABASE_URL (PostgreSQL/Supabase), GITGOV_API_KEY (key del cliente desktop), GITGOV_JWT_SECRET (firmar JWTs, DEBE cambiarse en prod), GITHUB_WEBHOOK_SECRET, JENKINS_WEBHOOK_SECRET (opcional), JIRA_WEBHOOK_SECRET (opcional), GEMINI_API_KEY (para el chatbot), GEMINI_MODEL (modelo Gemini a usar), RUST_LOG (nivel de logging), GITGOV_SERVER_ADDR (default 0.0.0.0:3000).",
    ),
    (
        "Rate limits configurables",
        &["rate limit", "throttle", "429", "too many requests", "limite peticiones", "rate vars"],
        "Rate limits configurables via variables de entorno: GITGOV_RATE_LIMIT_EVENTS_PER_MIN (default 240, ruta /events), GITGOV_RATE_LIMIT_AUDIT_STREAM_PER_MIN (default 60), GITGOV_RATE_LIMIT_JENKINS_PER_MIN (default 120), GITGOV_RATE_LIMIT_JIRA_PER_MIN (default 120), GITGOV_RATE_LIMIT_ADMIN_PER_MIN (default 60 para endpoints admin generales), GITGOV_RATE_LIMIT_LOGS_PER_MIN (default hereda ADMIN para /logs), GITGOV_RATE_LIMIT_STATS_PER_MIN (default hereda ADMIN para /stats, /stats/daily y /dashboard). Si recibes 429, aumenta el límite correspondiente.",
    ),
    (
        "Deploy en EC2 con systemd",
        &["ec2", "deploy", "aws", "systemd", "nginx", "produccion", "restart server", "deploy produccion"],
        "En producción, el Control Plane corre en EC2 con systemd y Nginx como reverse proxy. Para actualizar: (1) compilar release `cargo build --release`, (2) copiar binario al host, (3) `sudo systemctl restart gitgov-server`. Si un endpoint nuevo no responde (404), es probable que el binary desplegado sea anterior al último cambio.",
    ),
    (
        "Pipeline CI/CD para deploy",
        &["pipeline deploy", "auto deploy", "ci deploy", "jenkins deploy", "build release", "publicar"],
        "El deploy efectivo requiere un pipeline que: (1) compile `cargo build --release --target x86_64-unknown-linux-gnu`, (2) transfiera el binario al host EC2 (ej. scp o S3), (3) reinicie el servicio systemd. Si Jenkins solo corre lint/tests sin el paso de deploy, el runtime en EC2 no se actualiza.",
    ),
    (
        "HTTPS en producción",
        &["https", "ssl", "tls", "certificado", "lets encrypt", "dominio", "seguridad produccion"],
        "HTTPS en EC2 está en la lista de alta prioridad del roadmap. La recomendación es usar Let's Encrypt con certbot + Nginx como reverse proxy. En producción sin HTTPS, las API keys se transmiten en claro — riesgo de seguridad alto. Alternativa rápida: AWS ALB con certificado ACM.",
    ),
    (
        "Anti split-brain (localhost vs 127.0.0.1)",
        &["split brain", "localhost", "127.0.0.1", "ipv6", "eventos no aparecen", "dashboard vacio"],
        "Problema clásico: Desktop envía eventos pero el dashboard no los muestra. Causa: 'localhost' puede resolver a IPv6 (::1) y pegar a un proceso diferente. Fix: usar siempre 127.0.0.1. El código normaliza localhost→127.0.0.1 en 4 lugares. Si el dashboard aparece vacío, verificar que la URL del servidor en Settings usa 127.0.0.1:3000 (no localhost:3000). Docker server va en 127.0.0.1:3001.",
    ),
    (
        "Configuración de timezone",
        &["timezone", "zona horaria", "timestamp", "utc", "hora", "configurar hora"],
        "Los timestamps se almacenan en UTC en PostgreSQL. La Desktop App permite seleccionar la timezone de visualización en Settings. Esto afecta cómo se muestran los timestamps en el dashboard y la tabla de commits, pero no altera los datos almacenados. Para auditoría, siempre se puede ver el timestamp UTC raw.",
    ),
    // ── RETENCIÓN Y DATOS ────────────────────────────────────────────────
    (
        "Retención de datos y compliance",
        &["retencion", "retention", "5 anos", "5 years", "borrar datos", "datos historicos", "audit retention"],
        "AUDIT_RETENTION_DAYS define cuánto tiempo se retienen los eventos de auditoría, con un mínimo legal de 1825 días (5 años). client_sessions tiene una retención separada más corta para TTL operativo (CLIENT_SESSION_RETENTION_DAYS). Las tablas de auditoría son append-only: no se hacen UPDATE ni DELETE en eventos históricos.",
    ),
    (
        "Append-only y deduplicación",
        &["append only", "no borrar", "inmutabilidad", "audit trail", "deduplicacion", "event_uuid"],
        "Los eventos de auditoría son inmutables (append-only). No se hacen UPDATE ni DELETE. La deduplicación se garantiza por el campo event_uuid (UUID v4 único por evento). Si se reenvía el mismo event_uuid, el servidor lo clasifica como 'duplicate' y no lo inserta de nuevo. Esto permite reintentos seguros del outbox.",
    ),
    (
        "Datos de prueba y contaminación",
        &["test data", "sintetico", "synthetic", "datos prueba", "dev_team", "e2e", "contaminacion metricas"],
        "Para evitar contaminación de métricas con datos de prueba: GITGOV_REJECT_SYNTHETIC_LOGINS=true rechaza logins sintéticos (user_logins que coincidan con patrones de test). En pipelines E2E, usar user_logins distintos a los de producción. El endpoint /export también permite exportar solo rangos específicos para separar datos.",
    ),
    // ── DASHBOARD Y UI ────────────────────────────────────────────────────
    (
        "Dashboard del Control Plane",
        &["dashboard ui", "panel control", "auto refresh", "tabla commits", "widgets", "dashboard vista"],
        "El dashboard se refresca automáticamente cada 30 segundos. Carga datos en paralelo: stats, commits recientes, pipeline health, señales activas y correlaciones. La tabla de commits muestra badges de CI (Jenkins) y badges de ticket (Jira). Filtros disponibles: rango de fechas, developer, repositorio.",
    ),
    (
        "Tabla de commits recientes",
        &["tabla commits", "commits recientes", "recent commits", "badge ci", "badge ticket", "commit table"],
        "La tabla de commits recientes muestra: SHA del commit (abreviado), mensaje, developer, repo, rama, timestamp, badge de CI (verde si pipeline pasó, rojo si falló, gris si no correlacionado) y badge(s) de ticket Jira (clickeable si está configurado). Permite identificar rápidamente qué commits tienen cobertura de tickets y CI.",
    ),
    (
        "Settings de la Desktop App",
        &["settings", "configuracion desktop", "server url", "api key settings", "preferencias"],
        "En Settings de la Desktop App: URL del servidor (prioridad: input manual > .env VITE_SERVER_URL > localStorage > default 127.0.0.1:3000), API Key (prioridad similar), timezone de visualización, preferencias de notificaciones. Los cambios se aplican inmediatamente sin reiniciar la app.",
    ),
    (
        "Settings para administración",
        &["settings admin", "admin settings", "onboarding en settings", "gestion equipo settings", "api keys settings"],
        "Settings concentra la administración operativa: onboarding admin (org, invitaciones, provisión por rol), gestión de equipo (developers y repos por actividad real) y administración de API keys. Export JSON se mantiene fuera de ese bloque por diseño.",
    ),
    (
        "PR y merges",
        &["pr", "pull request", "merge", "merges", "evidencia merge", "pr-merges"],
        "GitGov expone evidencia de merges vía GET /pr-merges (admin). Se usa para auditoría de cumplimiento y para distinguir cambios por PR versus pushes directos.",
    ),
    (
        "Docs y FAQ",
        &["docs", "faq", "ayuda", "help", "soporte", "documentacion"],
        "Existe documentación en web (`/docs`) y página de Ayuda/FAQ en la app desktop. El asistente puede guiar con pasos concretos y comandos/endpoints exactos cuando aplique.",
    ),
    (
        "Pipeline Health widget",
        &["pipeline health", "widget pipeline", "ci health", "salud pipeline", "jenkins widget"],
        "El widget Pipeline Health (parte del dashboard, V1.2-A) muestra el estado de pipelines en los últimos 7 días: tasa de éxito, builds fallidos, promedio de duración. Requiere integración Jenkins activa. Si no hay datos de Jenkins, el widget muestra estado vacío o N/A.",
    ),
    // ── ONBOARDING ────────────────────────────────────────────────────────
    (
        "Onboarding de organización",
        &["onboarding", "org", "organizacion", "crear org", "primera vez", "empezar", "setup inicial"],
        "Flujo de onboarding admin: (1) Crear/upsert la organización, (2) Definir org activa para la sesión, (3) Provisionar miembros por rol (admin, developer) o generar invitaciones por email, (4) Cada developer acepta invitación y obtiene su API key, (5) Developers configuran la Desktop App con la URL del servidor y su API key, (6) Configurar integraciones (GitHub webhook, Jenkins, Jira) según necesidad.",
    ),
    (
        "Gestión de equipo",
        &["team", "equipo", "developers", "repos", "actividad", "miembros", "team management"],
        "La vista de gestión de equipo muestra: developers activos (con último evento), repos activos (con último push), actividad por ventana temporal (7d/30d/90d), estado por developer (activo/inactivo). Permite filtrar por días y estado. Es útil para que el tech lead vea quién está trabajando y en qué.",
    ),
    (
        "Invitaciones y provisioning",
        &["invitacion", "invitar developer", "provisionar", "nueva cuenta", "agregar miembro"],
        "Para agregar developers a la organización: (1) Admin genera invitación con rol asignado, (2) Developer recibe el token de invitación, (3) Developer acepta con POST /invitations/{token}/accept, (4) El sistema crea el registro de cliente y emite una API key, (5) Developer configura su Desktop App con la key recibida.",
    ),
    // ── TROUBLESHOOTING ───────────────────────────────────────────────────
    (
        "Error 401 Unauthorized",
        &["401", "unauthorized", "no autorizado", "error auth", "forbidden", "403"],
        "Causas de 401: (1) Usando header incorrecto — debe ser `Authorization: Bearer <key>`, NO `X-API-Key`, (2) API key revocada o expirada, (3) Rol insuficiente para el endpoint (ej. developer intentando acceder a /stats que requiere admin), (4) Key hasheada incorrectamente en DB (raro, verificar que la key no tenga espacios extra). Fix: verificar header, verificar estado de la key en Settings > API Keys.",
    ),
    (
        "Error 404 Not Found",
        &["404", "not found", "endpoint no existe", "ruta no encontrada"],
        "Causas de 404: (1) Endpoint nuevo que aún no está desplegado en el servidor de producción — el binary desplegado es anterior, (2) URL mal formada, (3) Chat endpoint /chat/ask no disponible si el backend no fue recompilado con la feature. Fix: verificar versión desplegada y reiniciar el servicio systemd.",
    ),
    (
        "Error 429 Too Many Requests",
        &["429", "too many requests", "rate limit error", "cuota", "throttled"],
        "429 significa que se superó el rate limit del endpoint. Causas: flujo de eventos muy frecuente, loop de reintentos sin backoff, o rate limit configurado demasiado bajo. Fix: (1) Aumentar GITGOV_RATE_LIMIT_*_PER_MIN en el .env del servidor, (2) Verificar que el outbox tiene backoff exponencial y no está en loop, (3) Revisar si hay un cliente mal configurado enviando eventos duplicados.",
    ),
    (
        "Error de deserialización query string",
        &["deserializacion", "missing field", "query string error", "offset missing", "limit missing", "parse error"],
        "Error 'Failed to deserialize query string: missing field offset/limit': el cliente no enviaba campos de paginación. Fix aplicado (Feb 2026): todos los structs de query tienen #[serde(default)] en limit y offset. Si aparece este error, verificar que el servidor esté en la versión más reciente. Backward compatible: valores default son 0.",
    ),
    (
        "Error de serialización / null JSON",
        &["serialization error", "null json", "json null", "invalid type null", "coalesce", "panic json"],
        "Error 'invalid type: null, expected a map': ocurre cuando json_object_agg() devuelve NULL sin filas. Fix: siempre usar COALESCE(json_object_agg(...), '{}') en SQL + #[serde(default)] en structs Rust. Si aparece en producción, revisar las queries en db.rs que agreguen JSON.",
    ),
    (
        "Dashboard vacío o sin datos",
        &["dashboard vacio", "no hay datos", "sin actividad", "eventos no llegan", "logs vacios"],
        "Si el dashboard aparece vacío: (1) Verificar split-brain: URL del servidor debe ser 127.0.0.1:3000 (no localhost), (2) Verificar que el servidor local está corriendo (curl http://127.0.0.1:3000/health), (3) Verificar que la API key en Settings coincide con GITGOV_API_KEY del servidor, (4) Revisar el outbox — puede estar reteniendo eventos por fallo de conexión, (5) Verificar logs del servidor (RUST_LOG=info) para ver si llegan requests.",
    ),
    (
        "Chatbot no responde o da error",
        &["chatbot error", "chat no funciona", "gemini error", "llm error", "chat 404", "bot roto"],
        "Errores comunes del chatbot: (1) 404 en /chat/ask — backend desactualizado sin deploy, (2) 401 — API key sin rol admin, (3) 429 — cuota de Gemini agotada (verificar en Google AI Studio), (4) Error de modelo deprecado — actualizar GEMINI_MODEL en .env del servidor, (5) GEMINI_API_KEY no configurada. El chatbot requiere GEMINI_API_KEY y GEMINI_MODEL válidos.",
    ),
    (
        "JWT_SECRET inseguro",
        &["jwt secret", "jwt inseguro", "token forjado", "jwt default", "cambiar jwt"],
        "ADVERTENCIA: GITGOV_JWT_SECRET tiene un default hardcodeado: 'gitgov-secret-key-change-in-production'. Si no se cambia en producción, cualquiera puede forjar tokens JWT. Fix: establecer un secreto fuerte con `openssl rand -hex 32` y configurarlo en el .env del servidor. NUNCA usar el valor por defecto en producción.",
    ),
    // ── CAPACIDADES DEL CHATBOT ───────────────────────────────────────────
    (
        "Consultas analíticas del chatbot",
        &["chatbot sql", "consultas chat", "preguntas soportadas", "quien hizo push", "blocked pushes", "commits usuario", "analitica chat"],
        "El chatbot soporta consultas analíticas determinísticas con datos reales de DB: pushes a main sin ticket (7d), pushes bloqueados del mes, commits por usuario (conteo/rango), y también variantes por usuario para bloqueos/sin-ticket. Si una pregunta no entra en estas capacidades, responde con límites explícitos en vez de inventar.",
    ),
    (
        "Capacidades y limitaciones del chatbot",
        &["chatbot capacidades", "que puede hacer el bot", "limitaciones bot", "feature bot", "bot faq"],
        "El chatbot PUEDE: responder preguntas sobre el proyecto, consultar datos analíticos soportados, explicar configuración, ayudar en troubleshooting, orientar sobre integraciones. El chatbot NO PUEDE: acceder a datos fuera del scope de la API key, ejecutar acciones (crear keys, revocar, push), inventar datos numéricos. Si falta una capacidad, puede reportar el feature request.",
    ),
    (
        "Feature requests desde el chat",
        &["feature request", "reportar feature", "capacidad faltante", "solicitar funcion", "pedir feature"],
        "Cuando el chatbot detecta que una capacidad no existe, devuelve status='feature_not_available' y can_report_feature=true. El usuario puede entonces confirmar el feature request, que se registra via POST /feature-requests y puede disparar un webhook para triage de producto. Es la forma de comunicar necesidades directamente desde el chat.",
    ),
    // ── SEGURIDAD ─────────────────────────────────────────────────────────
    (
        "Seguridad y buenas prácticas",
        &["seguridad", "security", "keys", "tokens", "secretos", "best practices", "vulnerabilidades"],
        "Prácticas de seguridad en GitGov: (1) API keys hasheadas con SHA256 antes de guardar en DB (nunca en texto plano), (2) Tokens OAuth en keyring del OS (no en archivos), (3) .env NUNCA se commitea al repo, (4) HTTPS obligatorio en producción, (5) JWT_SECRET debe ser fuerte y único por entorno, (6) Eventos de auditoría son append-only (no modificables), (7) No exponer secretos en logs ni en respuestas de error.",
    ),
    (
        "Erasing / derecho al olvido",
        &["erase", "borrar usuario", "derecho olvido", "gdpr", "eliminar datos", "privacidad"],
        "GitGov tiene soporte para 'erase' de usuario: endpoint que anonimiza o elimina registros de un developer específico dentro del scope de la org. Devuelve 404 si el usuario no existe en la org (privacy-preserving: indistinguible de 'no existe'). Útil para cumplimiento GDPR/LOPD.",
    ),
    // ── ROADMAP ───────────────────────────────────────────────────────────
    (
        "Roadmap y estado del proyecto",
        &["roadmap", "futuro", "proximas features", "cuando", "version", "v1.2", "v1.3", "pendiente"],
        "Estado actual (Feb 2026): V1.2-A (Jenkins MVP) — FUNCIONAL. V1.2-B (Jira + Ticket Coverage) — PREVIEW. Pendiente alta prioridad: correlación de related_prs automática, HTTPS en EC2 (Let's Encrypt), tests de integración con DB mock. Roadmap futuro: V1.2-C (Correlation Engine V2 + Compliance Signals), V1.3 (AI Governance Insights).",
    ),
    (
        "Tests y calidad",
        &["tests", "pruebas", "cargo test", "ci tests", "unit tests", "e2e tests", "smoke tests"],
        "Suite de tests: (1) cargo test — 36 unit tests en CI (models, handlers, auth), sin DB real, (2) smoke_contract.sh — 14 contract checks live (paginación + Golden Path), requiere servidor corriendo, (3) e2e_flow_test.sh — flujo E2E completo (manual), (4) jenkins_integration_test.sh y jira_integration_test.sh — tests de integración manuales. Pendiente: tests de integración con DB mock.",
    ),
    // ── CONVERSACIÓN NATURAL ──────────────────────────────────────────────
    (
        "Saludo y presentación",
        &["hola", "buenos dias", "buenas tardes", "hello", "hi", "hey", "como estas", "que tal"],
        "Soy GitGov Assistant, el asistente integrado en la plataforma GitGov. Puedo ayudarte con: métricas de tu equipo, configuración de integraciones (GitHub/Jenkins/Jira), troubleshooting, compliance, políticas de repositorio, y cualquier pregunta sobre cómo sacar el máximo de GitGov. ¿En qué te puedo ayudar hoy?",
    ),
    (
        "Ayuda general",
        &["ayuda", "help", "que puedes hacer", "como me ayudas", "menu", "opciones", "comandos"],
        "Puedo ayudarte con: (1) Analítica — 'Quién hizo push a main sin ticket esta semana?', 'Cuántos commits tiene X este mes?', (2) Configuración — setup de GitHub/Jenkins/Jira, API keys, variables de entorno, (3) Troubleshooting — errores 401/404/429, dashboard vacío, outbox atascado, (4) Compliance — señales, violaciones, cobertura de tickets, (5) Producto — qué hace GitGov, roadmap, capacidades. ¿Por dónde empezamos?",
    ),
    (
        "Métricas para manager / tech lead",
        &["metricas manager", "kpis", "reporte equipo", "vista ejecutiva", "tech lead", "gerente", "jefe"],
        "Para managers y tech leads, GitGov ofrece: (1) /stats — total commits, pushes, blocked pushes, developers activos, repos activos, (2) /compliance/{org} — porcentaje de commits con ticket, señales activas, violaciones pendientes, (3) Tabla de commits con badges CI y tickets, (4) Pipeline Health (7 días), (5) Cobertura de tickets Jira. Todo con trazabilidad de auditoría completa.",
    ),
    (
        "Métricas para developer",
        &["metricas developer", "mis commits", "mi actividad", "como ver mis datos", "developer vista"],
        "Como developer, puedes ver: (1) tus propios eventos en GET /logs (solo tus datos, scoped por tu API key), (2) el historial de commits, pushes y stages en el dashboard de la Desktop App, (3) estado de los pipelines Jenkins correlacionados con tus commits, (4) badges de tickets Jira en tus commits. Para ver datos de toda la org, necesitas rol admin.",
    ),
    (
        "Cuánto cuesta / pricing",
        &["precio", "costo", "pricing", "plan", "licencia", "gratis", "enterprise"],
        "La información de pricing y planes de GitGov está disponible en git-gov.vercel.app. GitGov está orientado a equipos de ingeniería que necesitan gobernanza y compliance. Para preguntas de licenciamiento enterprise, contacta al equipo de GitGov directamente.",
    ),

];

// Knowledge extracted from the public web docs/FAQ (gitgov-web/content/docs).
const WEB_FAQ_KNOWLEDGE_BASE: &[(&str, &[&str], &str)] = &[
    (
        "GitGov no es open source",
        &["open source", "codigo abierto", "código abierto", "repo publico", "repositorio publico"],
        "GitGov no es open source; es un producto de software orientado a trazabilidad operacional para organizaciones.",
    ),
    (
        "Plataformas de GitGov Desktop",
        &["windows", "mac", "macos", "linux", "plataformas", "sistema operativo", "desktop support", "soporta", "compatibilidad", "supported"],
        "GitGov Desktop soporta Windows, macOS y Linux (stack Tauri v2).",
    ),
    (
        "GitGov no lee código fuente",
        &["lee codigo", "código fuente", "source code", "diffs", "contenido de archivos", "chat accede codigo"],
        "GitGov no lee ni transmite código fuente, diffs ni contenido de archivos. Solo captura metadatos operacionales de Git.",
    ),
    (
        "GitGov no monitorea pantalla o teclado",
        &["pantalla", "teclado", "monitoriza", "monitoring", "clipboard", "ide", "telemetria", "telemetría"],
        "GitGov no monitorea pantalla, teclado, clipboard, navegador ni IDE fuera de operaciones Git.",
    ),
    (
        "Desktop App sin privilegios de admin",
        &["admin privileges", "privilegios administrador", "requiere admin", "instalar", "instalacion"],
        "La Desktop App no requiere privilegios de administrador para instalarse; se instala en espacio de usuario.",
    ),
    (
        "Modo offline con outbox local",
        &["offline", "sin internet", "sin conexion", "sin conexión", "outbox", "reintentos"],
        "Si se pierde conectividad, los eventos se encolan en un outbox JSONL local con backoff exponencial y se sincronizan al reconectar.",
    ),
    (
        "Cómo actualizar GitGov Desktop",
        &["actualizar", "updates", "stable", "beta", "changelog", "update app"],
        "Actualización de Desktop: Settings/Configuración > Updates, seleccionar canal (Stable/Beta), buscar actualizaciones, descargar e instalar.",
    ),
    (
        "Integraciones soportadas",
        &["integraciones", "integrations", "github", "jenkins", "jira", "audit stream"],
        "Integraciones soportadas: GitHub (webhooks push/create y audit stream), Jenkins (ingesta + correlación commit-pipeline), Jira (ingesta + correlación + cobertura).",
    ),
    (
        "Self-host de Control Plane",
        &["self host", "self-host", "autohospedar", "on premise", "control plane"],
        "Sí, el Control Plane se puede self-hostear en infraestructura propia con binario Rust + PostgreSQL.",
    ),
    (
        "Comportamiento cuando el servidor cae",
        &["server down", "servidor caido", "servidor caído", "caido", "caído", "downtime"],
        "Si el servidor cae, la Desktop App sigue operando localmente y sincroniza eventos cuando el servidor vuelve.",
    ),
    (
        "Chat y privacidad",
        &["chat codigo", "chat código", "assistant source code", "chat accede repo", "asistente accede codigo"],
        "El chat usa metadatos de eventos y conocimiento del producto; no accede al código fuente.",
    ),
    (
        "Si el chat no sabe la respuesta",
        &["no sabe", "no responde", "datos insuficientes", "feature request", "capacidad faltante"],
        "Cuando no hay datos suficientes o falta una capacidad, el chat responde explícitamente y puede registrar un feature request para el equipo de producto.",
    ),
    (
        "Pricing y contacto comercial",
        &["pricing", "precio", "costo", "licenciamiento", "enterprise", "soporte", "contacto"],
        "Para pricing, planes y contacto comercial, la referencia oficial es git-gov.vercel.app y sus canales de contacto.",
    ),
    (
        "Warning de firma en Windows",
        &["smartscreen", "firma windows", "warning windows", "unsigned", "code signing"],
        "Un warning en Windows suele aparecer cuando el instalador no está firmado con un certificado reconocido por SmartScreen.",
    ),
];

#[derive(Debug, serde::Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, serde::Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, serde::Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
    response_mime_type: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    system_instruction: GeminiSystemInstruction,
    contents: Vec<GeminiContent>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Debug, serde::Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationalRuntime {
    sessions: HashMap<String, ConversationState>,
}

const MAX_CONVERSATION_SESSIONS: usize = 2_000;
const CONVERSATION_IDLE_TTL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConversationState {
    #[serde(default)]
    turns: Vec<ConversationTurn>,
    #[serde(default)]
    slots: ConversationSlots,
    #[serde(default)]
    todos: Vec<TodoItem>,
    #[serde(default)]
    learning: LearningState,
    #[serde(default)]
    project_snapshot: serde_json::Value,
    #[serde(default)]
    session_started_ms: i64,
    #[serde(default)]
    last_project_snapshot_ms: i64,
    #[serde(default)]
    last_updated_ms: i64,
    #[serde(default)]
    next_todo_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConversationTurn {
    role: String,
    text: String,
    timestamp_ms: i64,
    intent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConversationSlots {
    #[serde(default)]
    last_user_login: Option<String>,
    #[serde(default)]
    last_repo: Option<String>,
    #[serde(default)]
    last_branch: Option<String>,
    #[serde(default)]
    last_org_name: Option<String>,
    #[serde(default)]
    last_time_window: Option<String>,
    #[serde(default)]
    last_intent: Option<String>,
    #[serde(default)]
    preferred_language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LearningState {
    #[serde(default)]
    intent_usage: HashMap<String, u64>,
    #[serde(default)]
    total_interactions: u64,
    #[serde(default)]
    successful_answers: u64,
    #[serde(default)]
    insufficient_answers: u64,
    #[serde(default)]
    positive_feedback: u64,
    #[serde(default)]
    negative_feedback: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum TodoStatus {
    #[default]
    Pending,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TodoItem {
    id: u64,
    text: String,
    status: TodoStatus,
    created_at_ms: i64,
    #[serde(default)]
    completed_at_ms: Option<i64>,
    source: String,
    priority: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum NlpIntent {
    Greeting,
    Farewell,
    Gratitude,
    AskDateTime,
    AskCapabilities,
    GuidedHelp,
    QueryAnalytics,
    TodoAdd,
    TodoList,
    TodoComplete,
    FeedbackPositive,
    FeedbackNegative,
    Unknown,
}

impl NlpIntent {
    fn as_str(self) -> &'static str {
        match self {
            NlpIntent::Greeting => "greeting",
            NlpIntent::Farewell => "farewell",
            NlpIntent::Gratitude => "gratitude",
            NlpIntent::AskDateTime => "ask_datetime",
            NlpIntent::AskCapabilities => "ask_capabilities",
            NlpIntent::GuidedHelp => "guided_help",
            NlpIntent::QueryAnalytics => "query_analytics",
            NlpIntent::TodoAdd => "todo_add",
            NlpIntent::TodoList => "todo_list",
            NlpIntent::TodoComplete => "todo_complete",
            NlpIntent::FeedbackPositive => "feedback_positive",
            NlpIntent::FeedbackNegative => "feedback_negative",
            NlpIntent::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct NlpEntities {
    #[serde(default)]
    user_login: Option<String>,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    org_name: Option<String>,
    #[serde(default)]
    time_window: Option<String>,
    #[serde(default)]
    todo_text: Option<String>,
    #[serde(default)]
    todo_id: Option<u64>,
    language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NlpAnalysis {
    intent: NlpIntent,
    confidence: f32,
    entities: NlpEntities,
    reasoning: String,
}

impl Default for NlpAnalysis {
    fn default() -> Self {
        Self {
            intent: NlpIntent::Unknown,
            confidence: 0.0,
            entities: NlpEntities {
                language: "es".to_string(),
                ..NlpEntities::default()
            },
            reasoning: "No intent detected".to_string(),
        }
    }
}

/// Pattern matching for the 3 supported queries.
#[derive(Debug, Clone)]
enum ChatQuery {
    ControlPlaneExecutiveSummary,
    OnlineDevelopersNow {
        minutes: i64,
    },
    CommitsWithoutTicketWindow {
        hours: i64,
    },
    PushesNoTicket,
    BlockedPushesMonth,
    UserPushesCount {
        user: String,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
    },
    UserActivityMonth {
        user: String,
    },
    UserPushesNoTicketWeek {
        user: String,
    },
    UserBlockedPushesMonth {
        user: String,
    },
    SessionCommitsCount {
        user: Option<String>,
    },
    TotalCommitsCount,
    UserCommitsCount {
        user: String,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
    },
    UserLastCommit {
        user: String,
    },
    UserCommitsRange { user: String, start_ms: i64, end_ms: i64 },
    UserAccessProfile {
        user: String,
    },
    UserScopeClarification {
        user: String,
    },
    NeedUserForCommitHistory,
    Greeting,
    DateMismatchClarification,
    CurrentDateTime,
    CapabilityOverview,
    GuidedHelp,
}

#[derive(Debug, Clone)]
struct RankedKnowledgeSnippet {
    score: i32,
    title: &'static str,
    content: &'static str,
    source: &'static str,
}

fn detect_language(question: &str) -> String {
    let q = question.to_lowercase();
    let english_markers = [
        "hello", "help", "how", "what", "when", "where", "commit", "push", "settings", "please",
    ];
    let spanish_markers = [
        "hola", "ayuda", "como", "cómo", "qué", "cuando", "cuándo", "donde", "dónde", "por favor",
    ];
    let en_hits = english_markers.iter().filter(|m| q.contains(**m)).count();
    let es_hits = spanish_markers.iter().filter(|m| q.contains(**m)).count();
    if en_hits > es_hits {
        "en".to_string()
    } else {
        "es".to_string()
    }
}

fn extract_repo_name(q: &str) -> Option<String> {
    let repo_re = Regex::new(r"(?:repo|repositorio)\s*[:=]?\s*([a-z0-9_\-\.]+/[a-z0-9_\-\.]+)").ok()?;
    repo_re
        .captures(q)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_branch_name(q: &str) -> Option<String> {
    let branch_re = Regex::new(r"(?:rama|branch)\s*[:=]?\s*([a-z0-9_\-\/\.]+)").ok()?;
    branch_re
        .captures(q)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_todo_add_text(q: &str) -> Option<String> {
    let trimmed = q.trim();
    if let Some(rest) = trimmed.strip_prefix("todo:") {
        let t = rest.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let add_re = Regex::new(
        r"(?:agrega|añade|anota|crea|create|add)\s+(?:una\s+)?(?:tarea|todo)\s*[:\-]?\s*(.+)$",
    )
    .ok()?;
    add_re
        .captures(trimmed)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_todo_complete_id(q: &str) -> Option<u64> {
    let done_re = Regex::new(r"(?:completa|complete|done|cerrar|resolver)\s+(?:tarea|todo)?\s*#?\s*(\d+)").ok()?;
    done_re
        .captures(q)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
}

fn analyze_nlp(question: &str, session: &ConversationState) -> NlpAnalysis {
    let q = question.trim().to_lowercase();
    let mut entities = NlpEntities {
        language: detect_language(&q),
        user_login: extract_user_login(&q).or_else(|| session.slots.last_user_login.clone()),
        repo: extract_repo_name(&q).or_else(|| session.slots.last_repo.clone()),
        branch: extract_branch_name(&q).or_else(|| session.slots.last_branch.clone()),
        org_name: session.slots.last_org_name.clone(),
        time_window: session.slots.last_time_window.clone(),
        todo_text: extract_todo_add_text(&q),
        todo_id: extract_todo_complete_id(&q),
    };

    if q.contains("esta semana") || q.contains("this week") {
        entities.time_window = Some("week".to_string());
    } else if q.contains("este mes") || q.contains("this month") {
        entities.time_window = Some("month".to_string());
    } else if q.contains("todo el historial") || q.contains("all history") {
        entities.time_window = Some("all_time".to_string());
    }

    if entities.todo_text.is_some() {
        return NlpAnalysis {
            intent: NlpIntent::TodoAdd,
            confidence: 0.95,
            entities,
            reasoning: "Detected TODO add command".to_string(),
        };
    }
    if q.contains("mis tareas")
        || q.contains("lista de tareas")
        || q.contains("pendientes")
        || q.contains("todo list")
        || q.contains("list todos")
    {
        return NlpAnalysis {
            intent: NlpIntent::TodoList,
            confidence: 0.9,
            entities,
            reasoning: "Detected TODO list command".to_string(),
        };
    }
    if entities.todo_id.is_some() {
        return NlpAnalysis {
            intent: NlpIntent::TodoComplete,
            confidence: 0.9,
            entities,
            reasoning: "Detected TODO completion command".to_string(),
        };
    }

    if q.contains("gracias") || q.contains("thanks") || q.contains("thank you") {
        return NlpAnalysis {
            intent: NlpIntent::Gratitude,
            confidence: 0.88,
            entities,
            reasoning: "Detected gratitude".to_string(),
        };
    }
    if q.contains("no sirve")
        || q.contains("mala respuesta")
        || q.contains("incorrecto")
        || q.contains("wrong")
        || q.contains("bad answer")
    {
        return NlpAnalysis {
            intent: NlpIntent::FeedbackNegative,
            confidence: 0.82,
            entities,
            reasoning: "Detected negative feedback".to_string(),
        };
    }
    if q.contains("bien hecho") || q.contains("excelente") || q.contains("good job") {
        return NlpAnalysis {
            intent: NlpIntent::FeedbackPositive,
            confidence: 0.82,
            entities,
            reasoning: "Detected positive feedback".to_string(),
        };
    }
    if q.contains("adios") || q.contains("hasta luego") || q.contains("bye") || q.contains("goodbye") {
        return NlpAnalysis {
            intent: NlpIntent::Farewell,
            confidence: 0.9,
            entities,
            reasoning: "Detected farewell".to_string(),
        };
    }
    if q.contains("hola")
        || q.contains("hello")
        || q.contains("hi ")
        || q.contains("buenos")
        || q.contains("buenas")
    {
        return NlpAnalysis {
            intent: NlpIntent::Greeting,
            confidence: 0.86,
            entities,
            reasoning: "Detected greeting".to_string(),
        };
    }
    if q.contains("fecha")
        || q.contains("hora")
        || q.contains("today")
        || q.contains("time")
        || q.contains("qué día")
        || q.contains("que dia")
    {
        return NlpAnalysis {
            intent: NlpIntent::AskDateTime,
            confidence: 0.9,
            entities,
            reasoning: "Detected date/time question".to_string(),
        };
    }
    if q.contains("puedes ver datos")
        || q.contains("puedes consultar")
        || q.contains("control plane")
        || q.contains("capacidad")
    {
        return NlpAnalysis {
            intent: NlpIntent::AskCapabilities,
            confidence: 0.78,
            entities,
            reasoning: "Detected capability question".to_string(),
        };
    }
    if q.contains("ayuda")
        || q.contains("help")
        || q.contains("paso a paso")
        || q.contains("como conectar")
        || q.contains("cómo conectar")
    {
        return NlpAnalysis {
            intent: NlpIntent::GuidedHelp,
            confidence: 0.8,
            entities,
            reasoning: "Detected guided-help intent".to_string(),
        };
    }

    if detect_query(question).is_some() {
        return NlpAnalysis {
            intent: NlpIntent::QueryAnalytics,
            confidence: 0.82,
            entities,
            reasoning: "Detected analytics intent from query engine patterns".to_string(),
        };
    }

    NlpAnalysis {
        intent: NlpIntent::Unknown,
        confidence: 0.25,
        entities,
        reasoning: "No deterministic intent matched; fallback to LLM".to_string(),
    }
}

fn build_conversation_key(auth_user: &AuthUser, scoped_org_id: Option<&str>) -> String {
    format!(
        "{}::{}",
        auth_user.client_id,
        scoped_org_id.unwrap_or("global")
    )
}

fn load_conversation_state(state: &Arc<AppState>, key: &str) -> ConversationState {
    let mut runtime = state
        .conversational_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime
        .sessions
        .entry(key.to_string())
        .or_default()
        .clone()
}

fn ensure_session_initialized(session: &mut ConversationState) {
    let now = chrono::Utc::now().timestamp_millis();
    if session.session_started_ms <= 0 {
        session.session_started_ms = now;
    }
    if session.last_updated_ms <= 0 {
        session.last_updated_ms = now;
    }
}

fn save_conversation_state(state: &Arc<AppState>, key: &str, session: ConversationState) {
    let mut runtime = state
        .conversational_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime.sessions.insert(key.to_string(), session);
    prune_conversation_runtime(&mut runtime);
}

fn prune_conversation_runtime(runtime: &mut ConversationalRuntime) {
    let now = chrono::Utc::now().timestamp_millis();
    runtime.sessions.retain(|_, s| {
        if s.last_updated_ms <= 0 {
            return true;
        }
        now.saturating_sub(s.last_updated_ms) <= CONVERSATION_IDLE_TTL_MS
    });

    if runtime.sessions.len() <= MAX_CONVERSATION_SESSIONS {
        return;
    }

    let mut by_oldest: Vec<(String, i64)> = runtime
        .sessions
        .iter()
        .map(|(k, s)| (k.clone(), s.last_updated_ms))
        .collect();
    by_oldest.sort_by_key(|(_, ts)| *ts);

    let excess = runtime.sessions.len().saturating_sub(MAX_CONVERSATION_SESSIONS);
    for (k, _) in by_oldest.into_iter().take(excess) {
        runtime.sessions.remove(&k);
    }
}

fn push_turn(session: &mut ConversationState, role: &str, text: &str, intent: &str) {
    let now = chrono::Utc::now().timestamp_millis();
    session.turns.push(ConversationTurn {
        role: role.to_string(),
        text: text.to_string(),
        timestamp_ms: now,
        intent: intent.to_string(),
    });
    if session.turns.len() > 20 {
        let keep_from = session.turns.len().saturating_sub(20);
        session.turns = session.turns.split_off(keep_from);
    }
    session.last_updated_ms = now;
}

fn update_slots_from_nlp(session: &mut ConversationState, nlp: &NlpAnalysis, org_name: Option<&str>) {
    if let Some(ref user) = nlp.entities.user_login {
        session.slots.last_user_login = Some(user.clone());
    }
    if let Some(ref repo) = nlp.entities.repo {
        session.slots.last_repo = Some(repo.clone());
    }
    if let Some(ref branch) = nlp.entities.branch {
        session.slots.last_branch = Some(branch.clone());
    }
    if let Some(ref window) = nlp.entities.time_window {
        session.slots.last_time_window = Some(window.clone());
    }
    if let Some(org) = org_name {
        session.slots.last_org_name = Some(org.to_string());
    }
    session.slots.last_intent = Some(nlp.intent.as_str().to_string());
    session.slots.preferred_language = Some(nlp.entities.language.clone());
}

fn update_learning(session: &mut ConversationState, intent: NlpIntent, status: &str) {
    let key = intent.as_str().to_string();
    *session.learning.intent_usage.entry(key).or_insert(0) += 1;
    session.learning.total_interactions += 1;
    match status {
        "ok" => session.learning.successful_answers += 1,
        "insufficient_data" => session.learning.insufficient_answers += 1,
        _ => {}
    }
    if intent == NlpIntent::FeedbackPositive {
        session.learning.positive_feedback += 1;
    }
    if intent == NlpIntent::FeedbackNegative {
        session.learning.negative_feedback += 1;
    }
}

fn add_todo(session: &mut ConversationState, text: &str, source: &str, priority: &str) -> TodoItem {
    let now = chrono::Utc::now().timestamp_millis();
    let id = if session.next_todo_id == 0 { 1 } else { session.next_todo_id };
    session.next_todo_id = id + 1;
    let item = TodoItem {
        id,
        text: text.to_string(),
        status: TodoStatus::Pending,
        created_at_ms: now,
        completed_at_ms: None,
        source: source.to_string(),
        priority: priority.to_string(),
    };
    session.todos.push(item.clone());
    item
}

fn complete_todo(session: &mut ConversationState, todo_id: u64) -> Option<TodoItem> {
    let now = chrono::Utc::now().timestamp_millis();
    for item in &mut session.todos {
        if item.id == todo_id && item.status == TodoStatus::Pending {
            item.status = TodoStatus::Completed;
            item.completed_at_ms = Some(now);
            return Some(item.clone());
        }
    }
    None
}

fn render_todo_list(session: &ConversationState, language: &str) -> String {
    let pending: Vec<&TodoItem> = session
        .todos
        .iter()
        .filter(|t| t.status == TodoStatus::Pending)
        .collect();
    if pending.is_empty() {
        return if language == "en" {
            "No pending TODO tasks. If you want, I can create one from your next action.".to_string()
        } else {
            "No tienes tareas TODO pendientes. Si quieres, te creo una desde tu próxima acción.".to_string()
        };
    }
    let mut lines: Vec<String> = Vec::new();
    for task in pending {
        lines.push(format!(
            "- #{} [{}] {} (source: {})",
            task.id, task.priority, task.text, task.source
        ));
    }
    if language == "en" {
        format!("Pending TODO tasks:\n{}", lines.join("\n"))
    } else {
        format!("Tareas TODO pendientes:\n{}", lines.join("\n"))
    }
}

fn apply_proactive_todos_from_snapshot(session: &mut ConversationState) -> Vec<String> {
    let mut created: Vec<String> = Vec::new();
    let snapshot = &session.project_snapshot;
    let blocked_pushes = snapshot
        .get("stats")
        .and_then(|s| s.get("client_events"))
        .and_then(|s| s.get("blocked_today"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let unresolved = snapshot
        .get("stats")
        .and_then(|s| s.get("violations"))
        .and_then(|s| s.get("unresolved"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let dead_jobs = snapshot
        .get("job_metrics")
        .and_then(|m| m.get("dead"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let has_todo_like = |needle: &str, todos: &[TodoItem]| -> bool {
        todos.iter().any(|t| t.status == TodoStatus::Pending && t.text.contains(needle))
    };

    if blocked_pushes > 0 && !has_todo_like("pushes bloqueados", &session.todos) {
        let item = add_todo(
            session,
            "Revisar causas de pushes bloqueados y ajustar policy/branch protection",
            "proactive.blocked_pushes",
            "high",
        );
        created.push(format!("#{} {}", item.id, item.text));
    }
    if unresolved > 0 && !has_todo_like("violaciones pendientes", &session.todos) {
        let item = add_todo(
            session,
            "Revisar violaciones pendientes y registrar decisiones de auditoría",
            "proactive.violations",
            "high",
        );
        created.push(format!("#{} {}", item.id, item.text));
    }
    if dead_jobs > 0 && !has_todo_like("dead jobs", &session.todos) {
        let item = add_todo(
            session,
            "Revisar dead jobs en /jobs/dead y ejecutar retry donde aplique",
            "proactive.jobs",
            "medium",
        );
        created.push(format!("#{} {}", item.id, item.text));
    }
    created
}
