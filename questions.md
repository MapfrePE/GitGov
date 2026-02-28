# Questions Audit (Integraciones + Lógica de Negocio)

Fecha: 2026-02-28  
Scope: backend `gitgov-server`, desktop dashboard `gitgov`, web `gitgov-web`.

---

## 1) ¿Jenkins está realmente integrado end-to-end?

Respuesta: Sí. Hay ingesta, health y correlación commit→pipeline expuestos y cableados.

Evidencia en código:  
- `gitgov/gitgov-server/src/main.rs:526` (`POST /integrations/jenkins`)  
- `gitgov/gitgov-server/src/main.rs:534` (`GET /integrations/jenkins/status`)  
- `gitgov/gitgov-server/src/main.rs:535` (`GET /integrations/jenkins/correlations`)  
- `gitgov/gitgov-server/src/handlers.rs:101` (`ingest_jenkins_pipeline_event`)  
- `gitgov/gitgov-server/src/handlers.rs:621` (`get_jenkins_commit_correlations`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 2) ¿Jira está implementado para cobertura de tickets?

Respuesta: Sí. Hay ingest webhook, status, detalle de ticket, correlación batch y métricas de cobertura.

Evidencia en código:  
- `gitgov/gitgov-server/src/main.rs:537` (`POST /integrations/jira`)  
- `gitgov/gitgov-server/src/main.rs:545` (`GET /integrations/jira/status`)  
- `gitgov/gitgov-server/src/main.rs:546` (`GET /integrations/jira/tickets/{ticket_id}`)  
- `gitgov/gitgov-server/src/main.rs:547` (`POST /integrations/jira/correlate`)  
- `gitgov/gitgov-server/src/main.rs:548` (`GET /integrations/jira/ticket-coverage`)  
- `gitgov/gitgov-server/src/handlers.rs:318` (`ingest_jira_webhook`)  
- `gitgov/gitgov-server/src/handlers.rs:483` (`correlate_jira_tickets`)  
- `gitgov/gitgov-server/src/handlers.rs:587` (`get_jira_ticket_coverage`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 3) ¿Ya existe auditoría por día (commits/pushes)?

Respuesta: Sí. Hay endpoint diario y query UTC por serie de días.

Evidencia en código:  
- `gitgov/gitgov-server/src/main.rs:517` (`GET /stats/daily`)  
- `gitgov/gitgov-server/src/handlers.rs:1978` (`get_daily_activity`)  
- `gitgov/gitgov-server/src/db.rs:1654` (`get_daily_activity`)  
- `gitgov/gitgov-server/src/db.rs:1663` (`generate_series ... NOW() AT TIME ZONE 'UTC'`)  
- `gitgov/gitgov-server/src/db.rs:1672` (conteo `commit`)  
- `gitgov/gitgov-server/src/db.rs:1673` (conteo `successful_push`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 4) ¿Se guarda evidencia de PR merge + aprobadores?

Respuesta: Sí. Se procesa webhook `pull_request`, se consultan approvals y se expone por `/pr-merges`.

Evidencia en código:  
- `gitgov/gitgov-server/src/handlers.rs:1268` (dispatch `"pull_request"`)  
- `gitgov/gitgov-server/src/handlers.rs:1509` (`fetch_pr_approvers`)  
- `gitgov/gitgov-server/src/handlers.rs:1560` (`process_pull_request_event`)  
- `gitgov/gitgov-server/src/handlers.rs:1637` (payload `gitgov.approvals_count`)  
- `gitgov/gitgov-server/src/main.rs:581` (`GET /pr-merges`)  
- `gitgov/gitgov-server/src/db.rs:2069` (lectura `/gitgov/approvals_count`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 5) ¿La auditoría admin es append-only y cubre acciones críticas?

Respuesta: Sí. `admin_audit_log` es append-only y se registra en confirm/revoke/export/policy_override.

Evidencia en código:  
- `gitgov/gitgov-server/supabase_schema_v7.sql:69` (tabla `admin_audit_log`)  
- `gitgov/gitgov-server/supabase_schema_v7.sql:85` (función append-only)  
- `gitgov/gitgov-server/supabase_schema_v7.sql:96` (trigger append-only)  
- `gitgov/gitgov-server/src/handlers.rs:840` (`confirm_signal` audit)  
- `gitgov/gitgov-server/src/handlers.rs:1127` (`export_events` audit)  
- `gitgov/gitgov-server/src/handlers.rs:2352` (`policy_override` audit)  
- `gitgov/gitgov-server/src/handlers.rs:2590` (`revoke_api_key` audit)

Nivel de certeza: Alto (leído en esta sesión)

---

## 6) ¿El auditor puede ver quiénes componen “Devs Activos 7d”?

Respuesta: Sí. Existe modal de detalle, recuento por login y etiqueta “aparente test”.

Evidencia en código:  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:118` (modal “Detalle: Devs Activos 7d”)  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:138` (badge “al parecer de test”)  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:163` (badge “aparente test”)  
- `gitgov/src/store/useControlPlaneStore.ts:257` (regex `isLikelySyntheticLogin`)  
- `gitgov/src/store/useControlPlaneStore.ts:528` (`suspicious_test_data` por heurística)

Nivel de certeza: Alto (leído en esta sesión)

---

## 7) ¿Se pueden consolidar múltiples logins del mismo developer?

Respuesta: Sí. Hay aliases de identidad a nivel backend y se aplican en `/logs`.

Evidencia en código:  
- `gitgov/gitgov-server/src/handlers.rs:3210` (`create_identity_alias`)  
- `gitgov/gitgov-server/src/handlers.rs:3278` (`list_identity_aliases`)  
- `gitgov/gitgov-server/src/db.rs:709` (`LEFT JOIN identity_aliases` para github)  
- `gitgov/gitgov-server/src/db.rs:739` (`LEFT JOIN identity_aliases` para client)  
- `gitgov/gitgov-server/src/models.rs:1381` (`expand_login_aliases`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 8) ¿GDPR mínimo (export/erase) está expuesto?

Respuesta: Sí. Existen endpoints para exportar/anonimizar por login y se auditan acciones.

Evidencia en código:  
- `gitgov/gitgov-server/src/main.rs:590` (`POST /users/{login}/erase`)  
- `gitgov/gitgov-server/src/main.rs:591` (`GET /users/{login}/export`)  
- `gitgov/gitgov-server/src/handlers.rs:3029` (`erase_user`)  
- `gitgov/gitgov-server/src/handlers.rs:3106` (`export_user`)  
- `gitgov/gitgov-server/src/handlers.rs:3063` (`gdpr_erase` audit)  
- `gitgov/gitgov-server/src/handlers.rs:3139` (`gdpr_export` audit)

Nivel de certeza: Alto (leído en esta sesión)

---

## 9) ¿La auth del Control Plane sigue siendo Bearer-only?

Respuesta: Sí. El middleware exige header `Authorization` con prefijo `Bearer `.

Evidencia en código:  
- `gitgov/gitgov-server/src/auth.rs:46` (`Missing Authorization header`)  
- `gitgov/gitgov-server/src/auth.rs:49` (`strip_prefix("Bearer ")`)  
- `gitgov/gitgov-server/src/auth.rs:50` (`Invalid Authorization header format`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 10) ¿Estamos sobreprometiendo SSO en pricing?

Respuesta: Sí estaba sobreprometido en copy comercial; ya se corrigió hoy para no vender algo no implementado.

Evidencia en código (fix aplicado):  
- `gitgov-web/lib/i18n/translations.ts:419` (`Starter` ahora “Compliance reports”)  
- `gitgov-web/lib/i18n/translations.ts:432` (`Team` ahora “Compliance reports”)  
- `gitgov-web/lib/i18n/translations.ts:441` (`Enterprise` ahora “Compliance reports (SSO roadmap)”)

Nivel de certeza: Alto (leído y editado en esta sesión)

---

## Nota operativa

No se modificó auth middleware, ingest `/events` ni outbox en esta ronda; el fix fue de copy comercial en web + este artefacto de auditoría técnica.

---

## 11) ¿Qué decisión concreta toma un CTO con GitGov en 5 minutos?

Respuesta: Puede decidir rápidamente el estado de trazabilidad operativa (actividad, salud CI, cobertura de tickets y commits recientes) desde un único dashboard.

Evidencia en código:  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:77` (`<MetricsGrid ...>`)  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:90` (`<PipelineHealthWidget ...>`)  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:98` (`<TicketCoverageWidget />`)  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:110` (`<RecentCommitsTable />`)  
- `gitgov/gitgov-server/src/models.rs:371` (`active_devs_week` en stats)

Nivel de certeza: Alto (leído en esta sesión)

---

## 12) ¿Qué reporte pediría un auditor y lo puedo entregar en 1 click?

Respuesta: Sí, el flujo de exportación de auditoría está implementado en UI y backend (JSON export + historial de exports).

Evidencia en código:  
- `gitgov/src/components/control_plane/ExportPanel.tsx:29` (`handleExport`)  
- `gitgov/src/components/control_plane/ExportPanel.tsx:93` (botón `Exportar JSON`)  
- `gitgov/src/components/control_plane/ExportPanel.tsx:101` (sección `Historial de exports`)  
- `gitgov/gitgov-server/src/main.rs:560` (`POST /export`)  
- `gitgov/gitgov-server/src/main.rs:561` (`GET /exports`)  
- `gitgov/gitgov-server/src/handlers.rs:1066` (`export_events`)  
- `gitgov/gitgov-server/src/handlers.rs:1149` (`list_exports`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 13) ¿Qué pasa cuando hay conflicto entre GitHub, Jira y CI: cuál es la fuente de verdad?

Respuesta: Hoy no hay motor explícito de “resolución de conflictos” entre fuentes; la vista operativa se arma como línea de tiempo combinada + correlaciones.

Evidencia en código:  
- `gitgov/gitgov-server/src/db.rs:658` (`get_combined_events`)  
- `gitgov/gitgov-server/src/db.rs:722` (`UNION ALL` entre fuentes)  
- `gitgov/gitgov-server/src/db.rs:752` (`ORDER BY created_at DESC`)  
- `gitgov/gitgov-server/src/handlers.rs:483` (`correlate_jira_tickets`)  
- `gitgov/gitgov-server/src/handlers.rs:621` (`get_jenkins_commit_correlations`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 14) ¿Cómo pruebo identidad real del actor en entorno corporativo (SSO/IdP)?

Respuesta: No está implementado SSO/SAML/OIDC en el backend actual. La identidad operativa hoy depende de API key + GitHub login/session de la desktop.

Evidencia en código:  
- `gitgov/gitgov-server/src/auth.rs:49` (`Bearer` token auth por API key)  
- `gitgov/src-tauri/src/github/auth.rs:94` (GitHub Device Flow `login/device/code`)  
- `gitgov/src-tauri/src/commands/auth_commands.rs:43` (sesión local en `current_user.json`)  
- `gitgov/src-tauri/src/github/auth.rs:189` (comentario de backup token file además de keyring)

Nivel de certeza: Alto (leído en esta sesión)

---

## 15) ¿Cuál es la “golden metric” comercial hoy con el código actual?

Respuesta: La métrica más defendible hoy es cobertura de trazabilidad por ticket (`commits_with_ticket / total_commits`), porque ya existe cálculo backend y widget en dashboard.

Evidencia en código:  
- `gitgov/gitgov-server/src/models.rs:1117` (`TicketCoverageResponse`)  
- `gitgov/gitgov-server/src/models.rs:1121` (`commits_with_ticket`)  
- `gitgov/gitgov-server/src/db.rs:1414` (cálculo `commits_with_ticket`)  
- `gitgov/gitgov-server/src/db.rs:1416` (cálculo `coverage_pct`)  
- `gitgov/src/components/control_plane/ServerDashboard.tsx:98` (`<TicketCoverageWidget />`)

Nivel de certeza: Alto (leído en esta sesión)

---

## 16) ¿Qué integración desbloquea más ventas este trimestre: Jira, SIEM o SSO?

Respuesta: Jira y Jenkins ya están operativos en API; SSO y SIEM no aparecen como integraciones implementadas en rutas del server.

Evidencia en código:  
- `gitgov/gitgov-server/src/main.rs:526`–`535` (integración Jenkins)  
- `gitgov/gitgov-server/src/main.rs:537`–`548` (integración Jira)  
- `gitgov/gitgov-server/src/main.rs:601` (`/webhooks/github`)  

NO VERIFICADO: priorización comercial exacta (SIEM vs SSO) depende de pipeline de ventas real y no está codificada en el repo.

Nivel de certeza: Alto para estado técnico; Bajo para prioridad comercial

---

## 17) ¿Cuál es el límite de responsabilidad legal que asumimos con nuestras señales?

NO VERIFICADO: El repositorio no muestra términos legales/contractuales que definan ese límite de responsabilidad.

Evidencia en código:  
- `gitgov/gitgov-server/src/handlers.rs:2101` (`advisory: true`)  
- `gitgov/gitgov-server/src/handlers.rs:2173` (warning explícito de modo advisory)  
- `gitgov/gitgov-server/src/handlers.rs:2176` (warning de bypass/drift no totalmente integrado)

Bloqueadores concretos: falta artefacto legal (Términos/SLA/limitación de responsabilidad) versionado en repo o referenciado por el producto.

Nivel de certeza: Alto para estado técnico; Bajo para marco legal

---

## 18) ¿Qué plan de pricing corresponde a valor (no a features sueltas)?

Respuesta: Hoy el pricing en web es estático/comercial (cards y CTA a contacto), sin enforcement técnico de planes en backend.

Evidencia en código:  
- `gitgov-web/components/marketing/PricingClient.tsx:45` (`ctaHref: '/contact'` en Starter)  
- `gitgov-web/components/marketing/PricingClient.tsx:66` (`ctaHref: '/contact'` en Team)  
- `gitgov-web/components/marketing/PricingClient.tsx:86` (`ctaHref: '/contact'` en Enterprise)  
- `gitgov-web/components/marketing/PricingClient.tsx:50`–`52` (features no incluidas en Starter marcadas en UI)  
- `gitgov/gitgov-server/src/main.rs:526`–`595` (rutas de producto, sin módulo de billing/entitlements)

Nivel de certeza: Alto (leído en esta sesión)
