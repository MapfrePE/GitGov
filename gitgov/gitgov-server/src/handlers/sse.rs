// ============================================================================
// SSE (Server-Sent Events) — Real-time dashboard notifications
// ============================================================================

use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;

/// SSE endpoint: streams lightweight notifications to connected dashboard clients.
/// Requires Bearer auth. Sends heartbeat every 30s to keep connection alive.
/// Guarded by a server-wide semaphore (`sse_max_connections`, default 50).
///
/// Event types:
///   - `new_events` — new events ingested via POST /events (client should refresh logs + stats)
///   - `heartbeat`  — keep-alive (no action needed)
pub async fn sse_stream(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Acquire a connection permit from the semaphore.
    let permit = match state.sse_max_connections.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            tracing::warn!(
                client_id = %auth_user.client_id,
                "SSE connection rejected: max concurrent connections reached"
            );
            // Return 503 with a JSON error — compatible with existing error shape.
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    r#"{"error":"Too many SSE connections. Try again later."}"#,
                ))
                .unwrap()
                .into_response();
        }
    };

    let mut rx = state.sse_tx.subscribe();
    let client_id = auth_user.client_id.clone();

    tracing::info!(
        client_id = %client_id,
        "SSE client connected"
    );

    metrics::gauge!("gitgov_sse_connections_active").increment(1.0);

    let stream = async_stream::stream! {
        // Hold the permit for the lifetime of the stream — it is released
        // automatically when _permit is dropped (i.e. when the client disconnects).
        let _permit = permit;

        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
                Ok(Ok(notification)) => {
                    match serde_json::to_string(&notification) {
                        Ok(data) => {
                            let event: Result<Event, Infallible> = Ok(Event::default().data(data));
                            yield event;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to serialize SSE notification");
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    tracing::debug!(
                        client_id = %client_id,
                        skipped = n,
                        "SSE client lagged, skipping messages"
                    );
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    tracing::info!(client_id = %client_id, "SSE broadcast channel closed");
                    break;
                }
                Err(_timeout) => {
                    // No message in 30s — send heartbeat
                    let hb = SseNotification::Heartbeat;
                    if let Ok(data) = serde_json::to_string(&hb) {
                        let event: Result<Event, Infallible> = Ok(Event::default().data(data));
                        yield event;
                    }
                }
            }
        }

        metrics::gauge!("gitgov_sse_connections_active").decrement(1.0);
    };

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}
