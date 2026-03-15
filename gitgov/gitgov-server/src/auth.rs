use crate::db::Database;
use crate::models::UserRole;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::Arc;

const FOUNDER_CLIENT_ID: &str = "bootstrap-admin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub client_id: String,
    pub role: UserRole,
    pub org_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthError {
    message: String,
    status: StatusCode,
    code: &'static str,
}

impl AuthError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
        }
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "AUTH_BACKEND_UNAVAILABLE",
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            axum::Json(serde_json::json!({
                "error": self.message,
                "code": self.code
            })),
        )
            .into_response()
    }
}

pub async fn auth_middleware(
    State(db): State<Arc<Database>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<axum::response::Response, AuthError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            metrics::counter!("gitgov_auth_total", "result" => "missing_header", "role" => "unknown").increment(1);
            AuthError::unauthorized("Missing Authorization header")
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        metrics::counter!("gitgov_auth_total", "result" => "bad_format", "role" => "unknown")
            .increment(1);
        AuthError::unauthorized("Invalid Authorization header format")
    })?;

    let key_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));

    let path = req.uri().path().to_string();
    let auth_validation = db.validate_api_key(&key_hash).await.map_err(|e| {
        tracing::error!("Authentication backend error: {}", e);
        AuthError::service_unavailable("Authentication backend unavailable")
    })?;
    let auth_user = auth_validation.auth.ok_or_else(|| {
        metrics::counter!("gitgov_auth_total", "result" => "invalid_key", "role" => "unknown")
            .increment(1);
        AuthError::unauthorized("Invalid or expired API key")
    })?;

    let (client_id, role, org_id) = auth_user;
    if auth_validation.used_stale_cache
        && role == UserRole::Admin
        && is_sensitive_admin_path(path.as_str())
    {
        tracing::warn!(
            path = %path,
            client_id = %client_id,
            "Blocking stale auth cache for sensitive admin endpoint"
        );
        return Err(AuthError::service_unavailable(
            "Authentication temporarily unavailable for this admin endpoint; retry shortly",
        ));
    }

    let user = AuthUser {
        client_id,
        role,
        org_id,
    };

    metrics::counter!("gitgov_auth_total", "result" => "success", "role" => user.role.as_str())
        .increment(1);

    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}

fn is_sensitive_admin_path(path: &str) -> bool {
    path.starts_with("/api-keys")
        || path.starts_with("/dashboard")
        || path.starts_with("/jobs/metrics")
        || path.starts_with("/outbox/lease/metrics")
}

pub fn require_admin(user: &AuthUser) -> Result<(), AuthError> {
    if user.role != UserRole::Admin {
        return Err(AuthError::unauthorized("Admin access required"));
    }
    Ok(())
}

pub fn is_founder_global_admin(user: &AuthUser) -> bool {
    user.role == UserRole::Admin
        && user.org_id.is_none()
        && user.client_id.eq_ignore_ascii_case(FOUNDER_CLIENT_ID)
}

#[cfg(test)]
pub fn require_same_user_or_admin(user: &AuthUser, target_login: &str) -> Result<(), AuthError> {
    if user.role == UserRole::Admin {
        return Ok(());
    }

    if user.client_id != target_login {
        return Err(AuthError::unauthorized("Can only access your own data"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admin_user() -> AuthUser {
        AuthUser {
            client_id: "admin1".to_string(),
            role: UserRole::Admin,
            org_id: None,
        }
    }

    fn dev_user(login: &str) -> AuthUser {
        AuthUser {
            client_id: login.to_string(),
            role: UserRole::Developer,
            org_id: None,
        }
    }

    #[test]
    fn require_admin_allows_admin() {
        assert!(require_admin(&admin_user()).is_ok());
    }

    #[test]
    fn require_admin_blocks_developer() {
        assert!(require_admin(&dev_user("dev1")).is_err());
    }

    #[test]
    fn founder_global_admin_detection_matches_expected_scope() {
        assert!(is_founder_global_admin(&AuthUser {
            client_id: "bootstrap-admin".to_string(),
            role: UserRole::Admin,
            org_id: None,
        }));
        assert!(is_founder_global_admin(&AuthUser {
            client_id: "BOOTSTRAP-ADMIN".to_string(),
            role: UserRole::Admin,
            org_id: None,
        }));
        assert!(!is_founder_global_admin(&AuthUser {
            client_id: "bootstrap-admin".to_string(),
            role: UserRole::Admin,
            org_id: Some("org-123".to_string()),
        }));
        assert!(!is_founder_global_admin(&AuthUser {
            client_id: "admin1".to_string(),
            role: UserRole::Admin,
            org_id: None,
        }));
    }

    #[test]
    fn require_same_user_or_admin_allows_admin_for_any_target() {
        assert!(require_same_user_or_admin(&admin_user(), "anyone").is_ok());
    }

    #[test]
    fn require_same_user_or_admin_allows_self() {
        assert!(require_same_user_or_admin(&dev_user("dev1"), "dev1").is_ok());
    }

    #[test]
    fn require_same_user_or_admin_blocks_different_user() {
        assert!(require_same_user_or_admin(&dev_user("dev1"), "dev2").is_err());
    }

    #[test]
    fn sensitive_admin_path_detection_matches_expected_routes() {
        assert!(is_sensitive_admin_path("/api-keys"));
        assert!(is_sensitive_admin_path("/api-keys/revoke"));
        assert!(is_sensitive_admin_path("/dashboard"));
        assert!(is_sensitive_admin_path("/jobs/metrics"));
        assert!(is_sensitive_admin_path("/outbox/lease/metrics"));
        assert!(!is_sensitive_admin_path("/logs"));
        assert!(!is_sensitive_admin_path("/stats"));
    }

    #[test]
    fn auth_error_service_unavailable_maps_to_503() {
        let response = AuthError::service_unavailable("backend down").into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
