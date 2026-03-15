// ============================================================================
// POLICY CHANGE REQUESTS — request/approve/reject workflow
// ============================================================================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PolicyChangeRequestQuery {
    #[serde(default)]
    pub status: Option<String>, // pending | approved | rejected
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub include_config: Option<bool>,
}

fn normalize_policy_request_status(value: Option<&str>) -> Result<Option<String>, StatusCode> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(None);
    }
    if matches!(normalized.as_str(), "pending" | "approved" | "rejected") {
        Ok(Some(normalized))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

pub async fn create_policy_change_request(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
    Json(payload): Json<PolicyChangeRequestInput>,
) -> impl IntoResponse {
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyChangeRequestCreateResponse {
                    accepted: false,
                    request_id: None,
                    status: "error".to_string(),
                    error: Some("Repository not found".to_string()),
                }),
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to resolve repository for policy request");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyChangeRequestCreateResponse {
                    accepted: false,
                    request_id: None,
                    status: "error".to_string(),
                    error: Some("Internal database error".to_string()),
                }),
            );
        }
    };

    let config_json = match serde_json::to_string(&payload.config) {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to serialize policy change request config");
            return (
                StatusCode::BAD_REQUEST,
                Json(PolicyChangeRequestCreateResponse {
                    accepted: false,
                    request_id: None,
                    status: "error".to_string(),
                    error: Some("Invalid policy config payload".to_string()),
                }),
            );
        }
    };
    let checksum = format!("{:x}", Sha256::digest(config_json.as_bytes()));
    let request_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    match state
        .db
        .create_policy_change_request(CreatePolicyChangeRequestInput {
            request_id: &request_id,
            org_id: repo.org_id.as_deref(),
            repo_id: &repo.id,
            repo_name: &repo_name,
            requested_by: &auth_user.client_id,
            requested_config: &payload.config,
            requested_checksum: &checksum,
            reason: payload.reason.as_deref(),
            created_at: now,
        })
        .await
    {
        Ok(()) => {
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "policy_change_request_created".to_string(),
                target_type: Some("repo".to_string()),
                target_id: Some(repo.id.clone()),
                metadata: serde_json::json!({
                    "repo_name": repo_name,
                    "request_id": request_id,
                    "requested_checksum": checksum
                }),
                created_at: now,
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (policy_change_request_created): {}", e);
            }

            (
                StatusCode::OK,
                Json(PolicyChangeRequestCreateResponse {
                    accepted: true,
                    request_id: Some(request_id),
                    status: "pending".to_string(),
                    error: None,
                }),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create policy change request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyChangeRequestCreateResponse {
                    accepted: false,
                    request_id: None,
                    status: "error".to_string(),
                    error: Some("Failed to create policy change request".to_string()),
                }),
            )
        }
    }
}

pub async fn list_policy_change_requests(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
    Query(query): Query<PolicyChangeRequestQuery>,
) -> impl IntoResponse {
    let status = match normalize_policy_request_status(query.status.as_deref()) {
        Ok(value) => value,
        Err(code) => {
            return (
                code,
                Json(serde_json::json!({
                    "error": "Invalid status filter",
                    "valid_values": ["pending", "approved", "rejected"]
                })),
            )
                .into_response();
        }
    };
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let include_config = query.include_config.unwrap_or(true);

    let requested_by = if auth_user.role == UserRole::Admin {
        None
    } else {
        Some(auth_user.client_id.as_str())
    };

    match state
        .db
        .list_policy_change_requests(ListPolicyChangeRequestsInput {
            org_id: auth_user.org_id.as_deref(),
            repo_name: Some(repo_name.as_str()),
            requested_by,
            status: status.as_deref(),
            limit,
            offset,
            include_config,
        })
        .await
    {
        Ok((requests, total)) => (
            StatusCode::OK,
            Json(PolicyChangeRequestListResponse {
                requests,
                total,
                limit,
                offset,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list policy change requests");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyChangeRequestListResponse {
                    requests: vec![],
                    total: 0,
                    limit,
                    offset,
                }),
            )
                .into_response()
        }
    }
}

pub async fn approve_policy_change_request(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<String>,
    Json(payload): Json<PolicyChangeRequestDecisionInput>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth_user) {
        return e.into_response();
    }

    let existing = match state
        .db
        .get_policy_change_request_by_id(&request_id, auth_user.org_id.as_deref())
        .await
    {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Policy change request not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::warn!(error = %e, request_id = %request_id, "Failed to load policy request");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal database error"})),
            )
                .into_response();
        }
    };

    if existing.status != "pending" {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "Policy change request already decided",
                "status": existing.status
            })),
        )
            .into_response();
    }

    if existing.requested_by == auth_user.client_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Self-approval is not allowed for policy changes"
            })),
        )
            .into_response();
    }

    let decided_at = chrono::Utc::now().timestamp_millis();
    match state
        .db
        .approve_policy_change_request(
            &request_id,
            auth_user.org_id.as_deref(),
            &auth_user.client_id,
            payload.note.as_deref(),
            decided_at,
        )
        .await
    {
        Ok(record) => {
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "policy_change_request_approved".to_string(),
                target_type: Some("policy_change_request".to_string()),
                target_id: Some(request_id.clone()),
                metadata: serde_json::json!({
                    "repo_name": record.repo_name,
                    "requested_by": record.requested_by,
                    "requested_checksum": record.requested_checksum
                }),
                created_at: decided_at,
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (policy_change_request_approved): {}", e);
            }

            (StatusCode::OK, Json(record)).into_response()
        }
        Err(DbError::Duplicate(_)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Policy change request already decided"})),
        )
            .into_response(),
        Err(DbError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Policy change request not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, request_id = %request_id, "Failed to approve policy request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to approve policy change request"})),
            )
                .into_response()
        }
    }
}

pub async fn reject_policy_change_request(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<String>,
    Json(payload): Json<PolicyChangeRequestDecisionInput>,
) -> impl IntoResponse {
    if let Err(e) = require_admin(&auth_user) {
        return e.into_response();
    }

    let existing = match state
        .db
        .get_policy_change_request_by_id(&request_id, auth_user.org_id.as_deref())
        .await
    {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Policy change request not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::warn!(error = %e, request_id = %request_id, "Failed to load policy request");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal database error"})),
            )
                .into_response();
        }
    };

    if existing.status != "pending" {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "Policy change request already decided",
                "status": existing.status
            })),
        )
            .into_response();
    }

    if existing.requested_by == auth_user.client_id {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Self-decision is not allowed for policy changes"
            })),
        )
            .into_response();
    }

    let decided_at = chrono::Utc::now().timestamp_millis();
    match state
        .db
        .reject_policy_change_request(
            &request_id,
            auth_user.org_id.as_deref(),
            &auth_user.client_id,
            payload.note.as_deref(),
            decided_at,
        )
        .await
    {
        Ok(record) => {
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "policy_change_request_rejected".to_string(),
                target_type: Some("policy_change_request".to_string()),
                target_id: Some(request_id.clone()),
                metadata: serde_json::json!({
                    "repo_name": record.repo_name,
                    "requested_by": record.requested_by,
                    "requested_checksum": record.requested_checksum
                }),
                created_at: decided_at,
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (policy_change_request_rejected): {}", e);
            }

            (StatusCode::OK, Json(record)).into_response()
        }
        Err(DbError::Duplicate(_)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Policy change request already decided"})),
        )
            .into_response(),
        Err(DbError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Policy change request not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, request_id = %request_id, "Failed to reject policy request");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to reject policy change request"})),
            )
                .into_response()
        }
    }
}
