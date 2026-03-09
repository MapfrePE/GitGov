// OpenAPI specification for GitGov Control Plane API.
// Uses utoipa to generate the spec and utoipa-swagger-ui to serve it.

use crate::handlers;
use crate::models;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "GitGov Control Plane API",
        version = "1.0.0",
        description = "Centralized audit, governance, and policy server for Git repositories.\n\nAuthentication: most endpoints require `Authorization: Bearer {api_key}`.",
        license(name = "Proprietary")
    ),
    components(schemas(
        // Health
        handlers::HealthResponse,
        handlers::ErrorResponse,
        models::DetailedHealthResponse,
        models::DatabaseHealth,
        // Events
        models::ClientEventBatch,
        models::ClientEventInput,
        models::ClientEventResponse,
        models::EventError,
        // Stats & Logs
        models::AuditStats,
        models::GitHubEventStats,
        models::ClientEventStats,
        models::ViolationStats,
        models::PipelineHealthStats,
        models::DailyActivityPoint,
        models::CombinedEvent,
        // Policy
        models::PolicyCheckRequest,
        models::PolicyCheckResponse,
        models::RuleViolation,
        // Jenkins
        models::JenkinsPipelineEventInput,
        models::JenkinsPipelineEventResponse,
        models::PipelineStage,
        // Auth
        models::MeResponse,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "events", description = "Client event ingestion"),
        (name = "logs", description = "Event logs, stats, and dashboard"),
        (name = "policy", description = "Governance policy checks"),
        (name = "jenkins", description = "Jenkins CI integration"),
        (name = "jira", description = "Jira ticket integration"),
        (name = "orgs", description = "Organization management"),
        (name = "admin", description = "Administration and API keys"),
        (name = "compliance", description = "Compliance signals and violations"),
        (name = "chat", description = "Conversational bot"),
        (name = "metrics", description = "Prometheus metrics"),
    )
)]
pub struct ApiDoc;

/// Modify the generated spec to add Bearer auth security scheme.
pub fn build_openapi_spec() -> utoipa::openapi::OpenApi {
    let mut spec = ApiDoc::openapi();

    // Add Bearer auth security scheme
    let security_scheme = SecurityScheme::Http(
        HttpBuilder::new()
            .scheme(HttpAuthScheme::Bearer)
            .bearer_format("API Key")
            .description(Some(
                "API key passed as Bearer token. Hash is validated server-side.",
            ))
            .build(),
    );

    if let Some(ref mut components) = spec.components {
        components
            .security_schemes
            .insert("bearer_auth".to_string(), security_scheme);
    }

    spec
}
