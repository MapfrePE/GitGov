// ============================================================================
// POLICIES
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyApiResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<GitGovConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn get_policy(
    Extension(_auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
) -> impl IntoResponse {
    // First get repo ID by full_name
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Repository not found".to_string()),
                }),
            );
        }
        Err(_e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    match state.db.get_policy(&repo.id).await {
        Ok(Some(policy)) => (
            StatusCode::OK,
            Json(PolicyApiResponse {
                version: Some(policy.version),
                checksum: Some(policy.checksum),
                config: Some(policy.config),
                updated_at: Some(policy.updated_at),
                error: None,
            }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Policy not found".to_string()),
            }),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

pub async fn override_policy(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
    Json(config): Json<GitGovConfig>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    // First get repo ID by full_name
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Repository not found".to_string()),
                }),
            );
        }
        Err(_e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    let config_json = match serde_json::to_string(&config) {
        Ok(json) => json,
        Err(_e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(PolicyApiResponse {
                    version: None,
                    checksum: None,
                    config: None,
                    updated_at: None,
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    let checksum = format!("{:x}", Sha256::digest(config_json.as_bytes()));

    // Record that this is an override
    tracing::warn!(
        "Policy override for {} by {} (is_override=true)",
        repo_name,
        auth_user.client_id
    );

    match state.db.save_policy(&repo.id, &config, &checksum, &auth_user.client_id).await {
        Ok(()) => {
            // Admin audit log — append-only, non-fatal
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "policy_override".to_string(),
                target_type: Some("repo".to_string()),
                target_id: Some(repo.id.clone()),
                metadata: serde_json::json!({
                    "repo_name": repo_name,
                    "checksum": checksum
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (policy_override): {}", e);
            }

            (
                StatusCode::OK,
                Json(PolicyApiResponse {
                    version: Some("1.0".to_string()),
                    checksum: Some(checksum),
                    config: Some(config),
                    updated_at: Some(chrono::Utc::now().timestamp_millis()),
                    error: None,
                }),
            )
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PolicyApiResponse {
                version: None,
                checksum: None,
                config: None,
                updated_at: None,
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

