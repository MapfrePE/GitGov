// ============================================================================
// ORG USERS (admin provisioning)
// ============================================================================

pub async fn create_org_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateOrgUserRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let login = payload.login.trim().to_string();
    if login.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "login cannot be empty" })),
        )
            .into_response();
    }

    let role = match parse_user_role_strict(payload.role.as_deref()) {
        Ok(r) => r,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };

    let status = match normalize_org_user_status(payload.status.as_deref()) {
        Ok(s) => s,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        payload.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            )
                .into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    match state
        .db
        .upsert_org_user(&UpsertOrgUserInput {
            org_id: &org_id,
            login: &login,
            display_name: payload.display_name.as_deref(),
            email: payload.email.as_deref(),
            role: role.as_str(),
            status: &status,
            actor: &auth_user.client_id,
        })
        .await
    {
        Ok((user, created)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: if created { "create_org_user" } else { "update_org_user" }.to_string(),
                target_type: Some("org_user".to_string()),
                target_id: Some(user.id.clone()),
                metadata: serde_json::json!({
                    "org_id": user.org_id,
                    "login": user.login,
                    "role": user.role,
                    "status": user.status,
                    "created": created
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (org_user upsert)");
            }

            (
                if created { StatusCode::CREATED } else { StatusCode::OK },
                Json(CreateOrgUserResponse { user, created }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, login = %login, org_id = %org_id, "Failed to upsert org user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn list_org_users(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<OrgUsersQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let status = if let Some(raw) = query.status.as_deref() {
        match normalize_org_user_status(Some(raw)) {
            Ok(s) => Some(s),
            Err(msg) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        query.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(Some(org_id)) => org_id,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "org_name is required for global admin keys" })),
            )
                .into_response();
        }
        Err(err) => {
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required for global admin keys",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (org_scope_status(err), Json(serde_json::json!({ "error": error }))).into_response();
        }
    };

    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_org_users(&org_id, status.as_deref(), limit, offset)
        .await
    {
        Ok((entries, total)) => (StatusCode::OK, Json(OrgUsersResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to list org users");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn update_org_user_status(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_user_id): Path<String>,
    Json(payload): Json<UpdateOrgUserStatusRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let status = match normalize_org_user_status(Some(payload.status.as_str())) {
        Ok(s) => s,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };

    match state
        .db
        .update_org_user_status(
            &org_user_id,
            auth_user.org_id.as_deref(),
            &status,
            &auth_user.client_id,
        )
        .await
    {
        Ok(Some(user)) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "update_org_user_status".to_string(),
                target_type: Some("org_user".to_string()),
                target_id: Some(user.id.clone()),
                metadata: serde_json::json!({
                    "login": user.login,
                    "status": user.status
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (org_user status)");
            }
            (StatusCode::OK, Json(user)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "org_user not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_user_id = %org_user_id, "Failed to update org user status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn create_api_key_for_org_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_user_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let org_user = match state
        .db
        .get_org_user_by_id(&org_user_id, auth_user.org_id.as_deref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: "".to_string(),
                    error: Some("org_user not found".to_string()),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, org_user_id = %org_user_id, "Failed to load org user for API key creation");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: "".to_string(),
                    error: Some("Internal database error".to_string()),
                }),
            )
                .into_response();
        }
    };

    if org_user.status != "active" {
        return (
            StatusCode::CONFLICT,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: org_user.login,
                error: Some("org_user must be active to issue API key".to_string()),
            }),
        )
            .into_response();
    }

    let api_key = Uuid::new_v4().to_string();
    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
    let role = UserRole::from_str(&org_user.role);

    match state
        .db
        .create_api_key(
            &key_hash,
            &org_user.login,
            Some(org_user.org_id.as_str()),
            &role,
        )
        .await
    {
        Ok(()) => {
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "create_api_key_for_org_user".to_string(),
                target_type: Some("org_user".to_string()),
                target_id: Some(org_user_id),
                metadata: serde_json::json!({
                    "client_id": org_user.login,
                    "role": org_user.role,
                    "org_id": org_user.org_id
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write admin audit log (create_api_key_for_org_user)");
            }

            (
                StatusCode::CREATED,
                Json(ApiKeyResponse {
                    api_key: Some(api_key),
                    client_id: org_user.login,
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, org_user_id = %org_user_id, "Failed to create API key for org user");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: org_user.login,
                    error: Some("Internal database error".to_string()),
                }),
            )
                .into_response()
        }
    }
}

// ============================================================================
// API KEYS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyRequest {
    pub client_id: String,
    pub org_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn create_api_key(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ApiKeyRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: payload.client_id,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    let role = match parse_user_role_strict(Some(payload.role.as_str())) {
        Ok(role) => role,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiKeyResponse {
                    api_key: None,
                    client_id: payload.client_id,
                    error: Some(msg.to_string()),
                }),
            );
        }
    };

    // Resolve requested org by login (if provided in payload).
    let requested_org_id = if let Some(ref org_name) = payload.org_name {
        state.db.get_org_by_login(org_name).await
            .ok()
            .flatten()
            .map(|o| o.id)
    } else {
        None
    };

    // If admin key is org-scoped, key creation must stay inside that org.
    let effective_org_id = if let Some(scoped_org_id) = auth_user.org_id.as_deref() {
        match requested_org_id {
            Some(ref requested) if requested == scoped_org_id => Some(requested.clone()),
            Some(_) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ApiKeyResponse {
                        api_key: None,
                        client_id: payload.client_id,
                        error: Some("Requested org_name is outside API key scope".to_string()),
                    }),
                );
            }
            None => Some(scoped_org_id.to_string()),
        }
    } else {
        requested_org_id
    };

    let api_key = Uuid::new_v4().to_string();
    let key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));

    match state
        .db
        .create_api_key(&key_hash, &payload.client_id, effective_org_id.as_deref(), &role)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(ApiKeyResponse {
                api_key: Some(api_key),
                client_id: payload.client_id,
                error: None,
            }),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiKeyResponse {
                api_key: None,
                client_id: payload.client_id,
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

pub async fn get_me(
    Extension(auth_user): Extension<AuthUser>,
) -> impl IntoResponse {
    let response = MeResponse {
        client_id: auth_user.client_id,
        role: auth_user.role.as_str().to_string(),
        org_id: auth_user.org_id,
    };
    (StatusCode::OK, Json(response))
}

pub async fn list_api_keys(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state.db.list_api_keys(auth_user.org_id.as_deref()).await {
        Ok(keys) => (StatusCode::OK, Json(keys)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

pub async fn revoke_api_key(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(key_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state
        .db
        .revoke_api_key(&key_id, auth_user.org_id.as_deref())
        .await
    {
        Ok(true) => {
            // Admin audit log — append-only, non-fatal
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "revoke_api_key".to_string(),
                target_type: Some("api_key".to_string()),
                target_id: Some(key_id.clone()),
                metadata: serde_json::Value::Null,
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (revoke_api_key): {}", e);
            }
            (
                StatusCode::OK,
                Json(RevokeApiKeyResponse {
                    success: true,
                    message: "API key revoked".to_string(),
                }),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(RevokeApiKeyResponse {
                success: false,
                message: "API key not found or already revoked".to_string(),
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RevokeApiKeyResponse {
                success: false,
                message: "Internal database error".to_string(),
            }),
        )
            .into_response(),
    }
}

