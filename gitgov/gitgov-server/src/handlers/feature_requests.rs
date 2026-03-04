// ============================================================================
// FEATURE REQUESTS — POST /feature-requests
// ============================================================================

pub async fn create_feature_request_handler(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FeatureRequestInput>,
) -> impl IntoResponse {
    let question = payload.question.trim().to_string();
    if question.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "question is required" })),
        );
    }

    let requested_by = auth_user.client_id.as_str();

    let effective_org_id = if let Some(scoped_org_id) = auth_user.org_id.as_deref() {
        if let Some(ref requested_org_id) = payload.org_id {
            if requested_org_id != scoped_org_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "error": "org_id is outside API key scope" })),
                );
            }
        }
        Some(scoped_org_id.to_string())
    } else {
        payload.org_id.clone()
    };

    let sanitized_payload = FeatureRequestInput {
        question: payload.question.clone(),
        missing_capability: payload.missing_capability.clone(),
        org_id: effective_org_id,
        user_login: None,
        metadata: payload.metadata.clone(),
    };

    let id = match state.db.create_feature_request(&sanitized_payload, requested_by).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("create_feature_request db error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to save feature request" })),
            );
        }
    };

    // Optional webhook notification
    if let Some(ref webhook_url) = state.feature_request_webhook_url {
        let body = serde_json::json!({
            "id": &id,
            "requested_by": requested_by,
            "question": &question,
            "missing_capability": &sanitized_payload.missing_capability,
            "org_id": &sanitized_payload.org_id,
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });
        let client = state.http_client.clone();
        let url = webhook_url.clone();
        tokio::spawn(async move {
            if let Err(e) = client.post(&url).json(&body).send().await {
                tracing::warn!("feature_request webhook failed: {}", e);
            }
        });
    }

    tracing::info!(id = %id, requested_by = %requested_by, "Feature request created");
    (StatusCode::CREATED, Json(serde_json::json!({ "id": id, "status": "new" })))
}

