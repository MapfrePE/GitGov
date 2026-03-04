// ============================================================================
// VIOLATION DECISIONS (v3 schema)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct AddDecisionRequest {
    pub decision_type: String,
    pub decided_by: String,
    pub notes: Option<String>,
    pub evidence: Option<serde_json::Value>,
}

pub async fn add_violation_decision(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(violation_id): Path<String>,
    Json(payload): Json<AddDecisionRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    let valid_types = ["acknowledged", "false_positive", "resolved", "escalated", "dismissed", "wont_fix"];
    if !valid_types.contains(&payload.decision_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid decision_type",
                "valid_types": valid_types
            })),
        );
    }

    match state.db.add_violation_decision(
        &violation_id,
        &payload.decision_type,
        &auth_user.client_id,
        payload.notes.as_deref(),
        payload.evidence.clone(),
    ).await {
        Ok(decision_id) => {
            tracing::info!(
                violation_id = %violation_id,
                decision_type = %payload.decision_type,
                decided_by = %auth_user.client_id,
                admin = %auth_user.client_id,
                "Violation decision added"
            );
            (
                StatusCode::OK,
                Json(json!({
                    "decision_id": decision_id,
                    "violation_id": violation_id,
                    "decision_type": payload.decision_type,
                    "decided_by": auth_user.client_id
                })),
            )
        }
        Err(e) => {
            tracing::error!("Failed to add violation decision: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": sanitize_db_error(&e)})))
        }
    }
}

pub async fn get_violation_decisions(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(violation_id): Path<String>,
) -> impl IntoResponse {
    if auth_user.role != UserRole::Admin {
        let scope = match state.db.get_violation_scope(&violation_id).await {
            Ok(Some(scope)) => scope,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"decisions": [], "error": "Violation not found"})),
                );
            }
            Err(e) => {
                tracing::error!("Failed to load violation scope: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"decisions": [], "error": "Internal database error"})),
                );
            }
        };

        let (violation_org_id, violation_user_login) = scope;
        let same_user = violation_user_login
            .as_deref()
            .map(|login| login == auth_user.client_id)
            .unwrap_or(false);
        let same_org = auth_user.org_id.is_some() && auth_user.org_id == violation_org_id;

        if !same_user && !same_org {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"decisions": [], "error": "Insufficient permissions"})),
            );
        }
    }

    match state.db.get_violation_decisions(&violation_id).await {
        Ok(decisions) => (StatusCode::OK, Json(json!({"decisions": decisions}))),
        Err(e) => {
            tracing::error!("Failed to get violation decisions: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"decisions": [], "error": "Internal database error"})))
        }
    }
}

// ============================================================================
// POLICY HISTORY
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyHistoryResponse {
    pub history: Vec<PolicyHistory>,
}

pub async fn get_policy_history(
    State(state): State<Arc<AppState>>,
    Path(repo_name): Path<String>,
) -> impl IntoResponse {
    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(PolicyHistoryResponse { history: vec![] }),
            );
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyHistoryResponse { history: vec![] }),
            );
        }
    };

    match state.db.get_policy_history(&repo.id).await {
        Ok(history) => (StatusCode::OK, Json(PolicyHistoryResponse { history })),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(PolicyHistoryResponse { history: vec![] })),
    }
}

// ============================================================================
// EXPORT ENDPOINT
// ============================================================================

pub async fn export_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExportRequest>,
) -> impl IntoResponse {
    let org_id = if let Some(ref org_name) = payload.org_name {
        state.db.get_org_by_login(org_name).await.ok().flatten().map(|o| o.id)
    } else {
        None
    };

    let mut filter = EventFilter {
        start_date: payload.start_date,
        end_date: payload.end_date,
        org_name: payload.org_name.clone(),
        ..Default::default()
    };
    if auth_user.role != UserRole::Admin {
        filter.user_login = Some(auth_user.client_id.clone());
    }

    let events = match state.db.get_events_for_export(&filter).await {
        Ok(e) => e,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExportResponse {
                    id: String::new(),
                    export_type: payload.export_type.clone(),
                    record_count: 0,
                    content_hash: String::new(),
                    data: None,
                    created_at: 0,
                }),
            );
        }
    };

    let record_count = events.len() as i32;
    let data_json = serde_json::to_string(&events).unwrap_or_default();
    let content_hash = format!("{:x}", Sha256::digest(data_json.as_bytes()));

    let export_log = ExportLog {
        id: Uuid::new_v4().to_string(),
        org_id: if auth_user.role == UserRole::Admin { org_id } else { auth_user.org_id.clone().or(org_id) },
        exported_by: auth_user.client_id.clone(),
        export_type: payload.export_type.clone(),
        date_range_start: payload.start_date,
        date_range_end: payload.end_date,
        filters: payload.filters.unwrap_or(serde_json::Value::Null),
        record_count,
        content_hash: Some(content_hash.clone()),
        file_path: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    let _ = state.db.create_export_log(&export_log).await;
    // Admin audit log — append-only, non-fatal
    let audit_entry = AdminAuditLogEntry {
        id: Uuid::new_v4().to_string(),
        actor_client_id: auth_user.client_id.clone(),
        action: "export_events".to_string(),
        target_type: Some("export_log".to_string()),
        target_id: Some(export_log.id.clone()),
        metadata: serde_json::json!({ "record_count": record_count, "export_type": export_log.export_type }),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
        tracing::warn!("Failed to write admin audit log (export_events): {}", e);
    }

    let response = ExportResponse {
        id: export_log.id,
        export_type: payload.export_type,
        record_count,
        content_hash,
        data: Some(serde_json::to_value(&events).unwrap_or(serde_json::Value::Null)),
        created_at: export_log.created_at,
    };

    (StatusCode::OK, Json(response))
}

pub async fn list_exports(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    match state.db.list_export_logs(auth_user.org_id.as_deref()).await {
        Ok(logs) => (StatusCode::OK, Json(logs)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

// ============================================================================
// GITHUB WEBHOOK HANDLER
