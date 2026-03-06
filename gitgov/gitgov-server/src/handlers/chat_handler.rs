fn query_needs_explicit_org_scope(query: &ChatQuery) -> bool {
    matches!(
        query,
        ChatQuery::ControlPlaneExecutiveSummary
            | ChatQuery::OnlineDevelopersNow { .. }
            | ChatQuery::CommitsWithoutTicketWindow { .. }
            | ChatQuery::PushesNoTicket
            | ChatQuery::BlockedPushesMonth
            | ChatQuery::UserPushesCount { .. }
            | ChatQuery::UserActivityMonth { .. }
            | ChatQuery::UserPushesNoTicketWeek { .. }
            | ChatQuery::UserBlockedPushesMonth { .. }
            | ChatQuery::SessionCommitsCount { .. }
            | ChatQuery::TotalCommitsCount
            | ChatQuery::UserCommitsCount { .. }
            | ChatQuery::UserLastCommit { .. }
            | ChatQuery::UserCommitsRange { .. }
            | ChatQuery::UserAccessProfile { .. }
    )
}

fn is_founder_scope_exception(auth_user: &AuthUser) -> bool {
    auth_user.role == UserRole::Admin
        && auth_user.org_id.is_none()
        && auth_user.client_id.eq_ignore_ascii_case("bootstrap-admin")
}

fn looks_generic_non_answer(text: &str) -> bool {
    let t = text.to_lowercase();
    let markers = [
        "puedo guiarte paso a paso",
        "opciones frecuentes",
        "información detallada",
        "la información detallada",
        "i can guide you step by step",
        "common options",
        "detailed information is available",
    ];
    markers.iter().any(|m| t.contains(m))
}

fn is_logs_precision_query(question: &str) -> bool {
    let q = question.to_lowercase();
    let has_logs_word = q
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| matches!(w, "log" | "logs" | "evento" | "eventos" | "event" | "events" | "historial"));
    has_logs_word
        || q.contains("actividad reciente")
        || q.contains("ultimos eventos")
        || q.contains("ultimos logs")
        || q.contains("recent activity")
        || q.contains("recent logs")
        || q.contains("latest logs")
}

fn extract_logs_limit(question: &str, default_limit: usize, max_limit: usize) -> usize {
    for token in question.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() {
            continue;
        }
        if let Ok(value) = token.parse::<usize>() {
            if value > 0 {
                return value.min(max_limit);
            }
        }
    }
    default_limit.min(max_limit).max(1)
}

fn extract_logs_event_type_hint(question: &str) -> Option<String> {
    let q = question.to_lowercase();
    if q.contains("blocked_push") || q.contains("push bloque") {
        return Some("blocked_push".to_string());
    }
    if q.contains("successful_push") || q.contains("push exitos") {
        return Some("successful_push".to_string());
    }
    if q.contains("attempt_push") || q.contains("intento de push") {
        return Some("attempt_push".to_string());
    }
    if q.contains("stage_files") || q.contains("staged") || q.contains("staging") {
        return Some("stage_files".to_string());
    }
    if q.contains("commit") {
        return Some("commit".to_string());
    }
    None
}

fn render_precise_logs_answer(events: &[CombinedEvent], language: &str) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(events.len());
    for event in events {
        let user = event.user_login.as_deref().unwrap_or("n/a");
        let repo = event.repo_name.as_deref().unwrap_or("n/a");
        let branch = event.branch.as_deref().unwrap_or("n/a");
        let status = event.status.as_deref().unwrap_or("n/a");
        let ts_label =
            if let Some(dt_utc) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(event.created_at) {
                let lima_tz = chrono::FixedOffset::west_opt(5 * 3600)
                    .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("valid offset"));
                let dt_lima = dt_utc.with_timezone(&lima_tz);
                format!(
                    "{} Lima | {} UTC | {}ms",
                    dt_lima.format("%Y-%m-%d %H:%M:%S"),
                    dt_utc.format("%Y-%m-%d %H:%M:%S"),
                    event.created_at
                )
            } else {
                format!("{}ms", event.created_at)
            };

        lines.push(format!(
            "- {} | source={} type={} user={} repo={} branch={} status={} id={}",
            ts_label, event.source, event.event_type, user, repo, branch, status, event.id
        ));
    }

    if language == "en" {
        format!(
            "Exact log sample ({} events, deterministic DB query):\n{}",
            events.len(),
            lines.join("\n")
        )
    } else {
        format!(
            "Muestra exacta de logs ({} eventos, consulta deterministica DB):\n{}",
            events.len(),
            lines.join("\n")
        )
    }
}

fn normalize_llm_response(
    mut response: ChatAskResponse,
    question: &str,
    language: &str,
) -> ChatAskResponse {
    let normalized_status = match response.status.as_str() {
        "ok" | "insufficient_data" | "feature_not_available" | "error" => response.status.clone(),
        _ => "error".to_string(),
    };
    response.status = normalized_status;

    response.answer = response.answer.trim().to_string();
    if response.answer.is_empty() {
        response.status = "error".to_string();
        response.answer = if language == "en" {
            "The model returned an empty response. Please try again.".to_string()
        } else {
            "El modelo devolvió una respuesta vacía. Intenta de nuevo.".to_string()
        };
    }

    if response.status == "error" {
        response.status = "insufficient_data".to_string();
        response.answer = if language == "en" {
            "I could not provide a verified answer for that exact request. I can still help with GitGov analytics, integrations, onboarding, and troubleshooting.".to_string()
        } else {
            "No pude dar una respuesta verificable para esa consulta exacta. Sí puedo ayudarte con analítica de GitGov, integraciones, onboarding y troubleshooting.".to_string()
        };
        response.missing_capability = None;
        response.can_report_feature = false;
    }

    if is_secret_exfiltration_request(question) {
        response.status = "ok".to_string();
        response.answer = "No puedo revelar API keys, tokens ni secretos de ningún usuario. Por seguridad solo puedo ayudarte con estado de acceso (rol, miembro activo/inactivo, y si existe key activa) sin mostrar valores sensibles.".to_string();
        response.missing_capability = None;
        response.can_report_feature = false;
        response.data_refs = vec!["security_policy".to_string()];
        return response;
    }

    if response.status == "ok" && looks_generic_non_answer(&response.answer) {
        response.status = "insufficient_data".to_string();
        response.answer = if language == "en" {
            "I need a more specific question or verifiable data to answer precisely. If your question is about metrics, include user/org/time window.".to_string()
        } else {
            "Necesito una pregunta más específica o datos verificables para responder con precisión. Si es una métrica, incluye usuario/org/ventana de tiempo.".to_string()
        };
    }

    if response.status == "insufficient_data" {
        let lower = response.answer.to_lowercase();
        let has_reason = [
            "falt",
            "insuf",
            "scope",
            "org",
            "dato",
            "context",
            "missing",
            "not enough",
        ]
        .iter()
        .any(|m| lower.contains(m));
        if !has_reason {
            response.answer = if language == "en" {
                "I can't answer that with the current scope/data. Please provide user/org/time window or ask a question covered by available project data.".to_string()
            } else {
                "No puedo responder eso con el scope/datos actuales. Indica usuario/org/ventana de tiempo o formula una pregunta cubierta por los datos disponibles.".to_string()
            };
        }
    }

    if response.status == "feature_not_available" {
        response.can_report_feature = true;
        if response.missing_capability.is_none() {
            response.missing_capability = Some("capability_not_available".to_string());
        }
    } else {
        response.can_report_feature = false;
        response.missing_capability = None;
    }

    response.data_refs = response
        .data_refs
        .into_iter()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty() && r.len() <= 80)
        .take(12)
        .collect();
    response.data_refs.sort();
    response.data_refs.dedup();

    response
}

fn llm_degraded_answer(question: &str, language: &str) -> String {
    if let Some(answer) = build_knowledge_fallback_answer(question, language) {
        return answer;
    }

    let mut answer = build_guided_help_answer(question);
    if language == "en" {
        answer.push_str(
            "\n\nI could not use the language model for this turn, so I answered with local project context.",
        );
    } else {
        answer.push_str(
            "\n\nNo pude usar el modelo en este turno, así que respondí con contexto local del proyecto.",
        );
    }
    answer
}

async fn user_exists_in_scope(
    state: &Arc<AppState>,
    user: &str,
    scoped_org_id: Option<&str>,
) -> Result<bool, DbError> {
    Ok(state
        .db
        .chat_query_user_access_profile(user, scoped_org_id)
        .await?
        .is_some())
}

pub async fn chat_ask(
    Extension(auth_user): Extension<AuthUser>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatAskRequest>,
) -> impl IntoResponse {
    if require_admin(&auth_user).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(ChatAskResponse {
                status: "error".to_string(),
                answer: "Admin access required".to_string(),
                missing_capability: None,
                can_report_feature: false,
                data_refs: vec![],
            }),
        );
    }

    let question = payload.question.trim().to_string();
    if question.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ChatAskResponse {
                status: "error".to_string(),
                answer: "La pregunta no puede estar vacía".to_string(),
                missing_capability: None,
                can_report_feature: false,
                data_refs: vec![],
            }),
        );
    }

    let org_name = payload.org_name.as_deref();
    let scoped_org_id = match resolve_and_check_org_scope(
        &state,
        auth_user.org_id.as_deref(),
        org_name,
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
                Json(ChatAskResponse {
                    status: "error".to_string(),
                    answer: error.to_string(),
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec![],
                }),
            );
        }
    };

    let conversation_key = build_conversation_key(&auth_user, scoped_org_id.as_deref());
    let mut session = load_conversation_state(&state, &conversation_key);
    ensure_session_initialized(&mut session);
    let nlp = analyze_nlp(&question, &session);
    let safe_question_for_state = sanitize_chat_answer_text(&question);
    push_turn(
        &mut session,
        "user",
        &safe_question_for_state,
        nlp.intent.as_str(),
    );
    update_slots_from_nlp(&mut session, &nlp, org_name);

    let mut snapshot_refs = refresh_project_snapshot_if_stale(&state, &mut session, scoped_org_id.as_deref()).await;
    let proactive_todos = apply_proactive_todos_from_snapshot(&mut session);

    match nlp.intent {
        NlpIntent::TodoAdd => {
            let text = nlp
                .entities
                .todo_text
                .clone()
                .unwrap_or_else(|| "Tarea pendiente sin descripción".to_string());
            let task = add_todo(&mut session, &text, "user_request", "medium");
            let answer = format!(
                "Listo. Registré la tarea #{}: {}. Puedes pedirme \"mis tareas\" para ver pendientes.",
                task.id, task.text
            );
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string(), "todo_runtime".to_string()],
                },
            );
        }
        NlpIntent::TodoList => {
            let answer = render_todo_list(&session, &nlp.entities.language);
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string(), "todo_runtime".to_string()],
                },
            );
        }
        NlpIntent::TodoComplete => {
            let response = if let Some(todo_id) = nlp.entities.todo_id {
                if let Some(task) = complete_todo(&mut session, todo_id) {
                    ChatAskResponse {
                        status: "ok".to_string(),
                        answer: format!("Tarea #{} completada: {}", task.id, task.text),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec!["assistant_runtime".to_string(), "todo_runtime".to_string()],
                    }
                } else {
                    ChatAskResponse {
                        status: "insufficient_data".to_string(),
                        answer: format!(
                            "No encontré una tarea pendiente con id #{}. Usa \"mis tareas\" para ver IDs válidos.",
                            todo_id
                        ),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec!["todo_runtime".to_string()],
                    }
                }
            } else {
                ChatAskResponse {
                    status: "insufficient_data".to_string(),
                    answer: "Indica el id de la tarea a completar. Ejemplo: \"completa tarea 3\".".to_string(),
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["todo_runtime".to_string()],
                }
            };
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,StatusCode::OK, response);
        }
        NlpIntent::FeedbackPositive => {
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer: if nlp.entities.language == "en" {
                        "Great. I will keep this response style for the next interactions.".to_string()
                    } else {
                        "Perfecto. Mantendré este estilo de respuesta en las próximas interacciones.".to_string()
                    },
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string()],
                },
            );
        }
        NlpIntent::FeedbackNegative => {
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer: if nlp.entities.language == "en" {
                        "Understood. I will answer with more precision and concrete steps from now on.".to_string()
                    } else {
                        "Entendido. Voy a responder con más precisión y pasos concretos desde ahora.".to_string()
                    },
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string()],
                },
            );
        }
        NlpIntent::Farewell => {
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer: farewell_answer(&nlp.entities.language),
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string()],
                },
            );
        }
        _ => {}
    }

    if is_secret_exfiltration_request(&question) {
        return finalize_chat_response(
            &state,
            &conversation_key,
            &mut session,
            &nlp,
            StatusCode::OK,
            ChatAskResponse {
                status: "ok".to_string(),
                answer: "No puedo revelar API keys, tokens ni secretos de ningún usuario. Por seguridad solo puedo ayudarte con estado de acceso (rol, miembro activo/inactivo, y si existe key activa) sin mostrar valores sensibles.".to_string(),
                missing_capability: None,
                can_report_feature: false,
                data_refs: vec!["security_policy".to_string()],
            },
        );
    }

    let mut query = detect_query(&question);
    if let Some(ChatQuery::NeedUserForCommitHistory) = query {
        if let Some(ref remembered_user) = session.slots.last_user_login {
            query = Some(ChatQuery::UserCommitsCount {
                user: remembered_user.clone(),
                start_ms: None,
                end_ms: None,
            });
        }
    }
    if query.is_none() {
        let q = question.to_lowercase();
        if (q.contains("todo el historial") || q.contains("all history"))
            && !q.contains("commit")
            && !q.contains("commits")
        {
            if let Some(ref remembered_user) = session.slots.last_user_login {
                query = Some(ChatQuery::UserCommitsCount {
                    user: remembered_user.clone(),
                    start_ms: None,
                    end_ms: None,
                });
            }
        }
        if let Some(ref remembered_user) = session.slots.last_user_login {
            if (q.contains("rol") || q.contains("role") || q.contains("api key") || q.contains("apikey"))
                && !q.contains("usuario")
                && !q.contains("user ")
            {
                query = Some(ChatQuery::UserAccessProfile {
                    user: remembered_user.clone(),
                });
            } else if (q.contains("bloqueado") || q.contains("blocked"))
                && (q.contains("push") || q.contains("pushes"))
                && !q.contains("usuario")
                && !q.contains("user ")
            {
                query = Some(ChatQuery::UserBlockedPushesMonth {
                    user: remembered_user.clone(),
                });
            } else if (q.contains("push") || q.contains("pushes"))
                && (q.contains("ticket") || q.contains("jira") || q.contains("sin ticket") || q.contains("without ticket"))
                && !q.contains("usuario")
                && !q.contains("user ")
            {
                query = Some(ChatQuery::UserPushesNoTicketWeek {
                    user: remembered_user.clone(),
                });
            } else if (q.contains("push") || q.contains("pushes"))
                && (q.contains("cuanto") || q.contains("cuánto") || q.contains("how many") || q.contains("total"))
                && !q.contains("bloqueado")
                && !q.contains("blocked")
                && !q.contains("ticket")
                && !q.contains("jira")
                && !q.contains("usuario")
                && !q.contains("user ")
            {
                query = Some(ChatQuery::UserPushesCount {
                    user: remembered_user.clone(),
                    start_ms: None,
                    end_ms: None,
                });
            }
        }
    }

    let founder_scope_exception = is_founder_scope_exception(&auth_user);
    if auth_user.org_id.is_none() && scoped_org_id.is_none() && !founder_scope_exception {
        if let Some(ref q) = query {
            if query_needs_explicit_org_scope(q) {
                return finalize_chat_response(
                    &state,
                    &conversation_key,
                    &mut session,
                    &nlp,
                    StatusCode::OK,
                    ChatAskResponse {
                        status: "insufficient_data".to_string(),
                        answer: if nlp.entities.language == "en" {
                            "This query needs an organization scope. Select or provide `org_name` first to avoid cross-org ambiguity.".to_string()
                        } else {
                            "Esta consulta requiere un scope de organización. Selecciona o envía `org_name` primero para evitar ambigüedad entre organizaciones.".to_string()
                        },
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec!["org_scope".to_string()],
                    },
                );
            }
        }
    }

    match query {
        Some(ChatQuery::Greeting) => {
            let mut answer = greeting_answer(&nlp.entities.language);
            if !proactive_todos.is_empty() {
                answer.push_str("\n\nSugerencias proactivas registradas en TODO:\n");
                answer.push_str(&proactive_todos.join("\n"));
            }
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec![
                        "assistant_runtime".to_string(),
                        "project_docs_kb".to_string(),
                        "todo_runtime".to_string(),
                    ],
                },
            );
        }
        Some(ChatQuery::DateMismatchClarification) => {
            let now_utc = chrono::Utc::now();
            let lima_tz = chrono::FixedOffset::west_opt(5 * 3600)
                .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("valid offset"));
            let now_lima = now_utc.with_timezone(&lima_tz);
            let answer = format!(
                "Buena alerta. Si una fecha parece \"adelantada\" (por ejemplo 04 vs 03), normalmente es por zona horaria (UTC vs America/Lima) o por una respuesta no determinística del LLM. Hora actual: {} (America/Lima) | {} UTC. Si quieres, te doy el dato exacto consultando el evento en base de datos con UTC y hora local.",
                now_lima.format("%Y-%m-%d %H:%M:%S"),
                now_utc.format("%Y-%m-%d %H:%M:%S")
            );
            return finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string(), "client_events".to_string()],
                },
            );
        }
        Some(ChatQuery::CurrentDateTime) => {
            let now_utc = chrono::Utc::now();
            let lima_tz = chrono::FixedOffset::west_opt(5 * 3600)
                .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("valid offset"));
            let now_lima = now_utc.with_timezone(&lima_tz);
            let answer = format!(
                "Fecha y hora actuales: {} (America/Lima, {}) | UTC: {}.",
                now_lima.format("%Y-%m-%d %H:%M:%S"),
                weekday_es(now_lima.weekday()),
                now_utc.format("%Y-%m-%d %H:%M:%S")
            );
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string()],
                },
            );
        }
        Some(ChatQuery::CapabilityOverview) => {
            let mut answer = "Sí. Puedo consultar datos reales del Control Plane con el scope de tu API key. Hoy tengo consultas en tiempo real para: resumen ejecutivo del control plane, devs online recientes, commits sin ticket (ventana), pushes por usuario (exitosos), pushes sin ticket (global y por usuario), pushes bloqueados del mes (global y por usuario), commits por usuario en rango y conteos de commits. También puedo consultar perfil de acceso de usuario (rol/estado y si tiene key activa), sin exponer secretos.".to_string();
            if !proactive_todos.is_empty() {
                answer.push_str("\n\nTambién detecté acciones sugeridas y las añadí a TODO.");
            }
            let mut refs = vec![
                "client_events".to_string(),
                "github_events".to_string(),
                "project_docs_kb".to_string(),
            ];
            refs.extend(snapshot_refs.clone());
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: refs,
                },
            );
        }
        Some(ChatQuery::ControlPlaneExecutiveSummary) => {
            let stats = match state.db.get_stats(scoped_org_id.as_deref()).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("get_stats(executive_summary) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando estadísticas del Control Plane".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };
            let now_ms = chrono::Utc::now().timestamp_millis();
            let start_7d = now_ms - 7 * 24 * 60 * 60 * 1000;
            let commits_7d = match state
                .db
                .chat_query_commits_count(Some(start_7d), Some(now_ms), scoped_org_id.as_deref())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_commits_count(7d) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando commits del Control Plane".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };
            let online_devs = match state
                .db
                .chat_query_online_developers_count(scoped_org_id.as_deref(), 15)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_online_developers_count error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando sesiones activas de developers".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };
            let pushes_no_ticket_7d = match state
                .db
                .chat_query_pushes_no_ticket_count(scoped_org_id.as_deref())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_pushes_no_ticket_count error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando pushes sin ticket".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };
            let commits_no_ticket_7d = match state
                .db
                .chat_query_commits_without_ticket_count(scoped_org_id.as_deref(), 24 * 7)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_commits_without_ticket_count(7d) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando commits sin ticket".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };
            let blocked_month = match state
                .db
                .chat_query_blocked_pushes_month(scoped_org_id.as_deref())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_blocked_pushes_month(executive) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando pushes bloqueados".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };
            let commits_with_ticket_7d = (commits_7d - commits_no_ticket_7d).max(0);
            let coverage_7d = if commits_7d > 0 {
                (commits_with_ticket_7d as f64 / commits_7d as f64) * 100.0
            } else {
                0.0
            };
            let scope_hint = if scoped_org_id.is_some() {
                "scope org activo"
            } else if auth_user.client_id.eq_ignore_ascii_case("bootstrap-admin") {
                "scope founder/global"
            } else {
                "scope global"
            };
            let now_utc = chrono::Utc::now();
            let lima_tz = chrono::FixedOffset::west_opt(5 * 3600)
                .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("valid offset"));
            let now_lima = now_utc.with_timezone(&lima_tz);
            let answer = format!(
                "Resumen ejecutivo Control Plane ({scope_hint})\n\
Devs ON (últimos 15 min): {online_devs}\n\
Devs activos 7d: {active_devs_week}\n\
Repos activos: {active_repos}\n\
Commits 7d: {commits_7d}\n\
Commits sin ticket 7d: {commits_no_ticket_7d}\n\
Cobertura ticket commits 7d: {coverage_7d:.1}%\n\
Pushes a main sin ticket 7d: {pushes_no_ticket_7d}\n\
Pushes bloqueados (mes actual): {blocked_month}\n\
Violaciones sin resolver: {violations_unresolved}\n\
Corte temporal: {lima} (America/Lima) | {utc} UTC.",
                scope_hint = scope_hint,
                online_devs = online_devs,
                active_devs_week = stats.active_devs_week,
                active_repos = stats.active_repos,
                commits_7d = commits_7d,
                commits_no_ticket_7d = commits_no_ticket_7d,
                coverage_7d = coverage_7d,
                pushes_no_ticket_7d = pushes_no_ticket_7d,
                blocked_month = blocked_month,
                violations_unresolved = stats.violations.unresolved,
                lima = now_lima.format("%Y-%m-%d %H:%M:%S"),
                utc = now_utc.format("%Y-%m-%d %H:%M:%S")
            );
            return finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec![
                        "stats".to_string(),
                        "client_sessions".to_string(),
                        "client_events".to_string(),
                        "github_events".to_string(),
                        "commit_ticket_correlations".to_string(),
                        "assistant_runtime".to_string(),
                    ],
                },
            );
        }
        Some(ChatQuery::OnlineDevelopersNow { minutes }) => {
            match state
                .db
                .chat_query_online_developers_count(scoped_org_id.as_deref(), minutes)
                .await
            {
                Ok(count) => {
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer: format!(
                                "Developers ON detectados: {} (ventana de actividad: últimos {} minutos).",
                                count, minutes
                            ),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_sessions".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_online_developers_count(single) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando developers online".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::CommitsWithoutTicketWindow { hours }) => {
            match state
                .db
                .chat_query_commits_without_ticket_count(scoped_org_id.as_deref(), hours)
                .await
            {
                Ok(count) => {
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer: format!(
                                "Commits sin ticket detectados: {} en la ventana de {} horas.",
                                count, hours
                            ),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![
                                "client_events".to_string(),
                                "commit_ticket_correlations".to_string(),
                            ],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_commits_without_ticket_count error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando commits sin ticket".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::NeedUserForCommitHistory) => {
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "insufficient_data".to_string(),
                    answer: "Para contar commits en todo el historial necesito un usuario. Ejemplo: \"¿Cuántos commits hizo el usuario MapfrePE en todo el historial?\"".to_string(),
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string()],
                },
            );
        }
        Some(ChatQuery::GuidedHelp) => {
            let mut answer = build_guided_help_answer(&question);
            if !proactive_todos.is_empty() {
                answer.push_str("\n\nAcciones proactivas añadidas a TODO:\n");
                answer.push_str(&proactive_todos.join("\n"));
            }
            return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec![
                        "project_docs_kb".to_string(),
                        "web_docs_faq".to_string(),
                        "todo_runtime".to_string(),
                    ],
                },
            );
        }
        Some(ChatQuery::PushesNoTicket) => match state.db.chat_query_pushes_no_ticket(scoped_org_id.as_deref()).await {
            Ok(rows) => {
                let answer = if rows.is_empty() {
                    "No se encontraron pushes a main sin ticket en los últimos 7 días.".to_string()
                } else {
                    format!("Se encontraron {} pushes a main sin ticket en los últimos 7 días.", rows.len())
                };
                return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                    StatusCode::OK,
                    ChatAskResponse {
                        status: "ok".to_string(),
                        answer,
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![
                            "github_events".to_string(),
                            "commit_ticket_correlations".to_string(),
                        ],
                    },
                );
            }
            Err(e) => {
                tracing::error!("chat_query_pushes_no_ticket error: {}", e);
                return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ChatAskResponse {
                        status: "error".to_string(),
                        answer: "Error consultando la base de datos".to_string(),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![],
                    },
                );
            }
        },
        Some(ChatQuery::BlockedPushesMonth) => match state.db.chat_query_blocked_pushes_month(scoped_org_id.as_deref()).await {
            Ok(count) => {
                let answer = format!("El equipo tiene {} pushes bloqueados en el mes actual.", count);
                return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                    StatusCode::OK,
                    ChatAskResponse {
                        status: "ok".to_string(),
                        answer,
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec!["client_events".to_string()],
                    },
                );
            }
            Err(e) => {
                tracing::error!("chat_query_blocked_pushes_month error: {}", e);
                return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ChatAskResponse {
                        status: "error".to_string(),
                        answer: "Error consultando la base de datos".to_string(),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![],
                    },
                );
            }
        },
        Some(ChatQuery::UserPushesCount {
            ref user,
            start_ms,
            end_ms,
        }) => {
            match state
                .db
                .chat_query_user_pushes_count(user, start_ms, end_ms, scoped_org_id.as_deref())
                .await
            {
                Ok(count) => {
                    let answer = if count == 0 {
                        let exists_in_scope = match user_exists_in_scope(
                            &state,
                            user,
                            scoped_org_id.as_deref(),
                        )
                        .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!("user_exists_in_scope(user_pushes_count) error: {}", e);
                                false
                            }
                        };
                        if exists_in_scope {
                            if start_ms.is_some() && end_ms.is_some() {
                                format!(
                                    "El usuario {} no tiene pushes exitosos en el rango solicitado.",
                                    user
                                )
                            } else {
                                format!(
                                    "El usuario {} no tiene pushes exitosos en el historial disponible para el scope activo.",
                                    user
                                )
                            }
                        } else {
                            format!(
                                "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                                user
                            )
                        }
                    } else if start_ms.is_some() && end_ms.is_some() {
                        format!(
                            "El usuario {} tiene {} pushes exitosos en el rango solicitado.",
                            user, count
                        )
                    } else {
                        format!(
                            "El usuario {} tiene {} pushes exitosos en el historial disponible para tu scope.",
                            user, count
                        )
                    };
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_pushes_count error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserActivityMonth { ref user }) => {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let month_start_ms = {
                let dt = chrono::Utc::now();
                let date = match chrono::NaiveDate::from_ymd_opt(dt.year(), dt.month(), 1) {
                    Some(d) => d,
                    None => {
                        return finalize_chat_response(
                            &state,
                            &conversation_key,
                            &mut session,
                            &nlp,
                            StatusCode::INTERNAL_SERVER_ERROR,
                            ChatAskResponse {
                                status: "error".to_string(),
                                answer: "No pude calcular el inicio del mes actual".to_string(),
                                missing_capability: None,
                                can_report_feature: false,
                                data_refs: vec![],
                            },
                        );
                    }
                };
                date.and_hms_opt(0, 0, 0)
                    .map(|x| {
                        chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(x, chrono::Utc)
                            .timestamp_millis()
                    })
                    .unwrap_or(0)
            };

            let exists_in_scope =
                match user_exists_in_scope(&state, user, scoped_org_id.as_deref()).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!("user_exists_in_scope(user_activity_month) error: {}", e);
                        false
                    }
                };
            if !exists_in_scope {
                return finalize_chat_response(
                    &state,
                    &conversation_key,
                    &mut session,
                    &nlp,
                    StatusCode::OK,
                    ChatAskResponse {
                        status: "insufficient_data".to_string(),
                        answer: format!(
                            "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                            user
                        ),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec!["org_users".to_string()],
                    },
                );
            }

            let commits = match state
                .db
                .chat_query_user_commits_count(
                    user,
                    Some(month_start_ms),
                    Some(now_ms),
                    scoped_org_id.as_deref(),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_user_commits_count(activity_month) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando commits del usuario".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };

            let pushes = match state
                .db
                .chat_query_user_pushes_count(
                    user,
                    Some(month_start_ms),
                    Some(now_ms),
                    scoped_org_id.as_deref(),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_user_pushes_count(activity_month) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando pushes del usuario".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            };

            let blocked_pushes = match state
                .db
                .chat_query_user_blocked_pushes_month(user, scoped_org_id.as_deref())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("chat_query_user_blocked_pushes_month(activity_month) error: {}", e);
                    0
                }
            };

            let answer = format!(
                "Actividad de {} en el mes actual (acumulado hasta ahora): commits={}, pushes exitosos={}, pushes bloqueados={}.",
                user, commits, pushes, blocked_pushes
            );
            return finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["client_events".to_string()],
                },
            );
        }
        Some(ChatQuery::UserPushesNoTicketWeek { ref user }) => {
            match state
                .db
                .chat_query_user_pushes_no_ticket_week(user, scoped_org_id.as_deref())
                .await
            {
                Ok(count) => {
                    let answer = if count == 0 {
                        let exists_in_scope = match user_exists_in_scope(
                            &state,
                            user,
                            scoped_org_id.as_deref(),
                        )
                        .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!("user_exists_in_scope(pushes_no_ticket) error: {}", e);
                                false
                            }
                        };
                        if exists_in_scope {
                            format!(
                                "No encontré pushes a main sin ticket para {} en los últimos 7 días.",
                                user
                            )
                        } else {
                            format!(
                                "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                                user
                            )
                        }
                    } else {
                        format!(
                            "Encontré {} pushes a main sin ticket para {} en los últimos 7 días.",
                            count, user
                        )
                    };
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![
                                "github_events".to_string(),
                                "commit_ticket_correlations".to_string(),
                            ],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_pushes_no_ticket_week error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserBlockedPushesMonth { ref user }) => {
            match state
                .db
                .chat_query_user_blocked_pushes_month(user, scoped_org_id.as_deref())
                .await
            {
                Ok(count) => {
                    let answer = if count == 0 {
                        let exists_in_scope = match user_exists_in_scope(
                            &state,
                            user,
                            scoped_org_id.as_deref(),
                        )
                        .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!("user_exists_in_scope(blocked_pushes) error: {}", e);
                                false
                            }
                        };
                        if exists_in_scope {
                            format!("{} no tiene pushes bloqueados en el mes actual.", user)
                        } else {
                            format!(
                                "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                                user
                            )
                        }
                    } else {
                        format!("{} tiene {} pushes bloqueados en el mes actual.", user, count)
                    };
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_blocked_pushes_month error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserAccessProfile { ref user }) => {
            match state
                .db
                .chat_query_user_access_profile(user, scoped_org_id.as_deref())
                .await
            {
                Ok(Some(profile)) => {
                    let login = profile
                        .get("login")
                        .and_then(|v| v.as_str())
                        .unwrap_or(user.as_str());
                    let role = profile
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("desconocido");
                    let status = profile
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("desconocido");
                    let has_active_api_key = profile
                        .get("has_active_api_key")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let answer = format!(
                        "Perfil de acceso de {}: rol={}, estado={}, API key activa={}. Nota: por seguridad no expongo valores de API key ni hashes.",
                        login,
                        role,
                        status,
                        if has_active_api_key { "sí" } else { "no" }
                    );
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["org_users".to_string(), "api_keys".to_string()],
                        },
                    );
                }
                Ok(None) => {
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "insufficient_data".to_string(),
                            answer: format!(
                                "No encontré al usuario {} en el scope activo. Verifica el login exacto y la organización seleccionada.",
                                user
                            ),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["org_users".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_access_profile error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserScopeClarification { ref user }) => {
            return finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "insufficient_data".to_string(),
                    answer: format!(
                        "¿Qué métrica quieres para {}? Opciones directas: 1) commits en rango, 2) pushes bloqueados del mes, 3) pushes a main sin ticket (7d), 4) rol/estado de acceso (sin exponer API key).",
                        user
                    ),
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs: vec!["assistant_runtime".to_string()],
                },
            );
        }
        Some(ChatQuery::SessionCommitsCount { ref user }) => {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let start_ms = session.session_started_ms.max(0);
            let selected_user = user
                .clone()
                .or_else(|| session.slots.last_user_login.clone());
            if let Some(ref selected_user) = selected_user {
                match state
                    .db
                    .chat_query_user_commits_count(
                        selected_user,
                        Some(start_ms),
                        Some(now_ms),
                        scoped_org_id.as_deref(),
                    )
                    .await
                {
                    Ok(count) => {
                        let answer = format!(
                            "En esta sesión (desde {} UTC), el usuario {} ha realizado {} commits.",
                            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(start_ms)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| "inicio no disponible".to_string()),
                            selected_user,
                            count
                        );
                        return finalize_chat_response(
                            &state,
                            &conversation_key,
                            &mut session,
                            &nlp,
                            StatusCode::OK,
                            ChatAskResponse {
                                status: "ok".to_string(),
                                answer,
                                missing_capability: None,
                                can_report_feature: false,
                                data_refs: vec!["client_events".to_string(), "assistant_runtime".to_string()],
                            },
                        );
                    }
                    Err(e) => {
                        tracing::error!("chat_query_user_commits_count(session) error: {}", e);
                        return finalize_chat_response(
                            &state,
                            &conversation_key,
                            &mut session,
                            &nlp,
                            StatusCode::INTERNAL_SERVER_ERROR,
                            ChatAskResponse {
                                status: "error".to_string(),
                                answer: "Error consultando la base de datos".to_string(),
                                missing_capability: None,
                                can_report_feature: false,
                                data_refs: vec![],
                            },
                        );
                    }
                }
            }

            match state
                .db
                .chat_query_commits_count(Some(start_ms), Some(now_ms), scoped_org_id.as_deref())
                .await
            {
                Ok(count) => {
                    let answer = format!(
                        "En esta sesión (desde {} UTC) hay {} commits registrados en el Control Plane.",
                        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(start_ms)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "inicio no disponible".to_string()),
                        count
                    );
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string(), "assistant_runtime".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_commits_count(session) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::TotalCommitsCount) => {
            match state
                .db
                .chat_query_commits_count(None, None, scoped_org_id.as_deref())
                .await
            {
                Ok(count) => {
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer: format!(
                                "El Control Plane registra {} commits en el historial disponible para tu scope.",
                                count
                            ),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_commits_count(total) error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserCommitsCount { ref user, start_ms, end_ms }) => {
            match state
                .db
                .chat_query_user_commits_count(user, start_ms, end_ms, scoped_org_id.as_deref())
                .await
            {
                Ok(count) => {
                    let answer = if count == 0 {
                        let exists_in_scope = match user_exists_in_scope(
                            &state,
                            user,
                            scoped_org_id.as_deref(),
                        )
                        .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!("user_exists_in_scope(user_commits_count) error: {}", e);
                                false
                            }
                        };
                        if exists_in_scope {
                            if start_ms.is_some() && end_ms.is_some() {
                                format!(
                                    "El usuario {} no tiene commits en el rango solicitado.",
                                    user
                                )
                            } else {
                                format!(
                                    "El usuario {} no tiene commits en el historial disponible para el scope activo.",
                                    user
                                )
                            }
                        } else {
                            format!(
                                "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                                user
                            )
                        }
                    } else if start_ms.is_some() && end_ms.is_some() {
                        format!(
                            "El usuario {} ha realizado {} commits en el rango solicitado.",
                            user, count
                        )
                    } else {
                        format!("El usuario {} ha realizado {} commits en todo el historial.", user, count)
                    };
                    return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_commits_count error: {}", e);
                    return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserLastCommit { ref user }) => {
            match state
                .db
                .chat_query_user_last_commit(user, scoped_org_id.as_deref())
                .await
            {
                Ok(Some(last_commit)) => {
                    let login = last_commit
                        .get("user_login")
                        .and_then(|v| v.as_str())
                        .unwrap_or(user.as_str());
                    let user_name = last_commit
                        .get("user_name")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|v| {
                            !v.is_empty()
                                && !v.eq_ignore_ascii_case("unknown")
                                && !v.eq_ignore_ascii_case("desconocido")
                        });
                    let branch = last_commit
                        .get("branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("desconocida");
                    let sha = last_commit
                        .get("commit_sha")
                        .and_then(|v| v.as_str())
                        .unwrap_or("desconocido");
                    let repo = last_commit
                        .get("repo_full_name")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|v| !v.is_empty());
                    let commit_message = last_commit
                        .get("commit_message")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|v| !v.is_empty());
                    let display_user = if let Some(name) = user_name {
                        if !name.eq_ignore_ascii_case(login) {
                            format!("{} ({})", login, name)
                        } else {
                            login.to_string()
                        }
                    } else {
                        login.to_string()
                    };
                    let repo_fragment = repo
                        .map(|r| format!(" | Repo: {}", r))
                        .unwrap_or_default();
                    let message_fragment = commit_message
                        .map(|m| format!(" | Mensaje: {}", m))
                        .unwrap_or_default();
                    let timestamp_ms = last_commit.get("timestamp").and_then(|v| v.as_i64());
                    let answer = if let Some(ts_ms) = timestamp_ms {
                        if let Some(dt_utc) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts_ms) {
                            let lima_tz = chrono::FixedOffset::west_opt(5 * 3600)
                                .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("valid offset"));
                            let dt_lima = dt_utc.with_timezone(&lima_tz);
                            format!(
                                "Último commit registrado para {}: {} en la rama `{}`{}{}. Fecha del evento: {} (America/Lima) | {} UTC.",
                                display_user,
                                sha,
                                branch,
                                repo_fragment,
                                message_fragment,
                                dt_lima.format("%Y-%m-%d %H:%M:%S"),
                                dt_utc.format("%Y-%m-%d %H:%M:%S")
                            )
                        } else {
                            format!(
                                "Último commit registrado para {}: {} en la rama `{}`{}{}. No pude convertir su timestamp a fecha legible.",
                                display_user, sha, branch, repo_fragment, message_fragment
                            )
                        }
                    } else {
                        format!(
                            "Último commit registrado para {}: {} en la rama `{}`{}{}. No hay timestamp disponible en el evento.",
                            display_user, sha, branch, repo_fragment, message_fragment
                        )
                    };
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string(), "assistant_runtime".to_string()],
                        },
                    );
                }
                Ok(None) => {
                    let exists_in_scope = match user_exists_in_scope(
                        &state,
                        user,
                        scoped_org_id.as_deref(),
                    )
                    .await
                    {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::error!("user_exists_in_scope(user_last_commit) error: {}", e);
                            false
                        }
                    };
                    let answer = if exists_in_scope {
                        format!(
                            "No encontré commits para {} en el historial disponible del scope activo.",
                            user
                        )
                    } else {
                        format!(
                            "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                            user
                        )
                    };
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "insufficient_data".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string(), "org_users".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_last_commit error: {}", e);
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        Some(ChatQuery::UserCommitsRange { ref user, start_ms, end_ms }) => {
            match state
                .db
                .chat_query_user_commits_range(user, start_ms, end_ms, scoped_org_id.as_deref())
                .await
            {
                Ok(rows) => {
                    let answer = if rows.is_empty() {
                        let exists_in_scope = match user_exists_in_scope(
                            &state,
                            user,
                            scoped_org_id.as_deref(),
                        )
                        .await
                        {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!("user_exists_in_scope(user_commits_range) error: {}", e);
                                false
                            }
                        };
                        if exists_in_scope {
                            format!("No encontré commits de {} en el rango solicitado.", user)
                        } else {
                            format!(
                                "No encontré al usuario {} dentro del scope activo. Verifica login exacto y organización seleccionada.",
                                user
                            )
                        }
                    } else {
                        format!("Encontré {} commits de {} en el rango solicitado.", rows.len(), user)
                    };
                    return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "ok".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec!["client_events".to_string()],
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("chat_query_user_commits_range error: {}", e);
                    return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ChatAskResponse {
                            status: "error".to_string(),
                            answer: "Error consultando la base de datos".to_string(),
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![],
                        },
                    );
                }
            }
        }
        None => {}
    }

    if is_logs_precision_query(&question) {
        let mut filter = EventFilter {
            limit: extract_logs_limit(&question, 5, 20),
            ..EventFilter::default()
        };
        filter.org_id = scoped_org_id.clone();
        filter.org_name = None;
        filter.user_login = nlp.entities.user_login.clone();
        filter.event_type = extract_logs_event_type_hint(&question);

        match state.db.get_combined_events(&filter).await {
            Ok(events) => {
                if events.is_empty() {
                    let answer = if nlp.entities.language == "en" {
                        "I did not find log events for the requested scope/filters. Provide org/user/event_type or a narrower time window.".to_string()
                    } else {
                        "No encontre eventos de log para el scope/filtros solicitados. Indica org/usuario/tipo de evento o una ventana de tiempo mas acotada.".to_string()
                    };
                    return finalize_chat_response(
                        &state,
                        &conversation_key,
                        &mut session,
                        &nlp,
                        StatusCode::OK,
                        ChatAskResponse {
                            status: "insufficient_data".to_string(),
                            answer,
                            missing_capability: None,
                            can_report_feature: false,
                            data_refs: vec![
                                "logs_endpoint".to_string(),
                                "deterministic_sql_results".to_string(),
                            ],
                        },
                    );
                }

                let answer = render_precise_logs_answer(&events, &nlp.entities.language);
                return finalize_chat_response(
                    &state,
                    &conversation_key,
                    &mut session,
                    &nlp,
                    StatusCode::OK,
                    ChatAskResponse {
                        status: "ok".to_string(),
                        answer,
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec![
                            "logs_endpoint".to_string(),
                            "deterministic_sql_results".to_string(),
                        ],
                    },
                );
            }
            Err(e) => {
                tracing::error!("deterministic logs answer error: {}", e);
                return finalize_chat_response(
                    &state,
                    &conversation_key,
                    &mut session,
                    &nlp,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ChatAskResponse {
                        status: "error".to_string(),
                        answer: "Error consultando logs exactos en la base de datos".to_string(),
                        missing_capability: None,
                        can_report_feature: false,
                        data_refs: vec!["logs_endpoint".to_string()],
                    },
                );
            }
        }
    }

    if let Some(answer) = build_grounded_knowledge_answer(&question, &nlp.entities.language) {
        return finalize_chat_response(
            &state,
            &conversation_key,
            &mut session,
            &nlp,
            StatusCode::OK,
            ChatAskResponse {
                status: "ok".to_string(),
                answer,
                missing_capability: None,
                can_report_feature: false,
                data_refs: vec!["project_docs_kb".to_string(), "web_docs_faq".to_string()],
            },
        );
    }

    let Some(api_key) = state.llm_api_key.as_deref() else {
        tracing::warn!("GEMINI_API_KEY not configured; returning feature_not_available");
        return finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
            StatusCode::OK,
            ChatAskResponse {
                status: "feature_not_available".to_string(),
                answer: "El asistente conversacional no está configurado en este servidor. Configura GEMINI_API_KEY para activarlo.".to_string(),
                missing_capability: Some("llm_integration".to_string()),
                can_report_feature: true,
                data_refs: vec![],
            },
        );
    };

    let mut data_refs = vec![
        "project_docs_kb".to_string(),
        "web_docs_faq".to_string(),
        "conversation_context".to_string(),
        "todo_runtime".to_string(),
    ];
    data_refs.append(&mut snapshot_refs);

    let queue_timeout = Duration::from_millis(state.chat_llm_queue_timeout_ms);
    let llm_timeout = Duration::from_millis(state.chat_llm_timeout_ms);
    let permit = match timeout(
        queue_timeout,
        state.chat_llm_semaphore.clone().acquire_owned(),
    )
    .await
    {
        Ok(Ok(permit)) => permit,
        Ok(Err(e)) => {
            tracing::error!("chat llm semaphore acquire failed: {}", e);
            return finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::SERVICE_UNAVAILABLE,
                ChatAskResponse {
                    status: "error".to_string(),
                    answer: if nlp.entities.language == "en" {
                        "Chat is temporarily unavailable due to internal capacity controls. Try again in a few seconds.".to_string()
                    } else {
                        "El chat está temporalmente no disponible por control interno de capacidad. Intenta de nuevo en unos segundos.".to_string()
                    },
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs,
                },
            );
        }
        Err(_) => {
            tracing::warn!(
                queue_timeout_ms = state.chat_llm_queue_timeout_ms,
                "chat request rejected due to LLM queue timeout"
            );
            return finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::TOO_MANY_REQUESTS,
                ChatAskResponse {
                    status: "error".to_string(),
                    answer: if nlp.entities.language == "en" {
                        "Chat is busy right now. Try again in a few seconds.".to_string()
                    } else {
                        "El chat está ocupado en este momento. Intenta de nuevo en unos segundos.".to_string()
                    },
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs,
                },
            );
        }
    };

    let llm_question = sanitize_chat_answer_text(&question);
    let data = build_advanced_conversation_payload(&llm_question, &nlp, &session);
    let llm_result = timeout(
        llm_timeout,
        call_llm(&state.http_client, api_key, &state.llm_model, &llm_question, &data),
    )
    .await;
    drop(permit);

    match llm_result {
        Ok(Ok(mut resp)) => {
            resp = normalize_llm_response(resp, &question, &nlp.entities.language);
            if should_override_llm_answer_with_kb(&resp, &question) {
                if let Some(answer) = build_grounded_knowledge_answer(&question, &nlp.entities.language) {
                    resp.status = "ok".to_string();
                    resp.answer = answer;
                    resp.missing_capability = None;
                    resp.can_report_feature = false;
                    resp.data_refs.push("project_docs_kb".to_string());
                    resp.data_refs.push("web_docs_faq".to_string());
                }
            }
            let mut refs = data_refs.clone();
            refs.extend(resp.data_refs.clone());
            refs.sort();
            refs.dedup();
            resp.data_refs = refs;
            finalize_chat_response(&state, &conversation_key, &mut session, &nlp, StatusCode::OK, resp)
        }
        Ok(Err(e)) => {
            tracing::error!("LLM call failed: {}", e);
            let answer = llm_degraded_answer(&question, &nlp.entities.language);
            finalize_chat_response(&state, &conversation_key, &mut session, &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs,
                },
            )
        }
        Err(_) => {
            tracing::warn!(
                llm_timeout_ms = state.chat_llm_timeout_ms,
                "chat request exceeded LLM timeout"
            );
            let answer = llm_degraded_answer(&question, &nlp.entities.language);
            finalize_chat_response(
                &state,
                &conversation_key,
                &mut session,
                &nlp,
                StatusCode::OK,
                ChatAskResponse {
                    status: "ok".to_string(),
                    answer,
                    missing_capability: None,
                    can_report_feature: false,
                    data_refs,
                },
            )
        }
    }
}

