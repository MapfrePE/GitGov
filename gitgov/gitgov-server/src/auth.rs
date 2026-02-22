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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub client_id: String,
    pub role: UserRole,
    pub org_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthError(pub String);

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": self.0,
                "code": "UNAUTHORIZED"
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
        .ok_or_else(|| AuthError("Missing Authorization header".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AuthError("Invalid Authorization header format".to_string()))?;

    let key_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));

    let auth_user = db
        .validate_api_key(&key_hash)
        .await
        .map_err(|e| AuthError(format!("Database error: {}", e)))?
        .ok_or_else(|| AuthError("Invalid or expired API key".to_string()))?;

    let (client_id, role, org_id) = auth_user;

    let user = AuthUser {
        client_id,
        role,
        org_id,
    };

    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}

pub fn require_admin(user: &AuthUser) -> Result<(), AuthError> {
    if user.role != UserRole::Admin {
        return Err(AuthError("Admin access required".to_string()));
    }
    Ok(())
}

pub fn require_same_user_or_admin(user: &AuthUser, target_login: &str) -> Result<(), AuthError> {
    if user.role == UserRole::Admin {
        return Ok(());
    }
    
    if user.client_id != target_login {
        return Err(AuthError("Can only access your own data".to_string()));
    }
    
    Ok(())
}
