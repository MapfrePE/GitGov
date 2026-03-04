// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub received: bool,
    pub delivery_id: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_github_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let delivery_id = headers
        .get("X-GitHub-Delivery")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&Uuid::new_v4().to_string())
        .to_string();

    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let signature = headers
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Validate HMAC signature if secret is configured
    if let Some(ref secret) = state.github_webhook_secret {
        if let Some(ref sig) = signature {
            if !validate_github_signature(secret, &body, sig) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(WebhookResponse {
                        received: false,
                        delivery_id: delivery_id.clone(),
                        event_type: event_type.clone(),
                        processed: Some(false),
                        error: Some("Invalid signature".to_string()),
                    }),
                );
            }
        } else {
            return (
                StatusCode::UNAUTHORIZED,
                Json(WebhookResponse {
                    received: false,
                    delivery_id: delivery_id.clone(),
                    event_type: event_type.clone(),
                    processed: Some(false),
                    error: Some("Missing signature".to_string()),
                }),
            );
        }
    }

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::warn!("Invalid JSON webhook payload: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse {
                    received: false,
                    delivery_id: delivery_id.clone(),
                    event_type: event_type.clone(),
                    processed: Some(false),
                    error: Some("Invalid JSON payload".to_string()),
                }),
            );
        }
    };

    // Store raw webhook event for debugging
    let webhook_id = match state.db.store_webhook_event(
        &delivery_id,
        &event_type,
        signature.as_deref(),
        &payload,
    ).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!("Failed to store webhook event: {}", e);
            None
        }
    };

    // Process the webhook based on event type
    let process_result = match event_type.as_str() {
        "push" => process_push_event(&state, &delivery_id, &payload).await,
        "create" => process_create_event(&state, &delivery_id, &payload).await,
        "pull_request" => process_pull_request_event(&state, &delivery_id, &payload).await,
        _ => {
            tracing::debug!("Unhandled event type: {}", event_type);
            Ok(())
        }
    };

    // Mark webhook as processed
    if let Some(ref id) = webhook_id {
        let error_msg = if process_result.is_err() {
            process_result.as_ref().err().map(|e| e.to_string())
        } else {
            None
        };
        let _ = state.db.mark_webhook_processed(id, error_msg.as_deref()).await;
    }

    match process_result {
        Ok(()) => (
            StatusCode::OK,
            Json(WebhookResponse {
                received: true,
                delivery_id,
                event_type,
                processed: Some(true),
                error: None,
            }),
        ),
        Err(e) if e.to_string().contains("duplicate") || e.to_string().contains("Duplicate") => {
            tracing::info!("Duplicate webhook received: delivery_id={}", delivery_id);
            (
                StatusCode::OK,
                Json(WebhookResponse {
                    received: true,
                    delivery_id,
                    event_type,
                    processed: Some(true),
                    error: Some("Duplicate delivery_id - already processed".to_string()),
                }),
            )
        }
        Err(_e) => (
            StatusCode::OK,
            Json(WebhookResponse {
                received: true,
                delivery_id,
                event_type,
                processed: Some(false),
                error: Some("Internal database error".to_string()),
            }),
        ),
    }
}

fn validate_github_signature(secret: &str, payload_bytes: &[u8], signature: &str) -> bool {
    let signature_hex = match signature.strip_prefix("sha256=") {
        Some(hex) => hex,
        None => return false,
    };
    let signature_bytes = match hex::decode(signature_hex) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let mut mac = match <hmac::Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(payload_bytes);
    mac.verify_slice(&signature_bytes).is_ok()
}

async fn process_push_event(
    state: &Arc<AppState>,
    delivery_id: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let push: PushEvent = serde_json::from_value(payload.clone())
        .map_err(|e| format!("Failed to parse push event: {}", e))?;

    // Extract org/repo info
    let (org_id, repo_id) = get_or_create_org_repo(&state.db, &push.repository).await?;

    // Extract commit SHAs
    let commit_shas: Vec<String> = push.commits.iter().map(|c| c.id.clone()).collect();
    let commits_count = commit_shas.len() as i32;

    // Determine ref type
    let ref_type = if push.r#ref.starts_with("refs/tags/") {
        "tag"
    } else {
        "branch"
    };

    let ref_name = push.r#ref
        .strip_prefix("refs/heads/")
        .or_else(|| push.r#ref.strip_prefix("refs/tags/"))
        .unwrap_or(&push.r#ref)
        .to_string();

    let actor_login = push.sender.login.clone();
    // Keep canonical type as "push" for compatibility with existing stats/signals SQL.
    let event_type = "push";

    if push.forced {
        tracing::warn!(
            actor = %actor_login,
            ref_name = %ref_name,
            repo = %push.repository.full_name,
            "Force push detected — history rewrite on branch"
        );
    }

    let event = GitHubEvent {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        event_type: event_type.to_string(),
        actor_login: Some(push.sender.login),
        actor_id: Some(push.sender.id),
        ref_name: Some(ref_name.clone()),
        ref_type: Some(ref_type.to_string()),
        before_sha: Some(push.before),
        after_sha: Some(push.after),
        commit_shas,
        commits_count,
        payload: payload.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    state.db.insert_github_event(&event).await
        .map_err(|e| {
            tracing::error!("Failed to insert github event: {}", e);
            "Internal database error".to_string()
        })?;

    tracing::info!(
        "Processed {} event: {} commits to {} by {}",
        event_type,
        event.commits_count,
        ref_name,
        actor_login
    );

    // Enqueue detection job instead of spawning directly (backpressure control)
    if let Some(ref org_id) = event.org_id {
        if let Err(e) = state.db.enqueue_job(org_id, "detect_signals", None).await {
            tracing::warn!("Failed to enqueue detection job for org {}: {}", org_id, e);
        }
    }

    Ok(())
}

async fn process_create_event(
    state: &Arc<AppState>,
    delivery_id: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let create: CreateEvent = serde_json::from_value(payload.clone())
        .map_err(|e| format!("Failed to parse create event: {}", e))?;

    // Extract org/repo info
    let (org_id, repo_id) = get_or_create_org_repo(&state.db, &create.repository).await?;

    let ref_name = create.r#ref.clone();
    let ref_type = create.ref_type.clone();
    let actor_login = create.sender.login.clone();

    let event = GitHubEvent {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        event_type: "create".to_string(),
        actor_login: Some(create.sender.login),
        actor_id: Some(create.sender.id),
        ref_name: Some(create.r#ref),
        ref_type: Some(create.ref_type),
        before_sha: None,
        after_sha: None,
        commit_shas: vec![],
        commits_count: 0,
        payload: payload.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    state.db.insert_github_event(&event).await
        .map_err(|e| format!("Failed to insert github event: {}", e))?;

    tracing::info!(
        "Processed create event: {} {} by {}",
        ref_type,
        ref_name,
        actor_login
    );

    Ok(())
}

#[derive(Debug, Deserialize)]
struct GitHubPrReviewUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPrReview {
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    user: Option<GitHubPrReviewUser>,
}

fn extract_final_approvers(reviews: &[GitHubPrReview]) -> Vec<String> {
    // GitHub reviews are evaluated per reviewer by latest review state.
    let mut latest_state_by_user: HashMap<String, String> = HashMap::new();

    for review in reviews {
        let Some(user) = review.user.as_ref() else { continue };
        let state = review
            .state
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_ascii_uppercase();
        if state.is_empty() {
            continue;
        }
        latest_state_by_user.insert(user.login.clone(), state);
    }

    let mut approvers: Vec<String> = latest_state_by_user
        .into_iter()
        .filter_map(|(login, state)| (state == "APPROVED").then_some(login))
        .collect();

    approvers.sort();
    approvers
}

async fn fetch_pr_approvers(
    http_client: &reqwest::Client,
    github_token: &str,
    repo_full_name: &str,
    pr_number: i32,
) -> Result<Vec<String>, String> {
    let mut all_reviews = Vec::new();
    let mut page = 1u8;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/pulls/{}/reviews?per_page=100&page={}",
            repo_full_name, pr_number, page
        );

        let response = http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", github_token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "gitgov-server")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("GitHub reviews request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("GitHub reviews API returned {}", status));
        }

        let reviews: Vec<GitHubPrReview> = response
            .json()
            .await
            .map_err(|e| format!("GitHub reviews decode failed: {}", e))?;

        let chunk_len = reviews.len();
        all_reviews.extend(reviews);

        if chunk_len < 100 || page >= 10 {
            break;
        }

        page += 1;
    }

    Ok(extract_final_approvers(&all_reviews))
}

// Processes pull_request webhook events.
// Only stores merged PRs (action == "closed" && pull_request.merged == true).
// All other actions (opened, reviewed, etc.) are silently skipped — no error.
async fn process_pull_request_event(
    state: &Arc<AppState>,
    delivery_id: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let pr = match payload.get("pull_request") {
        Some(pr) => pr,
        None => {
            tracing::debug!("pull_request event missing 'pull_request' field, delivery_id={}", delivery_id);
            return Ok(());
        }
    };

    // Only capture merged PRs
    let merged = pr.get("merged").and_then(|v| v.as_bool()).unwrap_or(false);
    if action != "closed" || !merged {
        tracing::debug!("Skipping non-merged pull_request event: action={}, delivery_id={}", action, delivery_id);
        return Ok(());
    }

    // Extract repository info for org/repo lookup
    let repo_val = match payload.get("repository") {
        Some(r) => r,
        None => {
            tracing::warn!("pull_request event missing 'repository' field, delivery_id={}", delivery_id);
            return Ok(());
        }
    };
    let repo: GitHubRepository = match serde_json::from_value(repo_val.clone()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse repository in pull_request event: {}, delivery_id={}", e, delivery_id);
            return Ok(());
        }
    };

    let (org_id, repo_id) = get_or_create_org_repo(&state.db, &repo).await?;

    let pr_number = pr.get("number").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let pr_title = pr.get("title").and_then(|v| v.as_str()).map(String::from);
    let author_login = pr.get("user").and_then(|u| u.get("login")).and_then(|v| v.as_str()).map(String::from);
    let merged_by_login = pr.get("merged_by").and_then(|u| u.get("login")).and_then(|v| v.as_str()).map(String::from);
    let head_sha = pr.get("head").and_then(|h| h.get("sha")).and_then(|v| v.as_str()).map(String::from);
    let base_branch = pr.get("base").and_then(|b| b.get("ref")).and_then(|v| v.as_str()).map(String::from);
    let approvers = match state.github_personal_access_token.as_deref() {
        Some(token) => match fetch_pr_approvers(&state.http_client, token, &repo.full_name, pr_number).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    delivery_id = %delivery_id,
                    repo = %repo.full_name,
                    pr_number,
                    error = %e,
                    "Failed to fetch PR approvers from GitHub API"
                );
                vec![]
            }
        },
        None => {
            tracing::debug!(
                delivery_id = %delivery_id,
                repo = %repo.full_name,
                pr_number,
                "GITHUB_PERSONAL_ACCESS_TOKEN not configured; storing PR merge without approvers"
            );
            vec![]
        }
    };
    let approvals_count = approvers.len() as i32;

    let mut enriched_payload = payload.clone();
    if let Some(obj) = enriched_payload.as_object_mut() {
        obj.insert(
            "gitgov".to_string(),
            serde_json::json!({
                "approvers": approvers,
                "approvals_count": approvals_count
            }),
        );
    }

    let record = PrMergeRecord {
        id: Uuid::new_v4().to_string(),
        org_id: Some(org_id),
        repo_id: Some(repo_id),
        delivery_id: delivery_id.to_string(),
        pr_number,
        pr_title: pr_title.clone(),
        author_login: author_login.clone(),
        merged_by_login: merged_by_login.clone(),
        head_sha,
        base_branch,
        payload: enriched_payload,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    match state.db.insert_pr_merge(&record).await {
        Ok(()) => {
            tracing::info!(
                "Processed PR merge: #{} '{}' by {} merged by {} (approvals={}), delivery_id={}",
                pr_number,
                pr_title.as_deref().unwrap_or(""),
                author_login.as_deref().unwrap_or("unknown"),
                merged_by_login.as_deref().unwrap_or("unknown"),
                approvals_count,
                delivery_id,
            );
            Ok(())
        }
        Err(DbError::Duplicate(_)) => {
            tracing::debug!("Duplicate PR merge event ignored: delivery_id={}", delivery_id);
            Ok(())
        }
        Err(e) => Err(format!("Failed to insert PR merge: {}", e)),
    }
}

async fn get_or_create_org_repo(db: &Database, repo: &GitHubRepository) -> Result<(String, String), String> {
    // Get or create org
    let org_id = if let Some(ref org) = repo.organization {
        db.upsert_org(org.id, &org.login, None, None).await
            .map_err(|e| e.to_string())?
    } else {
        // If no organization, use the owner as org
        db.upsert_org(repo.owner.id, &repo.owner.login, None, None).await
            .map_err(|e| e.to_string())?
    };

    // Get or create repo
    let repo_id = db.upsert_repo(
        Some(&org_id),
        repo.id,
        &repo.full_name,
        &repo.name,
        repo.private,
    ).await.map_err(|e| e.to_string())?;

    Ok((org_id, repo_id))
}

