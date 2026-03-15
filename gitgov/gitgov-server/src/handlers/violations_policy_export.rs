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
) -> axum::response::Response {
    let normalized_export_type = payload.export_type.trim().to_ascii_lowercase();
    let export_as_csv = matches!(normalized_export_type.as_str(), "csv" | "events_csv");
    let export_as_json = matches!(normalized_export_type.as_str(), "events" | "events_json");
    if !export_as_csv && !export_as_json {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Unsupported export_type",
                "valid_export_types": ["events", "events_json", "events_csv", "csv"]
            })),
        )
            .into_response();
    }

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
    if auth_user.role == UserRole::Admin {
        filter.org_id = org_id.clone();
    } else {
        filter.user_login = Some(auth_user.client_id.clone());
        filter.org_id = auth_user.org_id.clone().or(org_id.clone());
    }

    let events = match state.db.get_events_for_export(&filter).await {
        Ok(e) => e,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExportResponse {
                    id: String::new(),
                    export_type: normalized_export_type.clone(),
                    record_count: 0,
                    content_hash: String::new(),
                    data: None,
                    created_at: 0,
                }),
            )
                .into_response();
        }
    };

    let drift_events = match state.db.get_policy_drift_events_for_export(&filter).await {
        Ok(items) => items,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExportResponse {
                    id: String::new(),
                    export_type: normalized_export_type.clone(),
                    record_count: 0,
                    content_hash: String::new(),
                    data: None,
                    created_at: 0,
                }),
            )
                .into_response();
        }
    };

    let policy_change_requests = match state
        .db
        .get_policy_change_requests_for_export(&filter)
        .await
    {
        Ok(items) => items,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExportResponse {
                    id: String::new(),
                    export_type: normalized_export_type.clone(),
                    record_count: 0,
                    content_hash: String::new(),
                    data: None,
                    created_at: 0,
                }),
            )
                .into_response();
        }
    };

    let record_count = (events.len() + drift_events.len() + policy_change_requests.len()) as i32;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let export_data = if export_as_csv {
        serde_json::Value::String(build_compliance_export_csv(
            &events,
            &drift_events,
            &policy_change_requests,
        ))
    } else {
        serde_json::to_value(serde_json::json!({
            "export_type": normalized_export_type,
            "generated_at": now_ms,
            "summary": {
                "combined_events": events.len(),
                "policy_drift_events": drift_events.len(),
                "policy_change_requests": policy_change_requests.len(),
                "total_records": record_count
            },
            "events": events,
            "policy_drift_events": drift_events,
            "policy_change_requests": policy_change_requests,
        }))
        .unwrap_or(serde_json::Value::Null)
    };
    let content_basis = match &export_data {
        serde_json::Value::String(csv_content) => csv_content.clone(),
        _ => serde_json::to_string(&export_data).unwrap_or_default(),
    };
    let content_hash = format!("{:x}", Sha256::digest(content_basis.as_bytes()));

    let export_log = ExportLog {
        id: Uuid::new_v4().to_string(),
        org_id: if auth_user.role == UserRole::Admin { org_id } else { auth_user.org_id.clone().or(org_id) },
        exported_by: auth_user.client_id.clone(),
        export_type: normalized_export_type.clone(),
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
        export_type: normalized_export_type,
        record_count,
        content_hash,
        data: Some(export_data),
        created_at: export_log.created_at,
    };

    (StatusCode::OK, Json(response)).into_response()
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn build_compliance_export_csv(
    events: &[CombinedEvent],
    drift_events: &[PolicyDriftEventRecord],
    policy_change_requests: &[PolicyChangeRequestRecord],
) -> String {
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(
        events.len() + drift_events.len() + policy_change_requests.len() + 1,
    );
    rows.push(vec![
        "record_kind".to_string(),
        "id".to_string(),
        "source".to_string(),
        "event_type".to_string(),
        "action".to_string(),
        "result".to_string(),
        "created_at_ms".to_string(),
        "user_login".to_string(),
        "repo_name".to_string(),
        "branch".to_string(),
        "status".to_string(),
        "before_checksum".to_string(),
        "after_checksum".to_string(),
        "duration_ms".to_string(),
        "details_json".to_string(),
        "metadata_json".to_string(),
    ]);

    for event in events {
        rows.push(vec![
            "event".to_string(),
            event.id.clone(),
            event.source.clone(),
            event.event_type.clone(),
            String::new(),
            String::new(),
            event.created_at.to_string(),
            event.user_login.clone().unwrap_or_default(),
            event.repo_name.clone().unwrap_or_default(),
            event.branch.clone().unwrap_or_default(),
            event.status.clone().unwrap_or_default(),
            String::new(),
            String::new(),
            String::new(),
            serde_json::to_string(&event.details).unwrap_or_default(),
            String::new(),
        ]);
    }

    for drift in drift_events {
        rows.push(vec![
            "policy_drift".to_string(),
            drift.id.clone(),
            "policy_drift".to_string(),
            "policy_drift_event".to_string(),
            drift.action.clone(),
            drift.result.clone(),
            drift.created_at.to_string(),
            drift.user_login.clone(),
            drift.repo_name.clone(),
            String::new(),
            String::new(),
            drift.before_checksum.clone().unwrap_or_default(),
            drift.after_checksum.clone().unwrap_or_default(),
            drift.duration_ms.map(|v| v.to_string()).unwrap_or_default(),
            String::new(),
            serde_json::to_string(&drift.metadata).unwrap_or_default(),
        ]);
    }

    for request in policy_change_requests {
        rows.push(vec![
            "policy_change_request".to_string(),
            request.id.clone(),
            "policy".to_string(),
            "policy_change_request".to_string(),
            "request".to_string(),
            request.status.clone(),
            request.created_at.to_string(),
            request.requested_by.clone(),
            request.repo_name.clone(),
            String::new(),
            request.status.clone(),
            String::new(),
            request.requested_checksum.clone(),
            String::new(),
            serde_json::to_string(&request.requested_config).unwrap_or_default(),
            serde_json::to_string(&serde_json::json!({
                "reason": request.reason,
                "decided_by": request.decided_by,
                "decision_note": request.decision_note,
                "decided_at": request.decided_at,
            }))
            .unwrap_or_default(),
        ]);
    }

    let mut out = String::new();
    for row in rows {
        let escaped = row.iter().map(|value| csv_escape(value)).collect::<Vec<_>>();
        out.push_str(&escaped.join(","));
        out.push('\n');
    }
    out
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
