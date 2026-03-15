// ============================================================================
// CLIENT EVENTS (Batch Ingest)
// ============================================================================

pub async fn ingest_client_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(batch): Json<ClientEventBatch>,
) -> impl IntoResponse {
    let batch_len = batch.events.len();
    metrics::histogram!("gitgov_events_batch_size").record(batch_len as f64);
    if state.events_max_batch > 0 && batch_len > state.events_max_batch {
        tracing::warn!(
            auth_user = %auth_user.client_id,
            batch_len,
            max_batch = state.events_max_batch,
            "Rejecting /events payload because it exceeds max configured batch size"
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(ClientEventResponse {
                accepted: vec![],
                duplicates: vec![],
                errors: vec![EventError {
                    event_uuid: "batch".to_string(),
                    error: format!(
                        "Too many events in a single request: {} (max {})",
                        batch_len, state.events_max_batch
                    ),
                }],
            }),
        );
    }

    let mut events = Vec::new();
    let mut pre_validation_errors: Vec<EventError> = Vec::new();
    let strict_actor_match = state.strict_actor_match;
    let mut org_id_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut repo_cache: HashMap<String, Option<Repo>> = HashMap::new();

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
            pre_validation_errors.push(EventError {
                event_uuid: input.event_uuid,
                error: "user_login must match authenticated client_id (STRICT_ACTOR_MATCH)"
                    .to_string(),
            });
            continue;
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
            pre_validation_errors.push(EventError {
                event_uuid: input.event_uuid,
                error: "synthetic user_login is not allowed in this environment".to_string(),
            });
            continue;
        }

        // Get org and repo IDs
        let requested_org_id = if let Some(ref org_name) = input.org_name {
            if let Some(cached) = org_id_cache.get(org_name) {
                cached.clone()
            } else {
                let resolved = resolve_org_id_with_cache(&state, org_name).await;
                org_id_cache.insert(org_name.clone(), resolved.clone());
                resolved
            }
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
                    pre_validation_errors.push(EventError {
                        event_uuid: input.event_uuid,
                        error: "Event org_name is outside API key scope".to_string(),
                    });
                    continue;
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
            if let Some(cached) = repo_cache.get(repo_full_name) {
                cached.clone()
            } else {
                let resolved = resolve_repo_with_cache(&state, repo_full_name).await;
                repo_cache.insert(repo_full_name.clone(), resolved.clone());
                resolved
            }
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
                    pre_validation_errors.push(EventError {
                        event_uuid: input.event_uuid,
                        error: "Event repo_full_name is outside API key scope".to_string(),
                    });
                    continue;
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
            if should_schedule_repo_upsert(state.as_ref(), effective_org_id, full_name) {
                schedule_repo_upsert(
                    Arc::clone(&state),
                    effective_org_id.to_string(),
                    full_name.to_string(),
                    repo_name.to_string(),
                    input.event_uuid.clone(),
                );
            }
            None
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

    if events.is_empty() {
        return (
            StatusCode::OK,
            Json(ClientEventResponse {
                accepted: vec![],
                duplicates: vec![],
                errors: pre_validation_errors,
            }),
        );
    }

    match state.db.insert_client_events_batch(&events).await {
        Ok(mut response) => {
            if !pre_validation_errors.is_empty() {
                response.errors.extend(pre_validation_errors);
            }
            // Prometheus counters
            metrics::counter!("gitgov_events_ingested_total", "status" => "accepted")
                .increment(response.accepted.len() as u64);
            metrics::counter!("gitgov_events_ingested_total", "status" => "duplicate")
                .increment(response.duplicates.len() as u64);
            metrics::counter!("gitgov_events_ingested_total", "status" => "error")
                .increment(response.errors.len() as u64);

            // Notify SSE subscribers about new events (fire-and-forget).
            // Single notification — frontend refreshes both logs and stats on new_events.
            let accepted_count = response.accepted.len() as u32;
            if accepted_count > 0 {
                fanout_sse_new_events(&state, accepted_count).await;
            }

            // Fire-and-forget (debounced): update client_sessions last_seen + device metadata.
            let should_touch_session = !response.accepted.is_empty() || !response.duplicates.is_empty();
            if should_touch_session {
                let client_id = auth_user.client_id.clone();
                let org_id = auth_user.org_id.clone();
                // Extract device metadata from the first event that has it
                let device_meta = events
                    .iter()
                    .find_map(|e| {
                        e.metadata.get("device").cloned()
                    })
                    .unwrap_or(serde_json::json!({}));
                if should_upsert_client_session(&state, &client_id) {
                    let db = Arc::clone(&state.db);
                    tokio::spawn(async move {
                        if let Err(e) = db
                            .upsert_client_session(&client_id, org_id.as_deref(), &device_meta)
                            .await
                        {
                            tracing::debug!(
                                error = %e,
                                "Failed to upsert client session (non-critical)"
                            );
                        }
                    });
                }
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
            let mut errors = pre_validation_errors;
            errors.push(EventError {
                event_uuid: "batch".to_string(),
                error: "Internal database error".to_string(),
            });
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ClientEventResponse {
                    accepted: vec![],
                    duplicates: vec![],
                    errors,
                }),
            )
        }
    }
}

// ============================================================================
// QUERY ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OutboxLeaseRequest {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub holder: Option<String>,
    #[serde(default)]
    pub lease_ttl_ms: Option<u64>,
    #[serde(default)]
    pub max_wait_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutboxLeaseResponse {
    pub granted: bool,
    pub wait_ms: u64,
    pub lease_ttl_ms: u64,
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutboxLeaseTelemetryResponse {
    pub enabled: bool,
    pub default_lease_ttl_ms: u64,
    pub telemetry: OutboxLeaseTelemetrySnapshot,
}

#[derive(Debug, Clone, Copy)]
struct OutboxLeaseTelemetryInput {
    mode: OutboxLeaseTelemetryMode,
    requested_ttl_ms: u64,
    effective_ttl_ms: u64,
    wait_ms: u64,
    ttl_clamped: bool,
    wait_clamped: bool,
    request_started: Instant,
}

fn record_outbox_lease_telemetry(state: &Arc<AppState>, input: OutboxLeaseTelemetryInput) {
    match state.outbox_lease_telemetry.lock() {
        Ok(mut telemetry) => telemetry.record(OutboxLeaseTelemetryRecord {
            mode: input.mode,
            requested_ttl_ms: input.requested_ttl_ms,
            effective_ttl_ms: input.effective_ttl_ms,
            wait_ms: input.wait_ms,
            ttl_clamped: input.ttl_clamped,
            wait_clamped: input.wait_clamped,
            handler_duration_ms: input.request_started.elapsed().as_millis() as u64,
        }),
        Err(_) => tracing::warn!("Outbox lease telemetry lock poisoned; skipping telemetry record"),
    }
}

pub async fn acquire_outbox_flush_lease(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<OutboxLeaseRequest>,
) -> impl IntoResponse {
    let request_started = Instant::now();
    let requested_ttl_ms = request
        .lease_ttl_ms
        .unwrap_or(state.outbox_server_lease_ttl_ms);
    let lease_ttl_ms = requested_ttl_ms.clamp(1_000, 60_000);
    let ttl_clamped = requested_ttl_ms != lease_ttl_ms;
    let requested_max_wait_ms = request.max_wait_ms.unwrap_or(lease_ttl_ms);
    let max_wait_ms = requested_max_wait_ms.clamp(250, 120_000);
    let wait_clamped = requested_max_wait_ms != max_wait_ms;

    if !state.outbox_server_lease_enabled {
        record_outbox_lease_telemetry(
            &state,
            OutboxLeaseTelemetryInput {
                mode: OutboxLeaseTelemetryMode::DisabledFailOpen,
                requested_ttl_ms,
                effective_ttl_ms: lease_ttl_ms,
                wait_ms: 0,
                ttl_clamped,
                wait_clamped,
                request_started,
            },
        );
        return (
            StatusCode::OK,
            Json(OutboxLeaseResponse {
                granted: true,
                wait_ms: 0,
                lease_ttl_ms: state.outbox_server_lease_ttl_ms,
                mode: "disabled_fail_open".to_string(),
            }),
        );
    }

    let scope = request
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| auth_user.org_id.clone())
        .unwrap_or_else(|| "global".to_string());
    let scope_key = format!("flush:{}", scope);
    let holder = request
        .holder
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            format!(
                "client:{}:{}",
                auth_user.client_id,
                auth_user.org_id.as_deref().unwrap_or("global")
            )
        });

    match state
        .db
        .try_acquire_outbox_flush_lease(&scope_key, &holder, Duration::from_millis(lease_ttl_ms))
        .await
    {
        Ok(decision) => {
            let response_wait_ms = decision.wait_ms.min(max_wait_ms);
            record_outbox_lease_telemetry(
                &state,
                OutboxLeaseTelemetryInput {
                    mode: if decision.granted {
                        OutboxLeaseTelemetryMode::Granted
                    } else {
                        OutboxLeaseTelemetryMode::Denied
                    },
                    requested_ttl_ms,
                    effective_ttl_ms: lease_ttl_ms,
                    wait_ms: response_wait_ms,
                    ttl_clamped,
                    wait_clamped,
                    request_started,
                },
            );
            (
            StatusCode::OK,
            Json(OutboxLeaseResponse {
                granted: decision.granted,
                wait_ms: response_wait_ms,
                lease_ttl_ms,
                mode: "server_lease".to_string(),
            }),
        )
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                scope_key = %scope_key,
                holder = %holder,
                "Outbox lease acquisition failed; returning fail-open grant"
            );
            record_outbox_lease_telemetry(
                &state,
                OutboxLeaseTelemetryInput {
                    mode: OutboxLeaseTelemetryMode::DbErrorFailOpen,
                    requested_ttl_ms,
                    effective_ttl_ms: lease_ttl_ms,
                    wait_ms: 0,
                    ttl_clamped,
                    wait_clamped,
                    request_started,
                },
            );
            (
                StatusCode::OK,
                Json(OutboxLeaseResponse {
                    granted: true,
                    wait_ms: 0,
                    lease_ttl_ms,
                    mode: "db_error_fail_open".to_string(),
                }),
            )
        }
    }
}

pub async fn get_outbox_lease_metrics(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let telemetry = match state.outbox_lease_telemetry.lock() {
        Ok(telemetry) => telemetry.snapshot(),
        Err(_) => {
            tracing::warn!("Outbox lease telemetry lock poisoned while reading");
            OutboxLeaseTelemetrySnapshot::default()
        }
    };

    (
        StatusCode::OK,
        Json(OutboxLeaseTelemetryResponse {
            enabled: state.outbox_server_lease_enabled,
            default_lease_ttl_ms: state.outbox_server_lease_ttl_ms,
            telemetry,
        }),
    )
        .into_response()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsResponse {
    pub events: Vec<CombinedEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecations: Option<Vec<String>>,
}

const GLOBAL_STATS_CACHE_KEY: &str = "__global__";
const MAX_STATS_CACHE_ENTRIES: usize = 256;
const MAX_LOGS_CACHE_ENTRIES: usize = 512;
const LOGS_OFFSET_DEPRECATION_NOTICE: &str =
    "The /logs `offset` query parameter is deprecated. Prefer keyset pagination with `before_created_at` and `before_id`.";

fn logs_deprecations_for_request(filter: &EventFilter) -> Option<Vec<String>> {
    (filter.offset > 0).then(|| vec![LOGS_OFFSET_DEPRECATION_NOTICE.to_string()])
}

fn should_reject_logs_offset(filter: &EventFilter, reject_offset_pagination: bool) -> bool {
    reject_offset_pagination && filter.offset > 0 && filter.before_created_at.is_none()
}

fn stats_cache_key(org_id: Option<&str>) -> String {
    org_id.unwrap_or(GLOBAL_STATS_CACHE_KEY).to_string()
}

fn get_cached_stats(state: &AppState, org_id: Option<&str>) -> Option<AuditStats> {
    if state.stats_cache_ttl.is_zero() {
        return None;
    }

    let now = Instant::now();
    let key = stats_cache_key(org_id);
    let mut cache = match state.stats_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("Stats cache lock poisoned while reading; bypassing cache");
            return None;
        }
    };

    if let Some(entry) = cache.get(&key) {
        if entry.expires_at > now {
            return Some(entry.stats.clone());
        }
    }
    cache.remove(&key);
    None
}

fn put_cached_stats(state: &AppState, org_id: Option<&str>, stats: &AuditStats) {
    if state.stats_cache_ttl.is_zero() {
        return;
    }

    let key = stats_cache_key(org_id);
    let expires_at = Instant::now() + state.stats_cache_ttl;
    let mut cache = match state.stats_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("Stats cache lock poisoned while writing; skipping cache write");
            return;
        }
    };

    cache.insert(
        key,
        StatsCacheEntry {
            stats: stats.clone(),
            expires_at,
        },
    );
    if cache.len() > MAX_STATS_CACHE_ENTRIES {
        cache.retain(|_, entry| entry.expires_at > Instant::now());
    }
}

fn invalidate_stats_cache(state: &AppState) {
    if state.stats_cache_ttl.is_zero() {
        return;
    }
    let min_interval = if state.stats_cache_invalidation_min_interval.is_zero() {
        state.cache_invalidation_min_interval
    } else {
        state.stats_cache_invalidation_min_interval
    };
    if !should_invalidate_cache(
        &state.stats_cache_last_invalidation_ms,
        min_interval,
    ) {
        return;
    }

    match state.stats_cache.lock() {
        Ok(mut cache) => {
            if !cache.is_empty() {
                cache.clear();
            }
        }
        Err(_) => {
            tracing::warn!("Stats cache lock poisoned while invalidating");
        }
    }
}

fn logs_cache_key(role: &UserRole, filter: &EventFilter) -> Option<String> {
    if filter.before_created_at.is_some() || filter.offset > 0 {
        // Cursor/offset pages are rarely repeated; avoid polluting cache.
        return None;
    }
    let role_scope = role.as_str();
    serde_json::to_string(filter)
        .ok()
        .map(|serialized| format!("{role_scope}|{serialized}"))
}

fn get_cached_logs(state: &AppState, key: &str) -> Option<Vec<CombinedEvent>> {
    if state.logs_cache_ttl.is_zero() {
        return None;
    }

    let now = Instant::now();
    let mut cache = match state.logs_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("Logs cache lock poisoned while reading; bypassing cache");
            return None;
        }
    };

    if let Some(entry) = cache.get(key) {
        if entry.expires_at > now {
            return Some(entry.events.clone());
        }
    }
    cache.remove(key);
    None
}

fn get_cached_logs_on_error(state: &AppState, key: &str) -> Option<Vec<CombinedEvent>> {
    if state.logs_cache_ttl.is_zero() {
        return None;
    }
    let grace = state.logs_cache_stale_on_error;
    if grace.is_zero() {
        return None;
    }

    let now = Instant::now();
    let mut cache = match state.logs_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("Logs cache lock poisoned while serving stale fallback");
            return None;
        }
    };

    if let Some(entry) = cache.get(key) {
        let stale_deadline = entry.expires_at + grace;
        if stale_deadline > now {
            return Some(entry.events.clone());
        }
    }

    cache.remove(key);
    None
}

fn put_cached_logs(state: &AppState, key: &str, events: &[CombinedEvent]) {
    if state.logs_cache_ttl.is_zero() {
        return;
    }

    let expires_at = Instant::now() + state.logs_cache_ttl;
    let mut cache = match state.logs_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("Logs cache lock poisoned while writing; skipping cache write");
            return;
        }
    };

    cache.insert(
        key.to_string(),
        LogsCacheEntry {
            events: events.to_vec(),
            expires_at,
        },
    );

    if cache.len() > MAX_LOGS_CACHE_ENTRIES {
        cache.retain(|_, entry| entry.expires_at > Instant::now());
    }
}

fn invalidate_logs_cache(state: &AppState) {
    if state.logs_cache_ttl.is_zero() {
        return;
    }
    let min_interval = if state.logs_cache_invalidation_min_interval.is_zero() {
        state.cache_invalidation_min_interval
    } else {
        state.logs_cache_invalidation_min_interval
    };
    if !should_invalidate_cache(
        &state.logs_cache_last_invalidation_ms,
        min_interval,
    ) {
        return;
    }

    match state.logs_cache.lock() {
        Ok(mut cache) => {
            if !cache.is_empty() {
                cache.clear();
            }
        }
        Err(_) => {
            tracing::warn!("Logs cache lock poisoned while invalidating");
        }
    }
}

const MAX_ORG_LOOKUP_CACHE_ENTRIES: usize = 2_048;
const MAX_REPO_LOOKUP_CACHE_ENTRIES: usize = 8_192;

async fn resolve_org_id_with_cache(state: &AppState, org_name: &str) -> Option<String> {
    let cache_key = org_name.trim().to_ascii_lowercase();
    if cache_key.is_empty() {
        return None;
    }

    if let Some(cached) = get_cached_org_id(state, &cache_key) {
        return cached;
    }

    let resolved = state
        .db
        .get_org_by_login(org_name)
        .await
        .ok()
        .flatten()
        .map(|org| org.id);
    put_cached_org_id(state, cache_key, resolved.clone());
    resolved
}

fn get_cached_org_id(state: &AppState, cache_key: &str) -> Option<Option<String>> {
    if state.org_lookup_cache_ttl.is_zero() {
        return None;
    }

    let now = Instant::now();
    let mut cache = state.org_lookup_cache.lock().ok()?;
    let entry = cache.get(cache_key)?;
    if entry.expires_at <= now {
        cache.remove(cache_key);
        return None;
    }
    Some(entry.org_id.clone())
}

fn put_cached_org_id(state: &AppState, cache_key: String, org_id: Option<String>) {
    if state.org_lookup_cache_ttl.is_zero() {
        return;
    }

    let now = Instant::now();
    let mut cache = match state.org_lookup_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("org_lookup_cache lock poisoned; skipping cache write");
            return;
        }
    };
    cache.insert(
        cache_key,
        OrgLookupCacheEntry {
            org_id,
            expires_at: now + state.org_lookup_cache_ttl,
        },
    );
    if cache.len() > MAX_ORG_LOOKUP_CACHE_ENTRIES {
        cache.retain(|_, entry| entry.expires_at > now);
    }
}

async fn resolve_repo_with_cache(state: &AppState, repo_full_name: &str) -> Option<Repo> {
    let cache_key = repo_full_name.trim().to_ascii_lowercase();
    if cache_key.is_empty() {
        return None;
    }

    if let Some(cached) = get_cached_repo(state, &cache_key) {
        return cached;
    }

    let resolved = state
        .db
        .get_repo_by_full_name(repo_full_name)
        .await
        .unwrap_or_default();
    put_cached_repo(state, cache_key, resolved.clone());
    resolved
}

fn get_cached_repo(state: &AppState, cache_key: &str) -> Option<Option<Repo>> {
    if state.repo_lookup_cache_ttl.is_zero() {
        return None;
    }

    let now = Instant::now();
    let mut cache = state.repo_lookup_cache.lock().ok()?;
    let entry = cache.get(cache_key)?;
    if entry.expires_at <= now {
        cache.remove(cache_key);
        return None;
    }
    Some(entry.repo.clone())
}

fn put_cached_repo(state: &AppState, cache_key: String, repo: Option<Repo>) {
    if state.repo_lookup_cache_ttl.is_zero() {
        return;
    }

    let now = Instant::now();
    let mut cache = match state.repo_lookup_cache.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("repo_lookup_cache lock poisoned; skipping cache write");
            return;
        }
    };
    cache.insert(
        cache_key,
        RepoLookupCacheEntry {
            repo,
            expires_at: now + state.repo_lookup_cache_ttl,
        },
    );
    if cache.len() > MAX_REPO_LOOKUP_CACHE_ENTRIES {
        cache.retain(|_, entry| entry.expires_at > now);
    }
}

const MAX_TRACKED_REPO_UPSERT_ATTEMPTS: usize = 8_192;

fn should_schedule_repo_upsert(state: &AppState, org_id: &str, repo_full_name: &str) -> bool {
    if state.repo_upsert_min_interval.is_zero() {
        return true;
    }
    let cache_key = format!("{}:{}", org_id.trim(), repo_full_name.trim().to_ascii_lowercase());
    if cache_key.ends_with(':') {
        return false;
    }

    let now = Instant::now();
    let mut cache = match state.repo_upsert_last_attempt.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("repo_upsert_last_attempt lock poisoned; bypassing debounce");
            return true;
        }
    };

    if let Some(last_attempt) = cache.get(&cache_key) {
        if now.saturating_duration_since(*last_attempt) < state.repo_upsert_min_interval {
            return false;
        }
    }
    cache.insert(cache_key, now);
    if cache.len() > MAX_TRACKED_REPO_UPSERT_ATTEMPTS {
        let stale_after = std::cmp::max(state.repo_upsert_min_interval, Duration::from_secs(120));
        cache.retain(|_, ts| now.saturating_duration_since(*ts) <= stale_after);
    }

    true
}

fn schedule_repo_upsert(
    state: Arc<AppState>,
    org_id: String,
    full_name: String,
    repo_name: String,
    event_uuid: String,
) {
    tokio::spawn(async move {
        match state
            .db
            .upsert_repo_by_full_name(Some(org_id.as_str()), &full_name, &repo_name, true)
            .await
        {
            Ok(repo_id) => {
                let repo = Repo {
                    id: repo_id,
                    org_id: Some(org_id),
                    github_id: None,
                    full_name: full_name.clone(),
                    name: repo_name,
                    private: true,
                    created_at: chrono::Utc::now().timestamp_millis(),
                };
                put_cached_repo(&state, full_name.to_ascii_lowercase(), Some(repo));
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    repo = %full_name,
                    event_uuid = %event_uuid,
                    "Background repo upsert from /events failed (non-fatal)"
                );
            }
        }
    });
}

fn should_invalidate_cache(
    last_invalidation_ms: &Arc<AtomicI64>,
    min_interval: Duration,
) -> bool {
    if min_interval.is_zero() {
        return true;
    }

    let min_interval_ms = min_interval.as_millis().min(i64::MAX as u128) as i64;
    let now_ms = chrono::Utc::now().timestamp_millis();
    loop {
        let previous_ms = last_invalidation_ms.load(Ordering::Acquire);
        if previous_ms > 0 && now_ms.saturating_sub(previous_ms) < min_interval_ms {
            return false;
        }
        if last_invalidation_ms
            .compare_exchange(previous_ms, now_ms, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return true;
        }
    }
}

const MAX_TRACKED_CLIENT_SESSIONS: usize = 8_192;

fn should_upsert_client_session(state: &AppState, client_id: &str) -> bool {
    if state.client_session_upsert_min_interval.is_zero() {
        return true;
    }

    let now = Instant::now();
    let min_interval = state.client_session_upsert_min_interval;
    let mut cache = match state.client_session_last_upsert.lock() {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("client_session_last_upsert lock poisoned; bypassing debounce");
            return true;
        }
    };

    if let Some(last_seen) = cache.get(client_id) {
        if now.saturating_duration_since(*last_seen) < min_interval {
            return false;
        }
    }

    cache.insert(client_id.to_string(), now);
    if cache.len() > MAX_TRACKED_CLIENT_SESSIONS {
        let stale_after = std::cmp::max(min_interval, Duration::from_secs(120));
        cache.retain(|_, last_seen| now.saturating_duration_since(*last_seen) <= stale_after);
    }

    true
}

async fn load_audit_stats(state: &AppState, org_id: Option<&str>) -> Result<AuditStats, DbError> {
    if let Some(stats) = get_cached_stats(state, org_id) {
        return Ok(stats);
    }

    // Single-flight guard: if many requests miss cache simultaneously,
    // only one recomputes stats while others wait and reuse the result.
    let _refresh_guard = state.stats_cache_refresh_lock.lock().await;
    if let Some(stats) = get_cached_stats(state, org_id) {
        return Ok(stats);
    }

    let stats_fut = state.db.get_stats(org_id);
    let pipeline_fut = state.db.get_pipeline_health_stats(org_id);
    let desktop_pushes_fut = state.db.get_desktop_pushes_today(org_id);
    let (stats_result, pipeline_result, desktop_pushes_result) =
        tokio::join!(stats_fut, pipeline_fut, desktop_pushes_fut);

    let mut stats = stats_result?;
    stats.pipeline = pipeline_result.unwrap_or_default();
    stats.client_events.desktop_pushes_today = match desktop_pushes_result {
        Ok(count) => count,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to compute desktop pushes today for /stats");
            0
        }
    };
    put_cached_stats(state, org_id, &stats);
    Ok(stats)
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
    let deprecations = logs_deprecations_for_request(&filter);

    if filter.offset > 0 {
        tracing::warn!(
            requested_offset = filter.offset,
            "Deprecated /logs offset pagination requested; prefer keyset cursor"
        );
    }
    if should_reject_logs_offset(&filter, state.logs_reject_offset_pagination) {
        return (
            StatusCode::BAD_REQUEST,
            Json(LogsResponse {
                events: vec![],
                error: Some(
                    "offset pagination is disabled; use before_created_at and before_id"
                        .to_string(),
                ),
                stale: None,
                deprecations,
            }),
        );
    }

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
                    stale: None,
                    deprecations: deprecations.clone(),
                }),
            );
        }
    };
    if scoped_org_id.is_some() {
        // Prefer UUID scope to avoid extra org_name lookups in DB query path.
        filter.org_id = scoped_org_id;
        filter.org_name = None;
    }
    // Keyset pagination path should not also apply offset pagination.
    if filter.before_created_at.is_some() {
        filter.offset = 0;
    }
    let logs_key = logs_cache_key(&auth_user.role, &filter);
    if let Some(cache_key) = logs_key.as_deref() {
        if let Some(cached_events) = get_cached_logs(&state, cache_key) {
            return (
                StatusCode::OK,
                Json(LogsResponse {
                    events: cached_events,
                    error: None,
                    stale: None,
                    deprecations: deprecations.clone(),
                }),
            );
        }
    }

    match state.db.get_combined_events(&filter).await {
        Ok(events) => {
            if let Some(cache_key) = logs_key.as_deref() {
                put_cached_logs(&state, cache_key, &events);
            }
            (
                StatusCode::OK,
                Json(LogsResponse {
                    events,
                    error: None,
                    stale: None,
                    deprecations: deprecations.clone(),
                }),
            )
        }
        Err(e) => {
            if let Some(cache_key) = logs_key.as_deref() {
                if let Some(events) = get_cached_logs_on_error(&state, cache_key) {
                    tracing::warn!(
                        error = %e,
                        "Serving stale /logs cache due transient database error"
                    );
                    return (
                        StatusCode::OK,
                        Json(LogsResponse {
                            events,
                            error: None,
                            stale: Some(true),
                            deprecations: deprecations.clone(),
                        }),
                    );
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LogsResponse {
                    events: vec![],
                    error: Some("Internal database error".to_string()),
                    stale: None,
                    deprecations,
                }),
            )
        }
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
    match load_audit_stats(&state, org_id).await {
        Ok(stats) => (StatusCode::OK, Json(stats)),
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
    let stats = load_audit_stats(&state, org_id).await.unwrap_or_default();

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

fn org_name_from_repo_full_name(repo_full_name: &str) -> Option<&str> {
    repo_full_name
        .split('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn should_block_policy_check_transport(
    state: &AppState,
    repo_full_name: &str,
    branch: &str,
) -> bool {
    if state.policy_check_block_scopes.is_empty() {
        return false;
    }
    let org_name = match org_name_from_repo_full_name(repo_full_name) {
        Some(value) => value,
        None => return false,
    };
    state
        .policy_check_block_scopes
        .iter()
        .any(|scope| scope.matches(org_name, branch))
}

fn policy_check_response_status(
    state: &AppState,
    repo_full_name: &str,
    branch: &str,
    response: &PolicyCheckResponse,
) -> StatusCode {
    if response.allowed {
        return StatusCode::OK;
    }
    if should_block_policy_check_transport(state, repo_full_name, branch) {
        StatusCode::CONFLICT
    } else {
        StatusCode::OK
    }
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
                ..Default::default()
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
                ..Default::default()
            }),
        );
    }

    metrics::counter!("gitgov_policy_checks_total").increment(1);

    let repo_name = repo_name_from_policy_check_input(&payload.repo);
    let branch = payload.branch.trim();
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
        ..Default::default()
    };

    let repo = match state.db.get_repo_by_full_name(&repo_name).await {
        Ok(Some(repo)) => repo,
        Ok(None) => {
            response.allowed = false;
            response.reasons.push("Repository not found in GitGov".to_string());
            let status = policy_check_response_status(state.as_ref(), &repo_name, branch, &response);
            return (status, Json(response));
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
            let status = policy_check_response_status(state.as_ref(), &repo_name, branch, &response);
            return (status, Json(response));
        }
        Err(_) => {
            response.allowed = false;
            response.reasons.push("Internal database error".to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
        }
    };
    let config = &policy.config;
    let enforcement = &config.enforcement;

    // Determine highest enforcement level applied
    let has_block = [
        &enforcement.pull_requests,
        &enforcement.commits,
        &enforcement.branches,
        &enforcement.traceability,
    ]
    .iter()
    .any(|e| **e == EnforcementLevel::Block);
    let has_warn = [
        &enforcement.pull_requests,
        &enforcement.commits,
        &enforcement.branches,
        &enforcement.traceability,
    ]
    .iter()
    .any(|e| **e == EnforcementLevel::Warn);

    response.advisory = !has_block;
    response.enforcement_applied = if has_block {
        "block".to_string()
    } else if has_warn {
        "warn".to_string()
    } else {
        "off".to_string()
    };

    // --- Branch rules ---
    if enforcement.branches != EnforcementLevel::Off {
        response.evaluated_rules.push("branch_name_valid".to_string());
        if !branch_matches_policy(config, branch) {
            let v = RuleViolation {
                rule: "branch_name_valid".to_string(),
                category: "branches".to_string(),
                enforcement: format!("{:?}", enforcement.branches).to_lowercase(),
                message: format!("Branch '{}' does not match configured patterns", branch),
            };
            if enforcement.branches == EnforcementLevel::Block {
                response.allowed = false;
                response.reasons.push(v.message.clone());
            } else {
                response.warnings.push(v.message.clone());
            }
            response.violations.push(v);
        }

        response.evaluated_rules.push("not_protected_branch".to_string());
        if config.branches.protected.iter().any(|p| p == branch) {
            let v = RuleViolation {
                rule: "not_protected_branch".to_string(),
                category: "branches".to_string(),
                enforcement: format!("{:?}", enforcement.branches).to_lowercase(),
                message: format!("Branch '{}' is protected; direct push not allowed", branch),
            };
            if enforcement.branches == EnforcementLevel::Block {
                response.allowed = false;
                response.reasons.push(v.message.clone());
            } else {
                response.warnings.push(v.message.clone());
            }
            response.violations.push(v);
        }

        if config.rules.block_force_push {
            response.evaluated_rules.push("no_force_push".to_string());
        }
    }

    // --- Commit rules ---
    if enforcement.commits != EnforcementLevel::Off {
        if config.rules.require_conventional_commits {
            response.evaluated_rules.push("conventional_commit".to_string());
        }
        if config.rules.require_signed_commits {
            response.evaluated_rules.push("signed_commit".to_string());
        }
        if let Some(max) = config.rules.max_files_per_commit {
            response.evaluated_rules.push(format!("max_files_per_commit_{}", max));
        }
        if !config.rules.forbidden_patterns.is_empty() {
            response.evaluated_rules.push("forbidden_patterns".to_string());
        }
    }

    // --- Pull request rules ---
    if enforcement.pull_requests != EnforcementLevel::Off {
        if config.rules.require_pull_request {
            response.evaluated_rules.push("require_pull_request".to_string());
        }
        if config.rules.min_approvals > 0 {
            response.evaluated_rules.push(format!("min_approvals_{}", config.rules.min_approvals));
        }
    }

    // --- Traceability rules ---
    if enforcement.traceability != EnforcementLevel::Off && config.rules.require_linked_ticket {
        response
            .evaluated_rules
            .push("require_linked_ticket".to_string());
    }

    if payload.commit.as_deref().unwrap_or_default().is_empty() {
        response
            .warnings
            .push("Commit SHA not provided; commit-specific checks skipped".to_string());
    }

    let status = policy_check_response_status(state.as_ref(), &repo_name, branch, &response);
    if status == StatusCode::CONFLICT {
        metrics::counter!("gitgov_policy_checks_transport_blocked_total").increment(1);
    }
    (status, Json(response))
}
