fn project_state_summary(snapshot: &serde_json::Value) -> serde_json::Value {
    let blocked_today = snapshot
        .get("stats")
        .and_then(|s| s.get("client_events"))
        .and_then(|c| c.get("blocked_today"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let total_commits = snapshot
        .get("stats")
        .and_then(|s| s.get("total_commits"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let active_developers = snapshot
        .get("stats")
        .and_then(|s| s.get("active_developers"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let active_repos = snapshot
        .get("stats")
        .and_then(|s| s.get("active_repos"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let unresolved_violations = snapshot
        .get("stats")
        .and_then(|s| s.get("violations"))
        .and_then(|v| v.get("unresolved"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let dead_jobs = snapshot
        .get("job_metrics")
        .and_then(|m| m.get("dead"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    serde_json::json!({
        "blocked_pushes_today": blocked_today,
        "total_commits": total_commits,
        "active_developers": active_developers,
        "active_repos": active_repos,
        "unresolved_violations": unresolved_violations,
        "dead_jobs": dead_jobs
    })
}

fn build_project_knowledge_payload(question: &str) -> serde_json::Value {
    let ranked = rank_project_knowledge(question);
    let selected: Vec<serde_json::Value> = if ranked.is_empty() {
        let mut seed: Vec<serde_json::Value> = PROJECT_KNOWLEDGE_BASE
            .iter()
            .take(6)
            .map(|(title, _keywords, content)| {
                serde_json::json!({
                    "title": title,
                    "source": "project_docs_kb",
                    "content": content
                })
            })
            .collect();
        seed.extend(
            WEB_FAQ_KNOWLEDGE_BASE
                .iter()
                .take(6)
                .map(|(title, _keywords, content)| {
                    serde_json::json!({
                        "title": title,
                        "source": "web_docs_faq",
                        "content": content
                    })
                }),
        );
        seed
    } else {
        ranked
            .into_iter()
            .take(14)
            .map(|snippet| serde_json::json!({
                "title": snippet.title,
                "score": snippet.score,
                "source": snippet.source,
                "content": snippet.content
            }))
            .collect()
    };

    let now_utc = chrono::Utc::now();
    let lima_tz = chrono::FixedOffset::west_opt(5 * 3600)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("valid offset"));
    let now_lima = now_utc.with_timezone(&lima_tz);

    serde_json::json!({
        "mode": "project_knowledge",
        "runtime": {
            "now_utc_iso": now_utc.to_rfc3339(),
            "now_lima_iso": now_lima.to_rfc3339(),
            "weekday_lima_es": weekday_es(now_lima.weekday()),
            "timezone_hint": "America/Lima"
        },
        "capabilities": {
            "query_engine": [
                "control_plane_executive_summary",
                "online_developers_now",
                "commits_without_ticket_window",
                "pushes_no_ticket_main_7d",
                "blocked_pushes_this_month",
                "user_pushes_no_ticket_week",
                "user_pushes_count",
                "user_blocked_pushes_month",
                "user_commits_range",
                "user_commits_count",
                "session_commits_count",
                "total_commits_count"
            ],
            "integrations": [
                "github_webhooks",
                "jenkins_ingest",
                "jira_ingest",
                "jira_correlation",
                "github_actions_via_bridge"
            ],
            "auth": "authorization_bearer_required",
            "scoping": "api_key_role_and_org_scope_applies",
            "limits": "no_data_or_out_of_scope_must_not_invent"
        },
        "snippets": selected
    })
}

async fn refresh_project_snapshot_if_stale(
    state: &Arc<AppState>,
    session: &mut ConversationState,
    scoped_org_id: Option<&str>,
) -> Vec<String> {
    let now = chrono::Utc::now().timestamp_millis();
    if now - session.last_project_snapshot_ms < 30_000 {
        return Vec::new();
    }

    let mut refs = Vec::new();
    let stats = match state.db.get_stats(scoped_org_id).await {
        Ok(stats) => {
            refs.push("stats".to_string());
            serde_json::to_value(stats).unwrap_or_else(|_| serde_json::json!({}))
        }
        Err(e) => {
            tracing::warn!("refresh_project_snapshot_if_stale stats error: {}", e);
            serde_json::json!({})
        }
    };
    let job_metrics = match state.db.get_job_metrics().await {
        Ok(metrics) => {
            refs.push("jobs_metrics".to_string());
            serde_json::to_value(metrics).unwrap_or_else(|_| serde_json::json!({}))
        }
        Err(e) => {
            tracing::warn!("refresh_project_snapshot_if_stale job metrics error: {}", e);
            serde_json::json!({})
        }
    };

    session.project_snapshot = serde_json::json!({
        "captured_at_ms": now,
        "stats": stats,
        "job_metrics": job_metrics
    });
    session.last_project_snapshot_ms = now;
    refs
}

fn build_advanced_conversation_payload(
    question: &str,
    nlp: &NlpAnalysis,
    session: &ConversationState,
) -> serde_json::Value {
    let knowledge = build_project_knowledge_payload(question);
    let history: Vec<serde_json::Value> = session
        .turns
        .iter()
        .rev()
        .take(12)
        .cloned()
        .collect::<Vec<ConversationTurn>>()
        .into_iter()
        .rev()
        .map(|t| {
            serde_json::json!({
                "role": t.role,
                "text": t.text,
                "intent": t.intent,
                "timestamp_ms": t.timestamp_ms
            })
        })
        .collect();

    let todos: Vec<serde_json::Value> = session
        .todos
        .iter()
        .filter(|t| t.status == TodoStatus::Pending)
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "text": t.text,
                "priority": t.priority,
                "source": t.source
            })
        })
        .collect();

    let style_mode = if session.learning.negative_feedback > session.learning.positive_feedback {
        "high_precision"
    } else {
        "balanced"
    };

    serde_json::json!({
        "mode": "conversational_advanced",
        "question": question,
        "nlp": {
            "intent": nlp.intent.as_str(),
            "confidence": nlp.confidence,
            "entities": nlp.entities,
            "reasoning": nlp.reasoning
        },
        "conversation_state": {
            "slots": session.slots,
            "history": history,
            "pending_todos": todos,
            "learning": session.learning
        },
        "project_state_live": session.project_snapshot,
        "project_state_summary": project_state_summary(&session.project_snapshot),
        "knowledge_base": knowledge,
        "response_policy": {
            "priority_order": [
                "deterministic_sql_results",
                "todo_management",
                "guided_actionable_steps",
                "knowledge_based_answer",
                "insufficient_data_or_feature_not_available"
            ],
            "style": {
                "persona": "friendly_expert",
                "mode": style_mode,
                "must_be_clear": true,
                "must_include_next_steps_when_actionable": true,
                "respect_user_language": true
            }
        }
    })
}

fn greeting_answer(language: &str) -> String {
    if language == "en" {
        "Hi. I am GitGov Assistant. I can answer governance questions with real Control Plane data, guide integrations, and help with settings/onboarding.".to_string()
    } else {
        "Hola. Soy GitGov Assistant. Puedo responder preguntas de gobernanza con datos reales del Control Plane, guiar integraciones y ayudarte con settings/onboarding.".to_string()
    }
}

fn farewell_answer(language: &str) -> String {
    if language == "en" {
        "Done. If you want, I can leave a TODO list with your next governance actions.".to_string()
    } else {
        "Perfecto. Si quieres, te dejo una lista TODO con las próximas acciones de gobernanza.".to_string()
    }
}

fn finalize_chat_response(
    state: &Arc<AppState>,
    conversation_key: &str,
    session: &mut ConversationState,
    nlp: &NlpAnalysis,
    mut status_code: StatusCode,
    mut response: ChatAskResponse,
) -> (StatusCode, Json<ChatAskResponse>) {
    if status_code == StatusCode::INTERNAL_SERVER_ERROR {
        // Degrade gracefully on transient backend failures so chat UX does not look like an app crash.
        status_code = StatusCode::OK;
        if response.status == "error" {
            response.status = "insufficient_data".to_string();
        }
        if response.answer.trim().is_empty() {
            response.answer = if nlp.entities.language == "en" {
                "I could not complete that query due to temporary backend pressure. Please retry in a few seconds.".to_string()
            } else {
                "No pude completar esa consulta por presión temporal del backend. Reintenta en unos segundos.".to_string()
            };
        } else if nlp.entities.language == "en" {
            response.answer = format!(
                "{} Retry in a few seconds.",
                response.answer.trim()
            );
        } else {
            response.answer = format!(
                "{} Reintenta en unos segundos.",
                response.answer.trim()
            );
        }
    }

    response.answer = sanitize_chat_answer_text(&response.answer);
    update_learning(session, nlp.intent, &response.status);
    push_turn(session, "assistant", &response.answer, nlp.intent.as_str());
    save_conversation_state(state, conversation_key, session.clone());
    (status_code, Json(response))
}

async fn call_llm(
    http_client: &reqwest::Client,
    api_key: &str,
    model: &str,
    question: &str,
    data: &serde_json::Value,
) -> Result<ChatAskResponse, String> {
    let safe_prefix = |s: &str, max_chars: usize| -> String { s.chars().take(max_chars).collect() };

    let user_message = format!(
        "Pregunta: {}\n<data>{}</data>",
        question,
        serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string())
    );

    let req_body = GeminiRequest {
        system_instruction: GeminiSystemInstruction {
            parts: vec![GeminiPart {
                text: CHAT_SYSTEM_PROMPT.to_string(),
            }],
        },
        contents: vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart { text: user_message }],
        }],
        generation_config: GeminiGenerationConfig {
            temperature: 0.2,
            // Keep responses concise to reduce tail latency under chat bursts.
            max_output_tokens: 512,
            response_mime_type: "application/json".to_string(),
        },
    };

    let response = http_client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        ))
        .header("content-type", "application/json")
        .json(&req_body)
        .send()
        .await
        .map_err(|e| format!("LLM network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "LLM API returned {}: {}",
            status,
            safe_prefix(&body, 200)
        ));
    }

    let gemini_resp: GeminiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

    let text = gemini_resp
        .candidates
        .unwrap_or_default()
        .into_iter()
        .find_map(|c| c.content)
        .and_then(|content| content.parts)
        .unwrap_or_default()
        .into_iter()
        .find_map(|p| p.text)
        .ok_or_else(|| "LLM response had no text content".to_string())?;

    // Strip markdown code fences if present
    let json_str = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<ChatAskResponse>(json_str)
        .map_err(|e| format!("Failed to parse LLM JSON: {} — raw: {}", e, safe_prefix(json_str, 300)))
}

