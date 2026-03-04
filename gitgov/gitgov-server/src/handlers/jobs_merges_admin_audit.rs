// ============================================================================
// JOB QUEUE MANAGEMENT ENDPOINTS
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct JobMetricsResponse {
    pub worker_id: String,
    pub metrics: JobMetrics,
}

pub async fn get_job_metrics(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Admin access required".to_string(),
                code: "FORBIDDEN".to_string(),
            }),
        )
            .into_response();
    }

    match state.db.get_job_metrics().await {
        Ok(metrics) => (
            StatusCode::OK,
            Json(JobMetricsResponse {
                worker_id: state.worker_id.clone(),
                metrics,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to get job metrics".to_string(),
                code: "INTERNAL_ERROR".to_string(),
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeadJobsResponse {
    pub jobs: Vec<Job>,
    pub total: usize,
}

pub async fn get_dead_jobs(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeadJobsQuery>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(DeadJobsResponse { jobs: vec![], total: 0 }),
        );
    }

    let limit = params.limit.unwrap_or(50);

    match state.db.get_dead_jobs(limit).await {
        Ok(jobs) => {
            let total = jobs.len();
            (StatusCode::OK, Json(DeadJobsResponse { jobs, total }))
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DeadJobsResponse { jobs: vec![], total: 0 }),
        ),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeadJobsQuery {
    pub limit: Option<i64>,
}

pub async fn retry_dead_job(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        );
    }

    match state.db.retry_dead_job(&job_id).await {
        Ok(()) => {
            tracing::info!(
                job_id = %job_id,
                admin = %auth_user.client_id,
                "Dead job queued for retry"
            );
            (
                StatusCode::OK,
                Json(json!({"success": true, "job_id": job_id})),
            )
        }
        Err(e) => {
            let status = match e {
                DbError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({"error": sanitize_db_error(&e)})),
            )
        }
    }
}

// ============================================================================
// PULL REQUEST MERGES EVIDENCE ENDPOINT
// ============================================================================

pub async fn list_pr_merges(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PrMergeEvidenceQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let limit = if query.limit == 0 { 50 } else { query.limit.min(500) } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_pr_merge_evidence(
            auth_user.org_id.as_deref(),
            query.org_name.as_deref(),
            query.repo_full_name.as_deref(),
            query.merged_by.as_deref(),
            limit,
            offset,
        )
        .await
    {
        Ok((entries, total)) => (
            StatusCode::OK,
            Json(PrMergeEvidenceResponse { entries, total }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

// ============================================================================
// ADMIN AUDIT LOG ENDPOINT
// ============================================================================

pub async fn list_admin_audit_log(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<AdminAuditLogQuery>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&auth_user) {
        return resp.into_response();
    }

    let limit = if query.limit == 0 { 50 } else { query.limit } as i64;
    let offset = query.offset as i64;

    match state
        .db
        .list_admin_audit_logs(
            query.actor.as_deref(),
            query.action.as_deref(),
            limit,
            offset,
        )
        .await
    {
        Ok((entries, total)) => (
            StatusCode::OK,
            Json(AdminAuditLogResponse { entries, total }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Internal database error" })),
        )
            .into_response(),
    }
}

