// Split orchestrator for handler implementation files.
// Keep this file minimal to improve long-term maintainability.

include!("handlers/prelude_health.rs");
include!("handlers/integrations.rs");
include!("handlers/compliance_signals.rs");
include!("handlers/violations_policy_export.rs");
include!("handlers/github_webhook.rs");
include!("handlers/client_ingest_dashboard.rs");
include!("handlers/policy_admin.rs");
include!("handlers/org_core.rs");
include!("handlers/org_users_api_keys.rs");
include!("handlers/audit_stream_governance.rs");
include!("handlers/jobs_merges_admin_audit.rs");
include!("handlers/gdpr_clients_identities_scope.rs");
include!("handlers/conversational_runtime.rs");
include!("handlers/chat_handler.rs");
include!("handlers/feature_requests.rs");
include!("handlers/sse.rs");
include!("handlers/metrics_endpoint.rs");
include!("handlers/tests.rs");
