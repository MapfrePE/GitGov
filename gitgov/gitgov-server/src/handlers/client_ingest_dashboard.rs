// ============================================================================
// CLIENT EVENTS (Batch Ingest)
// ============================================================================

pub async fn ingest_client_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(batch): Json<ClientEventBatch>,
) -> impl IntoResponse {
    let mut events = Vec::new();
    let strict_actor_match = state.strict_actor_match;

    for input in batch.events {
        if strict_actor_match
            && auth_user.role != UserRole::Admin
            && input.user_login != auth_user.client_id
        {
            tracing::warn!(
                auth_user = %auth_user.client_id,
                requested_user_login = %input.user_login,
                event_uuid = %input.event_uuid,
                "Rejecting client event due to strict actor match enforcement"
            );
            return (
                StatusCode::FORBIDDEN,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors: vec![EventError {
                        event_uuid: input.event_uuid,
                        error: "user_login must match authenticated client_id (STRICT_ACTOR_MATCH)".to_string(),
                    }],
                }),
            );
        }

        let effective_user_login = if auth_user.role == UserRole::Admin {
            input.user_login.clone()
        } else {
            auth_user.client_id.clone()
        };

        if state.reject_synthetic_logins && is_likely_synthetic_login(&effective_user_login) {
            tracing::warn!(
                auth_user = %auth_user.client_id,
                rejected_user_login = %effective_user_login,
                event_uuid = %input.event_uuid,
                "Rejecting client event due to synthetic login policy"
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors: vec![EventError {
                        event_uuid: input.event_uuid,
                        error: "synthetic user_login is not allowed in this environment".to_string(),
                    }],
                }),
            );
        }

        // Get org and repo IDs
        let requested_org_id = if let Some(ref org_name) = input.org_name {
            state.db.get_org_by_login(org_name).await
                .ok()
                .flatten()
                .map(|o| o.id)
        } else {
            None
        };

        if auth_user.role != UserRole::Admin {
            if let (Some(scoped_org_id), Some(requested_org_id)) =
                (auth_user.org_id.as_deref(), requested_org_id.as_deref())
            {
                if scoped_org_id != requested_org_id {
                    tracing::warn!(
                        auth_user = %auth_user.client_id,
                        requested_org_id = %requested_org_id,
                        scoped_org_id = %scoped_org_id,
                        event_uuid = %input.event_uuid,
                        "Rejecting client event with org mismatch"
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ClientEventResponse {
                            accepted: vec![],
                            duplicates: vec![],
                            errors: vec![EventError {
                                event_uuid: input.event_uuid,
                                error: "Event org_name is outside API key scope".to_string(),
                            }],
                        }),
                    );
                }
            }
        }

        let org_id = if auth_user.role == UserRole::Admin {
            requested_org_id
        } else {
            auth_user.org_id.clone().or(requested_org_id)
        };

        let inferred_repo_full_name = input
            .repo_full_name
            .clone()
            .or_else(|| {
                input
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("repo_name"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
            });

        let repo = if let Some(ref repo_full_name) = inferred_repo_full_name {
            state
                .db
                .get_repo_by_full_name(repo_full_name)
                .await
                .unwrap_or_default()
        } else {
            None
        };
        if auth_user.role != UserRole::Admin {
            if let (Some(scoped_org_id), Some(repo)) = (auth_user.org_id.as_deref(), repo.as_ref()) {
                if repo.org_id.as_deref() != Some(scoped_org_id) {
                    tracing::warn!(
                        auth_user = %auth_user.client_id,
                        repo = %repo.full_name,
                        event_uuid = %input.event_uuid,
                        "Rejecting client event with repo outside API key scope"
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ClientEventResponse {
                            accepted: vec![],
                            duplicates: vec![],
                            errors: vec![EventError {
                                event_uuid: input.event_uuid,
                                error: "Event repo_full_name is outside API key scope".to_string(),
                            }],
                        }),
                    );
                }
            }
        }
        let repo_id = if let Some(repo) = repo {
            Some(repo.id)
        } else if let (Some(full_name), Some(effective_org_id)) =
            (inferred_repo_full_name.as_deref(), org_id.as_deref())
        {
            let repo_name = full_name
                .split('/')
                .next_back()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(full_name);
            match state
                .db
                .upsert_repo_by_full_name(Some(effective_org_id), full_name, repo_name, true)
                .await
            {
                Ok(id) => Some(id),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        full_name = %full_name,
                        org_id = %effective_org_id,
                        event_uuid = %input.event_uuid,
                        "Failed to upsert repo from client event (non-fatal)"
                    );
                    None
                }
            }
        } else {
            None
        };

        let event = ClientEvent {
            id: Uuid::new_v4().to_string(),
            org_id,
            repo_id,
            event_uuid: input.event_uuid,
            event_type: ClientEventType::from_str(&input.event_type),
            user_login: effective_user_login,
            user_name: input.user_name,
            branch: input.branch,
            commit_sha: input.commit_sha,
            files: input.files,
            status: EventStatus::from_str(&input.status),
            reason: input.reason,
            metadata: input.metadata.unwrap_or(serde_json::Value::Null),
            client_version: batch.client_version.clone(),
            created_at: input.timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        };

        events.push(event);
    }

    match state.db.insert_client_events_batch(&events).await {
        Ok(response) => {
            // Fire-and-forget: update client_sessions last_seen + device metadata
            {
                let client_id = auth_user.client_id.clone();
                let org_id = auth_user.org_id.clone();
                // Extract device metadata from the first event that has it
                let device_meta = events
                    .iter()
                    .find_map(|e| {
                        e.metadata.get("device").cloned()
                    })
                    .unwrap_or(serde_json::json!({}));
                let db = Arc::clone(&state.db);
                tokio::spawn(async move {
                    if let Err(e) = db
                        .upsert_client_session(&client_id, org_id.as_deref(), &device_meta)
                        .await
                    {
                        tracing::debug!(error = %e, "Failed to upsert client session (non-critical)");
                    }
                });
            }

            // Fire-and-forget alert for blocked_push events
            if let Some(ref webhook_url) = state.alert_webhook_url {
                let accepted_event_ids: HashSet<&str> =
                    response.accepted.iter().map(String::as_str).collect();
                for event in &events {
                    if event.event_type == ClientEventType::BlockedPush
                        && accepted_event_ids.contains(event.event_uuid.as_str())
                    {
                        let text = notifications::format_blocked_push_alert(
                            &event.user_login,
                            event.repo_id.as_deref().unwrap_or("unknown"),
                            event.branch.as_deref().unwrap_or("unknown"),
                        );
                        let client = state.http_client.clone();
                        let url = webhook_url.clone();
                        tokio::spawn(async move {
                            notifications::send_alert(&client, &url, text).await;
                        });
                    }
                }
            }
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to insert client events batch: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors: vec![EventError {
                        event_uuid: "batch".to_string(),
                        error: "Internal database error".to_string(),
                    }],
                }),
            )
        }
    }
}

// ============================================================================
// QUERY ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsResponse {
    pub events: Vec<CombinedEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn get_logs(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<EventFilter>,
) -> impl IntoResponse {
    // Non-admins can only see their own events
    let clamped_limit = if filter.limit == 0 { 100 } else { filter.limit.min(500) };
    let mut filter = if auth_user.role != UserRole::Admin {
        EventFilter {
            user_login: Some(auth_user.client_id.clone()),
            limit: clamped_limit,
            ..filter
        }
    } else {
        EventFilter {
            limit: clamped_limit,
            ..filter
        }
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
            let error = match err {
                OrgScopeError::BadRequest => "org_name is required",
                OrgScopeError::NotFound => "Organization not found",
                OrgScopeError::Forbidden => "Requested org is outside API key scope",
                OrgScopeError::Internal => "Internal database error",
            };
            return (
                org_scope_status(err),
                Json(LogsResponse {
                    events: vec![],
                    error: Some(error.to_string()),
                }),
            );
        }
    };
    if scoped_org_id.is_some() {
        // Prefer UUID scope to avoid extra org_name lookups in DB query path.
        filter.org_id = scoped_org_id;
        filter.org_name = None;
    }

    match state.db.get_combined_events(&filter).await {
        Ok(events) => (StatusCode::OK, Json(LogsResponse { events, error: None })),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LogsResponse {
                events: vec![],
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

pub async fn get_stats(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (StatusCode::FORBIDDEN, Json(AuditStats::default()));
    }

    let org_id = auth_user.org_id.as_deref();
    match state.db.get_stats(org_id).await {
        Ok(mut stats) => {
            stats.pipeline = state.db.get_pipeline_health_stats(org_id).await.unwrap_or_default();
            stats.client_events.desktop_pushes_today = match state.db.get_desktop_pushes_today(org_id).await {
                Ok(count) => count,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to compute desktop pushes today for /stats");
                    0
                }
            };
            (StatusCode::OK, Json(stats))
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(AuditStats::default())),
    }
}

pub async fn get_team_overview(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TeamOverviewQuery>,
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

    let days = query.days.unwrap_or(30).clamp(1, 180);
    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

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

    match state
        .db
        .get_team_overview(&org_id, status.as_deref(), days, limit, offset)
        .await
    {
        Ok((entries, total)) => (StatusCode::OK, Json(TeamOverviewResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to load team overview");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn get_team_repos(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TeamOverviewQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let days = query.days.unwrap_or(30).clamp(1, 180);
    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

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

    match state.db.get_team_repos(&org_id, days, limit, offset).await {
        Ok((entries, total)) => (StatusCode::OK, Json(TeamReposResponse { entries, total })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, org_id = %org_id, "Failed to load team repo overview");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal database error" })),
            )
                .into_response()
        }
    }
}

pub async fn get_daily_activity(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<DailyActivityQuery>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (StatusCode::FORBIDDEN, Json(Vec::<DailyActivityPoint>::new()));
    }

    let days = query.days.unwrap_or(14).clamp(1, 90) as i64;
    let org_id = auth_user.org_id.as_deref();

    match state.db.get_daily_activity(org_id, days).await {
        Ok(points) => (StatusCode::OK, Json(points)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Vec::<DailyActivityPoint>::new()),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub stats: AuditStats,
    pub recent_events: Vec<CombinedEvent>,
}

pub async fn get_dashboard(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (StatusCode::FORBIDDEN, Json(DashboardResponse {
            stats: AuditStats::default(),
            recent_events: vec![],
        }));
    }

    let org_id = auth_user.org_id.as_deref();
    let stats = state.db.get_stats(org_id).await.unwrap_or_default();

    let filter = EventFilter {
        limit: 10,
        org_id: auth_user.org_id.clone(),
        ..Default::default()
    };
    let recent = state.db.get_combined_events(&filter).await.unwrap_or_default();

    (StatusCode::OK, Json(DashboardResponse {
        stats,
        recent_events: recent,
    }))
}

fn repo_name_from_policy_check_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("git@") {
        if let Some(idx) = trimmed.find(':') {
            // git@github.com:owner/repo.git
            let candidate = &trimmed[idx + 1..];
            return candidate.trim_end_matches(".git").trim_matches('/').to_string();
        }
        if let Some(pos) = trimmed.find("github.com/") {
            let candidate = &trimmed[(pos + "github.com/".len())..];
            return candidate.trim_end_matches(".git").trim_matches('/').to_string();
        }
    }
    trimmed.trim_end_matches(".git").trim_matches('/').to_string()
}

fn branch_matches_policy(policy: &GitGovConfig, branch: &str) -> bool {
    if policy.branches.protected.iter().any(|b| b == branch) {
        return true;
    }

    for pattern in &policy.branches.patterns {
        if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
            if glob_pattern.matches(branch) {
                return true;
            }
        } else if pattern == branch {
            return true;
        }
    }

    false
}

fn ticket_id_regex() -> &'static Regex {
    static TICKET_ID_RE: OnceLock<Regex> = OnceLock::new();
    TICKET_ID_RE.get_or_init(|| {
        Regex::new(r"\b([A-Z][A-Z0-9]{1,15}-[0-9]{1,9})\b")
            .expect("valid ticket id regex")
    })
}

fn extract_ticket_ids(texts: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for text in texts {
        for captures in ticket_id_regex().captures_iter(text) {
            if let Some(ticket) = captures.get(1) {
                let normalized = ticket.as_str().to_ascii_uppercase();
                if seen.insert(normalized.clone()) {
                    found.push(normalized);
                }
            }
        }
    }

    found
}

pub async fn policy_check(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PolicyCheckRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(PolicyCheckResponse {
                advisory: true,
                allowed: false,
                reasons: vec!["Admin access required".to_string()],
                warnings: vec![],
                evaluated_rules: vec![],
            }),
        );
    }

    if payload.repo.trim().is_empty() || payload.branch.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(PolicyCheckResponse {
                advisory: true,
                allowed: false,
                reasons: vec!["repo and branch are required".to_string()],
                warnings: vec![],
                evaluated_rules: vec![],
            }),
        );
    }

    let repo_name = repo_name_from_policy_check_input(&payload.repo);
    let mut response = PolicyCheckResponse {
        advisory: true,
        allowed: true,
        reasons: vec![],
        warnings: vec![],
        evaluated_rules: vec![
            "repo_exists".to_string(),
            "policy_exists".to_string(),
            "branch_matches_policy".to_string(),
        ],
    };

    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            response.allowed = false;
            response.reasons.push("Repository not found in GitGov".to_string());
            return (StatusCode::OK, Json(response));
        }
        Err(_) => {
            response.allowed = false;
            response.reasons.push("Internal database error".to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let policy = match state.db.get_policy(&repo.id).await {
        Ok(Some(policy)) => policy,
        Ok(None) => {
            response.allowed = false;
            response.reasons.push("No policy configured for repository".to_string());
            return (StatusCode::OK, Json(response));
        }
        Err(_) => {
            response.allowed = false;
            response.reasons.push("Internal database error".to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };

    let branch = payload.branch.trim();
    if !branch_matches_policy(&policy.config, branch) {
        response.allowed = false;
        response
            .reasons
            .push(format!("Branch '{}' does not match configured policy patterns/protected branches", branch));
    }

    response.warnings.push(
        "Advisory mode: author/role validation not enforced yet in V1.2-A".to_string(),
    );
    response.warnings.push(
        "Advisory mode: bypass/drift checks are not fully integrated into policy/check yet".to_string(),
    );
    if payload.commit.as_deref().unwrap_or_default().is_empty() {
        response
            .warnings
            .push("Commit SHA not provided; commit-specific checks skipped".to_string());
    }

    (StatusCode::OK, Json(response))
}

