// ============================================================================
// COMPLIANCE DASHBOARD
// ============================================================================

pub async fn get_compliance_dashboard(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_name): Path<String>,
) -> impl IntoResponse {
    if let Err(_e) = require_admin(&auth_user) {
        return (StatusCode::FORBIDDEN, Json(ComplianceDashboard::default()));
    }

    let org = match state.db.get_org_by_login(&org_name).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ComplianceDashboard::default()),
            );
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ComplianceDashboard::default()),
            );
        }
    };

    match state.db.get_compliance_dashboard(&org.id).await {
        Ok(dashboard) => (StatusCode::OK, Json(dashboard)),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ComplianceDashboard::default())),
    }
}

// ============================================================================
// NONCOMPLIANCE SIGNALS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalsResponse {
    pub signals: Vec<NoncomplianceSignal>,
    pub total: i64,
}

pub async fn get_signals(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<SignalFilter>,
) -> impl IntoResponse {
    let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
    let offset = filter.offset as i64;

    // Non-admins can only see signals related to them
    let filter_user = if auth_user.role != UserRole::Admin {
        Some(auth_user.client_id.clone())
    } else {
        filter.user_login.clone()
    };

    let scoped_org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        filter.org_name.as_deref(),
        false,
    )
    .await
    {
        Ok(org_id) => org_id,
        Err(err) => {
            return (
                org_scope_status(err),
                Json(SignalsResponse {
                    signals: vec![],
                    total: 0,
                }),
            );
        }
    };

    let signals_query = NoncomplianceSignalsQuery {
        org_id: scoped_org_id.as_deref(),
        confidence: filter.confidence.as_deref(),
        status: filter.status.as_deref(),
        signal_type: filter.signal_type.as_deref(),
        actor_login: filter_user.as_deref(),
        limit,
        offset,
    };

    match state.db.get_noncompliance_signals(&signals_query).await {
        Ok((signals, total)) => (StatusCode::OK, Json(SignalsResponse { signals, total })),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(SignalsResponse { signals: vec![], total: 0 })),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalFilter {
    pub org_name: Option<String>,
    pub confidence: Option<String>,
    pub status: Option<String>,
    pub signal_type: Option<String>,
    pub user_login: Option<String>,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSignalRequest {
    pub status: String,
    pub notes: Option<String>,
}

pub async fn update_signal(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(signal_id): Path<String>,
    Json(payload): Json<UpdateSignalRequest>,
) -> impl IntoResponse {
    let signal = match state.db.get_signal_by_id(&signal_id).await {
        Ok(Some(signal)) => signal,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Signal not found"})),
            );
        }
        Err(e) => {
            tracing::error!("Failed to load signal {}: {}", signal_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal database error"})),
            );
        }
    };

    if auth_user.role != UserRole::Admin {
        let same_user = signal.actor_login == auth_user.client_id;
        let same_org = auth_user.org_id.is_some() && auth_user.org_id == signal.org_id;
        if !same_user && !same_org {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "Insufficient permissions to update this signal"})),
            );
        }
    }

    match state.db.update_signal_status(
        &signal_id,
        &payload.status,
        &auth_user.client_id,
        payload.notes.as_deref(),
    ).await {
        Ok(()) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(e) => {
            tracing::error!("Failed to update signal: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": sanitize_db_error(&e)})))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfirmSignalRequest {
    pub severity: String,
}

pub async fn confirm_signal(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(signal_id): Path<String>,
    Json(payload): Json<ConfirmSignalRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    match state.db.confirm_signal_as_violation(
        &signal_id,
        &auth_user.client_id,
        &payload.severity,
    ).await {
        Ok(violation_id) => {
            tracing::info!("Signal {} confirmed as violation {} by {}", signal_id, violation_id, auth_user.client_id);
            // Admin audit log — append-only, non-fatal
            let audit_entry = AdminAuditLogEntry {
                id: Uuid::new_v4().to_string(),
                actor_client_id: auth_user.client_id.clone(),
                action: "confirm_signal".to_string(),
                target_type: Some("signal".to_string()),
                target_id: Some(signal_id.clone()),
                metadata: serde_json::json!({ "violation_id": violation_id, "severity": payload.severity }),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Err(e) = state.db.insert_admin_audit_log(&audit_entry).await {
                tracing::warn!("Failed to write admin audit log (confirm_signal): {}", e);
            }
            // Fire-and-forget alert
            if let Some(ref webhook_url) = state.alert_webhook_url {
                let text = notifications::format_signal_confirmed_alert(
                    &payload.severity,
                    &auth_user.client_id,
                    None,
                );
                let client = state.http_client.clone();
                let url = webhook_url.clone();
                tokio::spawn(async move {
                    notifications::send_alert(&client, &url, text).await;
                });
            }
            (StatusCode::OK, Json(json!({
                "success": true,
                "violation_id": violation_id
            })))
        }
        Err(e) => {
            tracing::error!("Failed to confirm signal: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": sanitize_db_error(&e)})))
        }
    }
}

pub async fn trigger_detection(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(org_name): Path<String>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    let org = match state.db.get_org_by_login(&org_name).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Organization not found"})),
            );
        }
        Err(e) => {
            tracing::error!("Failed to get org: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal database error"})),
            );
        }
    };

    match state.db.detect_noncompliance_signals(&org.id).await {
        Ok(count) => (StatusCode::OK, Json(json!({"signals_created": count}))),
        Err(e) => {
            tracing::error!("Failed to detect signals: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal database error"})))
        }
    }
}

