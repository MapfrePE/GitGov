// ============================================================================
// POLICY DRIFT AUDIT — /policy/drift-events endpoint
// ============================================================================

pub async fn ingest_policy_drift_event(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PolicyDriftEventInput>,
) -> impl IntoResponse {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let action = payload.action.trim().to_lowercase();
    let result = payload.result.trim().to_lowercase();
    let is_valid_action = matches!(action.as_str(), "sync_local" | "push_local" | "drift_snapshot");
    let is_valid_result = matches!(result.as_str(), "success" | "failed" | "observed");

    if !is_valid_action || !is_valid_result || payload.repo_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(PolicyDriftEventResponse {
                accepted: false,
                id: None,
                error: Some("Invalid policy drift payload".to_string()),
            }),
        );
    }

    let record = PolicyDriftEventRecord {
        id: id.clone(),
        org_id: auth_user.org_id.clone(),
        user_login: auth_user.client_id.clone(),
        action,
        repo_name: payload.repo_name.trim().to_string(),
        result,
        before_checksum: payload.before_checksum.clone(),
        after_checksum: payload.after_checksum.clone(),
        duration_ms: payload.duration_ms,
        metadata: payload.metadata.clone(),
        created_at: now,
    };

    match state.db.insert_policy_drift_event(&record).await {
        Ok(()) => {
            maybe_notify_critical_drift(&state, &record);
            (
                StatusCode::OK,
                Json(PolicyDriftEventResponse {
                    accepted: true,
                    id: Some(id),
                    error: None,
                }),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, user = %auth_user.client_id, "Failed to insert policy drift event");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PolicyDriftEventResponse {
                    accepted: false,
                    id: None,
                    error: Some("Failed to record policy drift event".to_string()),
                }),
            )
        }
    }
}

pub async fn list_policy_drift_events(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PolicyDriftEventQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let repo_filter = query.repo_name.as_deref();

    // Admin sees all, developer sees only their own events.
    let user_filter = if auth_user.role == UserRole::Admin {
        query.user_login.as_deref()
    } else {
        Some(auth_user.client_id.as_str())
    };

    match state
        .db
        .list_policy_drift_events(
            auth_user.org_id.as_deref(),
            user_filter,
            repo_filter,
            limit,
            offset,
        )
        .await
    {
        Ok((records, total)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "events": records,
                "total": total,
                "limit": limit,
                "offset": offset,
            })),
        ),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list policy drift events");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "events": [],
                    "total": 0,
                    "error": "Failed to list policy drift events",
                })),
            )
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyDriftEventQuery {
    pub user_login: Option<String>,
    pub repo_name: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

fn metadata_i64(metadata: &serde_json::Value, key: &str) -> Option<i64> {
    match metadata.get(key) {
        Some(serde_json::Value::Number(n)) => n.as_i64(),
        Some(serde_json::Value::String(s)) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn extract_critical_drift_counts(record: &PolicyDriftEventRecord) -> Option<(i64, i64)> {
    if record.action != "drift_snapshot" || record.result != "observed" {
        return None;
    }

    let critical_count = metadata_i64(&record.metadata, "critical_count").unwrap_or(0);
    if critical_count <= 0 {
        return None;
    }

    let drift_count = metadata_i64(&record.metadata, "drift_count").unwrap_or(critical_count);
    Some((drift_count.max(critical_count), critical_count))
}

fn drift_alert_cache() -> &'static Mutex<HashMap<String, Instant>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn should_emit_critical_drift_alert(cache_key: &str) -> bool {
    const DEDUP_WINDOW: Duration = Duration::from_secs(20 * 60);
    let now = Instant::now();
    let Ok(mut cache) = drift_alert_cache().lock() else {
        return true;
    };

    cache.retain(|_, seen_at| now.duration_since(*seen_at) < DEDUP_WINDOW);
    if cache.contains_key(cache_key) {
        return false;
    }
    cache.insert(cache_key.to_string(), now);
    true
}

fn resolve_drift_alert_webhook_targets(
    dedicated_targets: &[String],
    generic_fallback: Option<&str>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    let mut push_unique = |candidate: &str| {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return;
        }
        if out.iter().any(|existing| existing == trimmed) {
            return;
        }
        out.push(trimmed.to_string());
    };

    if !dedicated_targets.is_empty() {
        for target in dedicated_targets {
            push_unique(target);
        }
        return out;
    }

    if let Some(fallback) = generic_fallback {
        push_unique(fallback);
    }

    out
}

fn maybe_notify_critical_drift(state: &Arc<AppState>, record: &PolicyDriftEventRecord) {
    let webhook_targets = resolve_drift_alert_webhook_targets(
        &state.drift_alert_webhook_urls,
        state.alert_webhook_url.as_deref(),
    );
    if webhook_targets.is_empty() {
        return;
    }

    let Some((drift_count, critical_count)) = extract_critical_drift_counts(record) else {
        return;
    };

    let drift_signature = record
        .after_checksum
        .as_deref()
        .or(record.before_checksum.as_deref())
        .unwrap_or("unknown");
    let dedup_key = format!("{}|{}|{}", record.repo_name, drift_signature, critical_count);
    if !should_emit_critical_drift_alert(&dedup_key) {
        return;
    }

    let text = notifications::format_critical_policy_drift_alert(
        &record.user_login,
        &record.repo_name,
        drift_count,
        critical_count,
    );
    let client = state.http_client.clone();
    let target_count = webhook_targets.len();
    tracing::debug!(
        repo = %record.repo_name,
        drift_count,
        critical_count,
        target_count,
        "Dispatching critical policy drift alert"
    );
    tokio::spawn(async move {
        for webhook_url in webhook_targets {
            notifications::send_alert(&client, &webhook_url, text.clone()).await;
        }
    });
}

#[cfg(test)]
mod policy_drift_audit_tests {
    use super::*;

    fn record(action: &str, result: &str, metadata: serde_json::Value) -> PolicyDriftEventRecord {
        PolicyDriftEventRecord {
            id: "00000000-0000-0000-0000-000000000001".to_string(),
            org_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
            user_login: "dev".to_string(),
            action: action.to_string(),
            repo_name: "org/repo".to_string(),
            result: result.to_string(),
            before_checksum: None,
            after_checksum: Some("abc123".to_string()),
            duration_ms: Some(10),
            metadata,
            created_at: 0,
        }
    }

    #[test]
    fn extract_critical_drift_counts_requires_snapshot_observed_with_critical_count() {
        let wrong_action = record(
            "push_local",
            "observed",
            serde_json::json!({"critical_count": 1, "drift_count": 2}),
        );
        assert!(extract_critical_drift_counts(&wrong_action).is_none());

        let wrong_result = record(
            "drift_snapshot",
            "success",
            serde_json::json!({"critical_count": 1, "drift_count": 2}),
        );
        assert!(extract_critical_drift_counts(&wrong_result).is_none());

        let no_critical = record(
            "drift_snapshot",
            "observed",
            serde_json::json!({"critical_count": 0, "drift_count": 5}),
        );
        assert!(extract_critical_drift_counts(&no_critical).is_none());
    }

    #[test]
    fn extract_critical_drift_counts_accepts_numeric_or_string_metadata() {
        let numeric = record(
            "drift_snapshot",
            "observed",
            serde_json::json!({"critical_count": 2, "drift_count": 5}),
        );
        assert_eq!(extract_critical_drift_counts(&numeric), Some((5, 2)));

        let stringy = record(
            "drift_snapshot",
            "observed",
            serde_json::json!({"critical_count": "3", "drift_count": "1"}),
        );
        assert_eq!(extract_critical_drift_counts(&stringy), Some((3, 3)));
    }

    #[test]
    fn resolve_drift_targets_prefers_dedicated_webhooks_and_deduplicates() {
        let dedicated = vec![
            " https://hooks.example/a ".to_string(),
            "https://hooks.example/b".to_string(),
            "https://hooks.example/a".to_string(),
            "".to_string(),
        ];
        let targets = resolve_drift_alert_webhook_targets(
            &dedicated,
            Some("https://hooks.example/fallback"),
        );
        assert_eq!(
            targets,
            vec![
                "https://hooks.example/a".to_string(),
                "https://hooks.example/b".to_string()
            ]
        );
    }

    #[test]
    fn resolve_drift_targets_falls_back_to_generic_webhook() {
        let targets =
            resolve_drift_alert_webhook_targets(&[], Some(" https://hooks.example/fallback "));
        assert_eq!(targets, vec!["https://hooks.example/fallback".to_string()]);

        let none_targets = resolve_drift_alert_webhook_targets(&[], None);
        assert!(none_targets.is_empty());
    }
}
