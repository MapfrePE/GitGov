// ============================================================================
// GDPR — T2 (POST /users/:login/erase, GET /users/:login/export)
// ============================================================================

pub async fn erase_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(login): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let login = login.trim().to_string();
    if login.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "login cannot be empty" })),
        )
            .into_response();
    }

    let org_scope = auth_user.org_id.as_deref();
    match state.db.erase_user_data(&login, org_scope).await {
        Ok((client_count, github_count)) => {
            let status = erase_result_status(client_count, github_count);
            if status == StatusCode::NOT_FOUND {
                return (
                    status,
                    Json(serde_json::json!({ "error": "User not found in visible scope" })),
                )
                    .into_response();
            }

            // Append-only audit log entry for the erasure
            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "gdpr_erase".to_string(),
                target_type: Some("user".to_string()),
                target_id: Some(login.clone()),
                metadata: serde_json::json!({
                    "client_events_erased": client_count,
                    "github_events_erased": github_count,
                }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write GDPR audit log entry");
            }

            tracing::info!(
                actor = %auth_user.client_id,
                login = %login,
                client_events = client_count,
                github_events = github_count,
                "GDPR erasure completed"
            );

            (
                status,
                Json(EraseUserResponse {
                    user_login: login,
                    client_events_erased: client_count,
                    github_events_erased: github_count,
                    erased_at: chrono::Utc::now().timestamp_millis(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, login = %login, "GDPR erase failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn export_user(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(login): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let login = login.trim().to_string();
    if login.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "login cannot be empty" })),
        )
            .into_response();
    }

    let org_scope = auth_user.org_id.as_deref();
    match state.db.export_user_data(&login, org_scope).await {
        Ok(events) => {
            let status = export_result_status(events.len());
            if status == StatusCode::NOT_FOUND {
                return (
                    status,
                    Json(serde_json::json!({ "error": "User not found in visible scope" })),
                )
                    .into_response();
            }

            let audit = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "gdpr_export".to_string(),
                target_type: Some("user".to_string()),
                target_id: Some(login.clone()),
                metadata: serde_json::json!({ "event_count": events.len() }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit).await {
                tracing::warn!(error = %e, "Failed to write GDPR export audit log entry");
            }

            let total = events.len();
            (
                status,
                Json(ExportUserResponse {
                    user_login: login,
                    events,
                    total,
                    exported_at: chrono::Utc::now().timestamp_millis(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, login = %login, "GDPR export failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// CLIENT SESSIONS — T3.A (GET /clients)
// ============================================================================

pub async fn get_clients(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let org_id = auth_user.org_id.as_deref();

    match state.db.get_client_sessions(org_id).await {
        Ok(sessions) => {
            let total = sessions.len();
            (
                StatusCode::OK,
                Json(ClientSessionsResponse { sessions, total }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch client sessions");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// IDENTITY ALIASES — T3.B (POST /identities/aliases, GET /identities/aliases)
// ============================================================================

pub async fn create_identity_alias(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateIdentityAliasRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let canonical = payload.canonical.trim().to_string();
    let alias = payload.alias.trim().to_string();

    if canonical.is_empty() || alias.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "canonical and alias cannot be empty" })),
        )
            .into_response();
    }
    if canonical == alias {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "canonical and alias must be different" })),
        )
            .into_response();
    }

    let org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        payload.org_name.as_deref(),
        true,
    )
    .await
    {
        Ok(org_id) => org_id,
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

    match state.db.create_identity_alias(&canonical, &alias, org_id.as_deref()).await {
        Ok(created) => (
            if created { StatusCode::CREATED } else { StatusCode::OK },
            Json(CreateIdentityAliasResponse {
                canonical_login: canonical,
                alias_login: alias,
                created,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create identity alias");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn list_identity_aliases(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let org_id = auth_user.org_id.as_deref();

    match state.db.list_identity_aliases(org_id).await {
        Ok(aliases) => (StatusCode::OK, Json(aliases)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list identity aliases");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// PURE SCOPE HELPERS — unit-testable without a database
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrgScopeError {
    BadRequest,
    NotFound,
    Forbidden,
    Internal,
}

fn org_scope_status(error: OrgScopeError) -> StatusCode {
    match error {
        OrgScopeError::BadRequest => StatusCode::BAD_REQUEST,
        OrgScopeError::NotFound => StatusCode::NOT_FOUND,
        OrgScopeError::Forbidden => StatusCode::FORBIDDEN,
        OrgScopeError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Resolves effective `org_id` and validates API key scope constraints.
///
/// | auth_org_id | org_was_provided | resolved_org_id | Result                      |
/// |-------------|------------------|-----------------|-----------------------------|
/// | None        | false            | —               | Err(BadRequest)             |
/// | *           | true             | None            | Err(NotFound)               |
/// | Some(a)     | true             | Some(b) a ≠ b   | Err(Forbidden)              |
/// | Some(a)     | true             | Some(a)         | Ok(Some(a))                 |
/// | Some(a)     | false            | —               | Ok(Some(a)) implicit scope  |
/// | None        | true             | Some(b)         | Ok(Some(b)) global admin    |
fn check_org_scope_match(
    auth_org_id: Option<&str>,
    org_was_provided: bool,
    resolved_org_id: Option<&str>,
) -> Result<Option<String>, OrgScopeError> {
    if !org_was_provided && auth_org_id.is_none() {
        return Err(OrgScopeError::BadRequest);
    }
    if org_was_provided && resolved_org_id.is_none() {
        return Err(OrgScopeError::NotFound);
    }
    let effective = resolved_org_id.or(auth_org_id);
    if let (Some(scoped), Some(eff)) = (auth_org_id, effective) {
        if scoped != eff {
            return Err(OrgScopeError::Forbidden);
        }
    }
    Ok(effective.map(|s| s.to_string()))
}

async fn resolve_and_check_org_scope(
    state: &Arc<AppState>,
    auth_org_id: Option<&str>,
    requested_org_name: Option<&str>,
    require_org_for_global: bool,
) -> Result<Option<String>, OrgScopeError> {
    let resolved_org_id = match requested_org_name {
        Some(org_name) => match state.db.get_org_by_login(org_name).await {
            Ok(Some(org)) => Some(org.id),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, org_name = %org_name, "Failed to resolve org scope");
                return Err(OrgScopeError::Internal);
            }
        },
        None => None,
    };

    if !require_org_for_global && requested_org_name.is_none() && auth_org_id.is_none() {
        return Ok(None);
    }

    check_org_scope_match(
        auth_org_id,
        requested_org_name.is_some(),
        resolved_org_id.as_deref(),
    )
}

/// Returns the HTTP status for an erase operation given how many rows were
/// updated. When both counts are zero the user was not visible in the org
/// scope, so we return 404 — privacy-preserving (indistinguishable from
/// "user does not exist").
fn erase_result_status(client_count: i64, github_count: i64) -> StatusCode {
    if client_count == 0 && github_count == 0 {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    }
}

/// Returns the HTTP status for an export operation given how many events were
/// found. When zero, the user was not visible in the org scope → 404.
fn export_result_status(event_count: usize) -> StatusCode {
    if event_count == 0 {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::OK
    }
}

