// ============================================================================
// JENKINS INTEGRATION (V1.2-A)
// ============================================================================

pub async fn ingest_jenkins_pipeline_event(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<JenkinsPipelineEventInput>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    if let Some(expected_secret) = state.jenkins_webhook_secret.as_deref() {
        let provided_secret = headers
            .get("x-gitgov-jenkins-secret")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .unwrap_or_default();

        if provided_secret.is_empty() || provided_secret.as_bytes().ct_eq(expected_secret.as_bytes()).unwrap_u8() != 1 {
            tracing::warn!("Rejected Jenkins pipeline event due to missing/invalid secret header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(JenkinsPipelineEventResponse {
                    accepted: false,
                    duplicate: false,
                    pipeline_event_id: None,
                    error: Some("Invalid Jenkins webhook secret".to_string()),
                }),
            );
        }
    }

    if payload.pipeline_id.trim().is_empty() || payload.job_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some("pipeline_id and job_name are required".to_string()),
            }),
        );
    }

    let Some(status) = PipelineStatus::from_str(payload.status.trim()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some("Invalid status. Use: success, failure, aborted, unstable".to_string()),
            }),
        );
    };

    let org_id = if let Some(repo_full_name) = payload.repo_full_name.as_deref() {
        match state.db.get_repo_by_full_name(repo_full_name).await {
            Ok(Some(repo)) => repo.org_id,
            Ok(None) => {
                let guessed_org = repo_full_name.split('/').next().unwrap_or_default();
                if guessed_org.is_empty() {
                    None
                } else {
                    state.db.get_org_by_login(guessed_org).await.ok().flatten().map(|o| o.id)
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let raw_payload = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
    let event = PipelineEvent {
        id: Uuid::new_v4().to_string(),
        org_id,
        pipeline_id: payload.pipeline_id,
        job_name: payload.job_name,
        status,
        commit_sha: payload.commit_sha.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        branch: payload.branch.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        repo_full_name: payload.repo_full_name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        duration_ms: payload.duration_ms,
        triggered_by: payload.triggered_by.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        stages: payload.stages,
        artifacts: payload.artifacts,
        payload: raw_payload,
        ingested_at: payload
            .timestamp
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
    };

    tracing::info!(
        pipeline_id = %event.pipeline_id,
        job_name = %event.job_name,
        status = %event.status.as_str(),
        commit_sha = ?event.commit_sha,
        repo = ?event.repo_full_name,
        "Received Jenkins pipeline event"
    );

    match state.db.insert_pipeline_event(&event).await {
        Ok(pipeline_event_id) => (
            StatusCode::OK,
            Json(JenkinsPipelineEventResponse {
                accepted: true,
                duplicate: false,
                pipeline_event_id: Some(pipeline_event_id),
                error: None,
            }),
        ),
        Err(DbError::Duplicate(_)) => (
            StatusCode::OK,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: true,
                pipeline_event_id: None,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JenkinsPipelineEventResponse {
                accepted: false,
                duplicate: false,
                pipeline_event_id: None,
                error: Some(sanitize_db_error(&e)),
            }),
        ),
    }
}

// ============================================================================
// JIRA INTEGRATION (V1.2-B groundwork)
// ============================================================================

fn jira_issue_text(value: Option<&serde_json::Value>) -> Option<String> {
    value?.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn jira_issue_timestamp_ms(value: Option<&serde_json::Value>) -> Option<i64> {
    let raw = value?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn build_project_ticket_from_jira_payload(
    org_id: Option<String>,
    payload: &JiraWebhookEvent,
) -> Result<ProjectTicket, String> {
    let issue = payload.issue.as_ref().ok_or_else(|| "Missing issue object".to_string())?;
    let key = issue
        .get("key")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing issue.key".to_string())?
        .to_ascii_uppercase();

    let fields = issue.get("fields");
    let title = jira_issue_text(fields.and_then(|f| f.get("summary")));
    let status = jira_issue_text(fields.and_then(|f| f.get("status")).and_then(|s| s.get("name")));
    let assignee = jira_issue_text(fields.and_then(|f| f.get("assignee")).and_then(|a| a.get("displayName")))
        .or_else(|| jira_issue_text(fields.and_then(|f| f.get("assignee")).and_then(|a| a.get("name"))));
    let reporter = jira_issue_text(fields.and_then(|f| f.get("reporter")).and_then(|a| a.get("displayName")))
        .or_else(|| jira_issue_text(fields.and_then(|f| f.get("reporter")).and_then(|a| a.get("name"))));
    let priority = jira_issue_text(fields.and_then(|f| f.get("priority")).and_then(|p| p.get("name")));
    let ticket_type = jira_issue_text(fields.and_then(|f| f.get("issuetype")).and_then(|t| t.get("name")));
    let created_at = jira_issue_timestamp_ms(fields.and_then(|f| f.get("created")));
    let updated_at = jira_issue_timestamp_ms(fields.and_then(|f| f.get("updated")));

    let self_url = issue
        .get("self")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut text_candidates: Vec<&str> = Vec::new();
    if let Some(summary) = title.as_deref() {
        text_candidates.push(summary);
    }
    if let Some(description) = fields
        .and_then(|f| f.get("description"))
        .and_then(|d| d.as_str())
    {
        text_candidates.push(description);
    }
    let related_branches = extract_ticket_ids(&text_candidates);

    Ok(ProjectTicket {
        id: Uuid::new_v4().to_string(),
        org_id,
        ticket_id: key,
        ticket_url: self_url,
        title,
        status,
        assignee,
        reporter,
        priority,
        ticket_type,
        related_commits: vec![],
        related_prs: vec![],
        related_branches,
        created_at,
        updated_at,
        ingested_at: chrono::Utc::now().timestamp_millis(),
    })
}

pub async fn ingest_jira_webhook(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<JiraWebhookEvent>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraWebhookIngestResponse {
                accepted: false,
                duplicate: false,
                ticket_id: None,
                error: Some("Admin access required".to_string()),
            }),
        );
    }

    if let Some(expected_secret) = state.jira_webhook_secret.as_deref() {
        let provided_secret = headers
            .get("x-gitgov-jira-secret")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .unwrap_or_default();
        if provided_secret.is_empty() || provided_secret.as_bytes().ct_eq(expected_secret.as_bytes()).unwrap_u8() != 1 {
            return (
                StatusCode::UNAUTHORIZED,
                Json(JiraWebhookIngestResponse {
                    accepted: false,
                    duplicate: false,
                    ticket_id: None,
                    error: Some("Invalid Jira secret".to_string()),
                }),
            );
        }
    }

    let org_id = None;
    let ticket = match build_project_ticket_from_jira_payload(org_id, &payload) {
        Ok(ticket) => ticket,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(JiraWebhookIngestResponse {
                    accepted: false,
                    duplicate: false,
                    ticket_id: None,
                    error: Some(error),
                }),
            )
        }
    };

    let ticket_id = ticket.ticket_id.clone();
    match state.db.upsert_project_ticket(&ticket).await {
        Ok(()) => (
            StatusCode::OK,
            Json(JiraWebhookIngestResponse {
                accepted: true,
                duplicate: false,
                ticket_id: Some(ticket_id),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JiraWebhookIngestResponse {
                accepted: false,
                duplicate: false,
                ticket_id: Some(ticket_id),
                error: Some(sanitize_db_error(&e)),
            }),
        ),
    }
}

pub async fn get_jira_integration_status(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraIntegrationStatusResponse::default()),
        );
    }

    match state.db.get_jira_integration_status().await {
        Ok(status) => (StatusCode::OK, Json(status)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JiraIntegrationStatusResponse::default()),
        ),
    }
}

pub async fn get_jira_ticket_detail(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(ticket_id): Path<String>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraTicketDetailResponse::default()),
        );
    }

    let normalized = ticket_id.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(JiraTicketDetailResponse::default()),
        );
    }

    match state.db.get_project_ticket_by_ticket_id(&normalized).await {
        Ok(Some(ticket)) => (
            StatusCode::OK,
            Json(JiraTicketDetailResponse { found: true, ticket: Some(ticket) }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(JiraTicketDetailResponse { found: false, ticket: None }),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JiraTicketDetailResponse::default()),
        ),
    }
}

pub async fn get_jenkins_integration_status(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JenkinsIntegrationStatusResponse {
                ok: false,
                ..Default::default()
            }),
        );
    }

    match state.db.get_jenkins_integration_status().await {
        Ok(status) => (StatusCode::OK, Json(status)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JenkinsIntegrationStatusResponse {
                ok: false,
                ..Default::default()
            }),
        ),
    }
}

fn read_metadata_commit_message(metadata: &serde_json::Value) -> Option<&str> {
    metadata
        .as_object()
        .and_then(|m| m.get("commit_message"))
        .and_then(|v| v.as_str())
}

pub async fn correlate_jira_tickets(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<JiraCorrelateRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JiraCorrelateResponse::default()),
        );
    }

    let hours = payload.hours.unwrap_or(24).clamp(1, 24 * 30);
    let limit = payload.limit.unwrap_or(500).clamp(1, 5000);

    let commits = match state
        .db
        .get_recent_commit_events_for_ticket_correlation(
            payload.org_name.as_deref(),
            payload.repo_full_name.as_deref(),
            hours,
            limit,
        )
        .await
    {
        Ok(commits) => commits,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(JiraCorrelateResponse::default()),
            )
        }
    };

    let mut created = 0i64;
    let mut correlated_tickets: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (commit_sha, branch, org_id, metadata, _repo_name) in &commits {
        let mut commit_sources: Vec<(&str, Vec<String>)> = Vec::new();
        if let Some(msg) = read_metadata_commit_message(metadata) {
            let tickets = extract_ticket_ids(&[msg]);
            if !tickets.is_empty() {
                commit_sources.push(("commit_message", tickets));
            }
        }
        if let Some(branch_name) = branch.as_deref() {
            let tickets = extract_ticket_ids(&[branch_name]);
            if !tickets.is_empty() {
                commit_sources.push(("branch_name", tickets));
            }
        }

        for (source, tickets) in commit_sources {
            for ticket_id in tickets {
                let correlation = CommitTicketCorrelation {
                    id: Uuid::new_v4().to_string(),
                    org_id: org_id.clone(),
                    commit_sha: commit_sha.clone(),
                    ticket_id: ticket_id.clone(),
                    correlation_source: source.to_string(),
                    confidence: if source == "commit_message" { 1.0 } else { 0.8 },
                    created_at: chrono::Utc::now().timestamp_millis(),
                };
                match state.db.insert_commit_ticket_correlation(&correlation).await {
                    Ok(true) => {
                        created += 1;
                        correlated_tickets.insert(ticket_id);
                        if let Err(e) = state
                            .db
                            .append_project_ticket_relations(
                                &correlation.ticket_id,
                                Some(&correlation.commit_sha),
                                branch.as_deref(),
                            )
                            .await
                        {
                            tracing::warn!(
                                ticket_id = %correlation.ticket_id,
                                commit_sha = %correlation.commit_sha,
                                error = %e,
                                "Failed to append Jira ticket relations after correlation"
                            );
                        }
                    }
                    Ok(false) => {}
                    Err(_) => {}
                }
            }
        }
    }

    let mut correlated_tickets: Vec<String> = correlated_tickets.into_iter().collect();
    correlated_tickets.sort();

    (
        StatusCode::OK,
        Json(JiraCorrelateResponse {
            scanned_commits: commits.len() as i64,
            correlations_created: created,
            correlated_tickets,
        }),
    )
}

pub async fn get_jira_ticket_coverage(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TicketCoverageQuery>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(TicketCoverageResponse::default()),
        );
    }

    let hours = query.hours.unwrap_or(24).clamp(1, 24 * 30);
    match state
        .db
        .get_ticket_coverage(
            query.org_name.as_deref(),
            query.repo_full_name.as_deref(),
            query.branch.as_deref(),
            hours,
        )
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err(e) => {
            tracing::error!(error = %e, "Failed to compute Jira ticket coverage");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TicketCoverageResponse::default()),
            )
        }
    }
}

pub async fn get_jenkins_commit_correlations(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<JenkinsCorrelationFilter>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(JenkinsCorrelationsResponse::default()),
        );
    }

    let filter = JenkinsCorrelationFilter {
        limit: if filter.limit == 0 { 20 } else { filter.limit },
        ..filter
    };

    match state.db.get_commit_pipeline_correlations(&filter).await {
        Ok(correlations) => (
            StatusCode::OK,
            Json(JenkinsCorrelationsResponse { correlations }),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(JenkinsCorrelationsResponse::default()),
        ),
    }
}

