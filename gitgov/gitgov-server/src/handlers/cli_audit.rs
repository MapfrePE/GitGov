// ============================================================================
// CLI COMMAND AUDIT — /cli/commands endpoint
// ============================================================================

pub async fn ingest_cli_command(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CliCommandInput>,
) -> impl IntoResponse {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let record = CliCommandRecord {
        id: id.clone(),
        org_id: auth_user.org_id.clone(),
        user_login: auth_user.client_id.clone(),
        command: payload.command.clone(),
        origin: payload.origin.clone(),
        branch: payload.branch.clone(),
        repo_name: payload.repo_name.clone(),
        exit_code: payload.exit_code,
        duration_ms: payload.duration_ms,
        metadata: payload.metadata.clone(),
        created_at: now,
    };

    match state.db.insert_cli_command(&record).await {
        Ok(()) => (
            StatusCode::OK,
            Json(CliCommandResponse {
                accepted: true,
                id: Some(id),
                error: None,
            }),
        ),
        Err(e) => {
            tracing::warn!(error = %e, user = %auth_user.client_id, "Failed to insert CLI command audit");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CliCommandResponse {
                    accepted: false,
                    id: None,
                    error: Some("Failed to record command".to_string()),
                }),
            )
        }
    }
}

pub async fn list_cli_commands(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<CliCommandQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);

    // Admin sees all, developer sees only their own
    let user_filter = if auth_user.role == UserRole::Admin {
        query.user_login.as_deref()
    } else {
        Some(auth_user.client_id.as_str())
    };

    match state
        .db
        .list_cli_commands(auth_user.org_id.as_deref(), user_filter, limit, offset)
        .await
    {
        Ok((records, total)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "commands": records,
                "total": total,
                "limit": limit,
                "offset": offset,
            })),
        ),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list CLI commands");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "commands": [],
                    "total": 0,
                    "error": "Failed to list commands",
                })),
            )
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CliCommandQuery {
    pub user_login: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}
