use serde_json::json;

/// Send a Slack-compatible webhook alert (fire-and-forget).
/// Errors are logged as warnings but never propagate — the caller must not await this in the hot path.
pub async fn send_alert(client: &reqwest::Client, webhook_url: &str, text: String) {
    let payload = json!({ "text": text });
    match client.post(webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!("Alert webhook delivered");
        }
        Ok(resp) => {
            tracing::warn!(
                status = %resp.status(),
                "Alert webhook returned non-success status"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to deliver alert webhook");
        }
    }
}

pub fn format_blocked_push_alert(actor: &str, repo: &str, branch: &str) -> String {
    format!(
        ":no_entry: *Blocked Push* — `{actor}` intentó hacer push a `{branch}` en `{repo}`. \
         El push fue bloqueado por política de gobernanza.",
        actor = actor,
        branch = branch,
        repo = repo
    )
}

pub fn format_signal_confirmed_alert(signal_type: &str, actor: &str, repo: Option<&str>) -> String {
    let repo_part = repo.map(|r| format!(" en `{r}`")).unwrap_or_default();
    format!(
        ":warning: *Signal Confirmada* — Tipo: `{signal_type}` | Actor: `{actor}`{repo_part}. \
         Revisar el dashboard de GitGov para más detalles.",
        signal_type = signal_type,
        actor = actor,
        repo_part = repo_part
    )
}
