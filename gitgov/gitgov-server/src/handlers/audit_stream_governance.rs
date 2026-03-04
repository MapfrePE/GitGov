// ============================================================================
// AUDIT STREAM (GitHub Audit Log Ingestion)
// ============================================================================

pub async fn ingest_audit_stream(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(batch): Json<AuditStreamBatch>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(AuditStreamResponse {
                accepted: 0,
                filtered: 0,
                errors: vec!["Admin access required".to_string()],
            }),
        );
    }

    let org = if let Some(ref org_name) = batch.org_name {
        match state.db.get_org_by_login(org_name).await {
            Ok(Some(o)) => Some(o),
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuditStreamResponse {
                        accepted: 0,
                        filtered: 0,
                        errors: vec![format!("Organization '{}' not found", org_name)],
                    }),
                );
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(AuditStreamResponse {
                        accepted: 0,
                        filtered: 0,
                        errors: vec![e.to_string()],
                    }),
                );
            }
        }
    } else {
        None
    };

    let mut governance_events = Vec::new();
    let mut filtered = 0;

    for entry in batch.entries {
        if !is_relevant_audit_action(&entry.action) {
            tracing::debug!("Filtered non-relevant audit action: {}", entry.action);
            filtered += 1;
            continue;
        }

        let event_org_id = org.as_ref().map(|o| o.id.clone());
        let event_repo_id = get_repo_id_for_audit_entry(&state, &entry, event_org_id.as_deref()).await;

        let delivery_id = make_audit_delivery_id(&entry, event_org_id.as_deref());

        let (target, old_value, new_value) = extract_audit_changes(&entry);

        let event = GovernanceEvent {
            id: Uuid::new_v4().to_string(),
            org_id: event_org_id,
            repo_id: event_repo_id,
            delivery_id,
            event_type: entry.action.clone(),
            actor_login: entry.actor.clone(),
            target,
            old_value,
            new_value,
            payload: serde_json::to_value(&entry).unwrap_or(serde_json::Value::Null),
            created_at: entry.timestamp,
        };

        governance_events.push(event);
    }

    let (accepted, errors) = match state.db.insert_governance_events_batch(&governance_events).await {
        Ok(result) => result,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuditStreamResponse {
                    accepted: 0,
                    filtered,
                    errors: vec![e.to_string()],
                }),
            );
        }
    };

    tracing::info!(
        "Ingested {} governance events, filtered {} (org={})",
        accepted,
        filtered,
        batch.org_name.as_deref().unwrap_or("unknown")
    );

    (StatusCode::OK, Json(AuditStreamResponse { accepted, filtered, errors }))
}

pub async fn get_governance_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(filter): Query<GovernanceEventFilter>,
) -> impl IntoResponse {
    let limit = if filter.limit == 0 { 100 } else { filter.limit } as i64;
    let offset = filter.offset as i64;

    let requested_org_id = if let Some(ref org_name) = filter.org_name {
        state.db.get_org_by_login(org_name).await.ok().flatten().map(|o| o.id)
    } else {
        None
    };

    let org_id = if auth_user.role == UserRole::Admin {
        requested_org_id
    } else {
        if let (Some(scoped_org_id), Some(requested_org_id)) =
            (auth_user.org_id.as_deref(), requested_org_id.as_deref())
        {
            if scoped_org_id != requested_org_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(GovernanceEventsResponse { events: vec![] }),
                );
            }
        }
        auth_user.org_id.clone().or(requested_org_id)
    };

    match state.db.get_governance_events(org_id.as_deref(), filter.event_type.as_deref(), limit, offset).await {
        Ok(events) => (StatusCode::OK, Json(GovernanceEventsResponse { events })),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GovernanceEventsResponse { events: vec![] }),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceEventFilter {
    pub org_name: Option<String>,
    pub event_type: Option<String>,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceEventsResponse {
    pub events: Vec<GovernanceEvent>,
}

fn is_relevant_audit_action(action: &str) -> bool {
    RELEVANT_AUDIT_ACTIONS.contains(&action)
}

fn make_audit_delivery_id(entry: &GitHubAuditLogEntry, org_id: Option<&str>) -> String {
    let digest_input = serde_json::json!({
        "org_id": org_id,
        "timestamp": entry.timestamp,
        "action": entry.action,
        "actor": entry.actor,
        "repo": entry.repo,
        "repository": entry.repository,
        "repository_id": entry.repository_id,
        "team": entry.team,
        "user": entry.user,
        "data": entry.data,
    });

    let bytes = serde_json::to_vec(&digest_input).unwrap_or_default();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    format!("audit-{}-{}", entry.timestamp, hash)
}

async fn get_repo_id_for_audit_entry(state: &Arc<AppState>, entry: &GitHubAuditLogEntry, _org_id: Option<&str>) -> Option<String> {
    let repo_name = entry.repo.as_ref()
        .or(entry.repository.as_ref())?;

    state.db.get_repo_by_full_name(repo_name).await.ok().flatten().map(|r| r.id)
}

fn extract_audit_changes(entry: &GitHubAuditLogEntry) -> (Option<String>, Option<serde_json::Value>, Option<serde_json::Value>) {
    let target = entry.repo.clone()
        .or_else(|| entry.team.clone())
        .or_else(|| entry.user.clone());

    let (old_value, new_value) = if let Some(ref data) = entry.data {
        let old = data.get("old").cloned();
        let new = data.get("new").cloned();
        (old, new)
    } else {
        (None, None)
    };

    (target, old_value, new_value)
}

