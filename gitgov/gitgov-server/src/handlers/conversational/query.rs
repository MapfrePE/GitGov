fn is_probable_user_login_token(token: &str) -> bool {
    let t = token.trim().to_ascii_lowercase();
    if t.len() < 3 || t.len() > 39 {
        return false;
    }
    if !t.chars().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return false;
    }

    let stopwords = [
        "jira",
        "github",
        "gitgov",
        "control",
        "plane",
        "main",
        "esta",
        "este",
        "estas",
        "estos",
        "this",
        "that",
        "the",
        "all",
        "mes",
        "month",
        "semana",
        "week",
        "sesion",
        "sesión",
        "actual",
        "hoy",
        "today",
        "ayer",
        "yesterday",
        "historial",
        "history",
        "ticket",
        "tickets",
        "rol",
        "role",
        "api",
        "key",
        "apikey",
        "acceso",
        "access",
        "usuario",
        "user",
        "equipo",
        "team",
        "soporte",
        "support",
        "precio",
        "pricing",
        "plan",
        "windows",
        "linux",
        "mac",
        "macos",
        "webhook",
        "jenkins",
        "activo",
        "activa",
    ];
    !stopwords.contains(&t.as_str())
}

fn extract_user_login(q: &str) -> Option<String> {
    static EXPLICIT_RE: OnceLock<Regex> = OnceLock::new();
    static COMMIT_SCOPED_RE: OnceLock<Regex> = OnceLock::new();
    static COMMIT_VERB_RE: OnceLock<Regex> = OnceLock::new();
    static PREPOSITION_RE: OnceLock<Regex> = OnceLock::new();
    static ROLE_ACCESS_RE: OnceLock<Regex> = OnceLock::new();

    let explicit_re = EXPLICIT_RE.get_or_init(|| {
        Regex::new(r"(?:del usuario|el usuario|usuario|of user|user)\s+([a-z0-9_\-\.]+)\b")
            .expect("valid explicit user regex")
    });
    if let Some(user) = explicit_re
        .captures(q)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|u| is_probable_user_login_token(u))
    {
        return Some(user);
    }

    let commit_scoped_re = COMMIT_SCOPED_RE.get_or_init(|| {
        Regex::new(r"commit(?:s)?\s+(?:de|by)\s+([a-z0-9_\-\.]+)\b")
            .expect("valid commit scoped regex")
    });
    if let Some(user) = commit_scoped_re
        .captures(q)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|u| is_probable_user_login_token(u))
    {
        return Some(user);
    }

    let commit_verb_re = COMMIT_VERB_RE.get_or_init(|| {
        Regex::new(r"commit(?:s)?\s+(?:did|hizo|made|for)\s+([a-z0-9_\-\.]+)\b")
            .expect("valid commit verb regex")
    });
    if let Some(user) = commit_verb_re
        .captures(q)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|u| is_probable_user_login_token(u))
    {
        return Some(user);
    }

    let role_access_re = ROLE_ACCESS_RE.get_or_init(|| {
        Regex::new(
            r"(?:rol|role|acceso|access|api key|apikey)\s+(?:de|del|for|of|tiene|has)?\s*([a-z0-9_\-\.]{3,39})\b",
        )
        .expect("valid role/access regex")
    });
    if let Some(user) = role_access_re
        .captures(q)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|u| is_probable_user_login_token(u))
    {
        return Some(user);
    }

    let has_user_metric_context = [
        "commit",
        "commits",
        "push",
        "pushes",
        "bloqueado",
        "blocked",
        "ticket",
        "jira",
        "rol",
        "role",
        "api key",
        "apikey",
        "acceso",
        "access",
    ]
    .iter()
    .any(|m| q.contains(m));

    if has_user_metric_context {
        let preposition_re = PREPOSITION_RE.get_or_init(|| {
            Regex::new(r"(?:\bde\b|\bdel\b|\bfor\b|\bof\b)\s+([a-z0-9_\-\.]{3,39})\b")
                .expect("valid preposition user regex")
        });
        for caps in preposition_re.captures_iter(q) {
            if let Some(token) = caps.get(1).map(|m| m.as_str().to_ascii_lowercase()) {
                if is_probable_user_login_token(&token) {
                    return Some(token);
                }
            }
        }
    }

    None
}

fn detect_query(question: &str) -> Option<ChatQuery> {
    let q = question.to_lowercase();
    let q = q.trim();
    let extracted_user = extract_user_login(q);

    let asks_session_window = q.contains("esta sesion")
        || q.contains("esta sesión")
        || q.contains("sesion actual")
        || q.contains("sesión actual")
        || q.contains("this session");

    let asks_all_history = q.contains("todo el historial")
        || q.contains("historial completo")
        || q.contains("all history")
        || q.contains("entire history");

    let asks_count = q.contains("cuanto")
        || q.contains("cuánto")
        || q.contains("cuantos")
        || q.contains("cuántos")
        || q.contains("how many")
        || q.contains("cantidad")
        || q.contains("total")
        || q.contains("numero")
        || q.contains("número");
    let asks_last_commit = q.contains("ultimo commit")
        || q.contains("último commit")
        || q.contains("last commit");
    let asks_date_mismatch = (q.contains("como es posible")
        || q.contains("cómo es posible")
        || q.contains("confunde fechas")
        || q.contains("inconsistencia de fecha")
        || q.contains("fecha incorrecta"))
        && (q.contains("hoy")
            || q.contains("utc")
            || q.contains("zona horaria")
            || q.contains("marzo")
            || q.contains("fecha"));

    let asks_role = q.contains("rol") || q.contains("role");
    let asks_api_key = q.contains("api key") || q.contains("apikey");
    let asks_user_context = q.contains("usuario ") || q.contains("user ") || extracted_user.is_some();
    let asks_pushes = q.contains("push") || q.contains("pushes");
    let asks_activity = q.contains("actividad") || q.contains("activity");
    let asks_blocked_pushes =
        (q.contains("bloqueado") || q.contains("bloqueados") || q.contains("blocked"))
            && (q.contains("push") || q.contains("pushes"));
    let asks_no_ticket_pushes =
        (q.contains("push") || q.contains("pushes") || q.contains("empujo") || q.contains("empujaron"))
            && (q.contains("ticket") || q.contains("jira") || q.contains("sin ticket") || q.contains("without ticket"));
    let asks_no_ticket_commits = (q.contains("commit") || q.contains("commits"))
        && (q.contains("ticket")
            || q.contains("jira")
            || q.contains("sin ticket")
            || q.contains("without ticket")
            || q.contains("sin jira")
            || q.contains("without jira")
            || q.contains("huerf")
            || q.contains("huérf"));
    let asks_online_devs = (q.contains("dev")
        || q.contains("developer")
        || q.contains("desarrollador")
        || q.contains("desarrolladores"))
        && (q.contains(" on")
            || q.ends_with(" on")
            || q.contains("online")
            || q.contains("conectad")
            || q.contains("activo ahora")
            || q.contains("active now")
            || q.contains("en linea")
            || q.contains("en línea"));
    let asks_executive_summary = (q.contains("resumen")
        || q.contains("overview")
        || q.contains("estado general")
        || q.contains("estado del control plane")
        || q.contains("dashboard completo")
        || q.contains("todo lo que")
        || q.contains("todo del control plane")
        || q.contains("todo, todo"))
        && (q.contains("control plane")
            || q.contains("dashboard")
            || q.contains("governance")
            || q.contains("gitgov")
            || q.contains("metricas")
            || q.contains("métricas"));

    let parse_date = |s: &str| -> Option<i64> {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .or_else(|_| chrono::NaiveDate::parse_from_str(s, "%d/%m/%Y"))
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| {
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)
                    .timestamp_millis()
            })
    };
    let date_re = Regex::new(
        r"(?:entre|from|desde)\s+(\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4})\s+(?:y|and|to|hasta)\s+(\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4})",
    )
    .ok();
    let explicit_range = date_re.as_ref().and_then(|re| {
        re.captures(q).and_then(|caps| {
            let s = parse_date(caps.get(1)?.as_str())?;
            let e = parse_date(caps.get(2)?.as_str())?;
            Some((s, e))
        })
    });

    let now = chrono::Utc::now().timestamp_millis();
    let this_month_start_ms = {
        let dt = chrono::Utc::now();
        let date = chrono::NaiveDate::from_ymd_opt(dt.year(), dt.month(), 1)?;
        date.and_hms_opt(0, 0, 0)
            .map(|x| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(x, chrono::Utc).timestamp_millis())
            .unwrap_or(0)
    };
    let week_start_ms = now - 7 * 24 * 60 * 60 * 1000;
    let window_range_keywords: Option<(i64, i64)> = if q.contains("esta semana") || q.contains("this week") {
        Some((week_start_ms, now))
    } else if q.contains("este mes") || q.contains("this month") || q.contains("mes de marzo") || q.contains("month of march") {
        Some((this_month_start_ms, now))
    } else {
        None
    };

    if asks_activity
        && asks_user_context
        && (q.contains("mes") || q.contains("month") || q.contains("marzo") || q.contains("march"))
    {
        if let Some(user) = extracted_user.clone() {
            return Some(ChatQuery::UserActivityMonth { user });
        }
    }

    if asks_executive_summary {
        return Some(ChatQuery::ControlPlaneExecutiveSummary);
    }

    if asks_online_devs && asks_count {
        let minutes = if q.contains("hoy") || q.contains("today") {
            24 * 60
        } else {
            15
        };
        return Some(ChatQuery::OnlineDevelopersNow { minutes });
    }

    if asks_no_ticket_commits {
        let hours = if q.contains("mes") || q.contains("month") {
            24 * 30
        } else if q.contains("semana")
            || q.contains("week")
            || q.contains("7d")
            || q.contains("7 dias")
            || q.contains("7 días")
        {
            24 * 7
        } else {
            24 * 7
        };
        return Some(ChatQuery::CommitsWithoutTicketWindow { hours });
    }

    if asks_pushes && asks_count && asks_user_context && !asks_no_ticket_pushes && !asks_blocked_pushes {
        if let Some(user) = extracted_user.clone() {
            let (start_ms, end_ms) = explicit_range
                .or(window_range_keywords)
                .map(|(s, e)| (Some(s), Some(e)))
                .unwrap_or((None, None));
            return Some(ChatQuery::UserPushesCount {
                user,
                start_ms,
                end_ms,
            });
        }
    }

    if (asks_role || asks_api_key) && asks_user_context {
        if let Some(user) = extracted_user.clone() {
            return Some(ChatQuery::UserAccessProfile { user });
        }
    }

    if asks_blocked_pushes && asks_user_context {
        if let Some(user) = extracted_user.clone() {
            return Some(ChatQuery::UserBlockedPushesMonth { user });
        }
    }

    if asks_no_ticket_pushes && asks_user_context {
        if let Some(user) = extracted_user.clone() {
            return Some(ChatQuery::UserPushesNoTicketWeek { user });
        }
    }

    // Q1: push a main esta semana sin ticket de Jira
    if (q.contains("push") || q.contains("empujo") || q.contains("empujaron"))
        && (q.contains("main") || q.contains("principal"))
        && (q.contains("semana") || q.contains("week") || q.contains("últimos 7") || q.contains("last 7"))
        && (q.contains("ticket") || q.contains("jira") || q.contains("sin ticket") || q.contains("without ticket"))
    {
        return Some(ChatQuery::PushesNoTicket);
    }

    // Q2: pushes bloqueados este mes
    if (q.contains("bloqueado") || q.contains("bloqueados") || q.contains("blocked"))
        && (q.contains("push") || q.contains("pushes"))
        && (q.contains("mes") || q.contains("month") || q.contains("este mes") || q.contains("this month"))
    {
        return Some(ChatQuery::BlockedPushesMonth);
    }

    // Q-session: commits during current assistant session
    if (q.contains("commit") || q.contains("commits")) && asks_session_window {
        return Some(ChatQuery::SessionCommitsCount {
            user: extracted_user.clone(),
        });
    }

    // Q-total: total commits in Control Plane
    if (q.contains("commit") || q.contains("commits"))
        && asks_count
        && (q.contains("control plane") || q.contains("gitgov") || q.contains("plataforma"))
        && !q.contains("usuario ")
        && !q.contains("user ")
    {
        return Some(ChatQuery::TotalCommitsCount);
    }

    // Follow-up corto: "y del usuario X?"
    if (q.starts_with("y del usuario ")
        || q.starts_with("y de ")
        || asks_all_history)
        && asks_user_context
    {
        let user = extracted_user.clone()?;
        if q.contains("commit") || q.contains("commits") || asks_all_history || q.contains("historial") {
            return Some(ChatQuery::UserCommitsCount {
                user,
                start_ms: None,
                end_ms: None,
            });
        }
        if asks_blocked_pushes {
            return Some(ChatQuery::UserBlockedPushesMonth { user });
        }
        if asks_no_ticket_pushes {
            return Some(ChatQuery::UserPushesNoTicketWeek { user });
        }
        if asks_role || asks_api_key {
            return Some(ChatQuery::UserAccessProfile { user });
        }
        return Some(ChatQuery::UserScopeClarification { user });
    }

    // Q3/Q4/Q-last: commits por usuario (listado, conteo o último commit)
    if q.contains("commit") {
        // Extract user login from explicit user markers or commit-scoped phrasing.
        let user = extracted_user.clone();

        let commit_window_keywords: Option<(i64, i64)> = if asks_session_window {
            // Session range is handled by ChatQuery::SessionCommitsCount.
            None
        } else {
            window_range_keywords
        };

        if user.is_none() {
            return Some(ChatQuery::NeedUserForCommitHistory);
        }
        let user = user.unwrap_or_default();

        if asks_last_commit {
            return Some(ChatQuery::UserLastCommit { user });
        }

        if asks_count {
            let (start_ms, end_ms) = explicit_range
                .or(commit_window_keywords)
                .map(|(s, e)| (Some(s), Some(e)))
                .unwrap_or((None, None)); // all-time by default for count intent
            return Some(ChatQuery::UserCommitsCount { user, start_ms, end_ms });
        }

        let (start_ms, end_ms) = explicit_range
            .or(commit_window_keywords)
            .unwrap_or_else(|| {
                // Default to last 30 days for listing intent
                let thirty_days_ago = now - 30 * 24 * 60 * 60 * 1000;
                (thirty_days_ago, now)
            });

        return Some(ChatQuery::UserCommitsRange { user, start_ms, end_ms });
    }

    // If user asks for "all history" in commit context without a user, ask for user explicitly
    // instead of falling back to generic docs/KB.
    if asks_all_history && !q.contains("usuario ") && !q.contains("user ") {
        return Some(ChatQuery::NeedUserForCommitHistory);
    }

    // Conversational intents (no SQL required)
    if q.contains("hola")
        || q.contains("hello")
        || q.contains("hi ")
        || q.contains("hey")
        || q.contains("buenos dias")
        || q.contains("buenas tardes")
        || q.contains("buenas noches")
    {
        return Some(ChatQuery::Greeting);
    }

    if asks_date_mismatch {
        return Some(ChatQuery::DateMismatchClarification);
    }

    let asks_current_datetime = q.contains("fecha actual")
        || q.contains("hora actual")
        || q.contains("qué hora es")
        || q.contains("que hora es")
        || q.contains("qué fecha es")
        || q.contains("que fecha es")
        || q.contains("que dia es hoy")
        || q.contains("qué día es hoy")
        || q.contains("today date")
        || q.contains("date and time")
        || q.contains("current date")
        || q.contains("current time")
        || q.trim() == "hora"
        || q.trim() == "fecha";

    if asks_current_datetime {
        return Some(ChatQuery::CurrentDateTime);
    }

    if (q.contains("control plane") || q.contains("datos"))
        && (q.contains("puedes ver")
            || q.contains("puede ver")
            || q.contains("puedes consultar")
            || q.contains("puedes leer"))
    {
        return Some(ChatQuery::CapabilityOverview);
    }

    if q.contains("ayuda")
        || q.contains("help")
        || q.contains("guiame")
        || q.contains("guíame")
        || q.contains("paso a paso")
        || q.contains("configurar")
        || q.contains("setup")
        || q.contains("conectar")
    {
        return Some(ChatQuery::GuidedHelp);
    }

    if q.contains("precio")
        || q.contains("pricing")
        || q.contains("plan")
        || q.contains("gratis")
        || q.contains("free")
        || q.contains("descarga")
        || q.contains("download")
        || q.contains("windows")
        || q.contains("linux")
        || q.contains("mac")
        || q.contains("macos")
        || q.contains("smartscreen")
        || q.contains("firma")
        || q.contains("soporte")
        || q.contains("support")
        || q.contains("contacto")
        || q.contains("open source")
        || q.contains("codigo abierto")
        || q.contains("código abierto")
        || q.contains("lee codigo")
        || q.contains("código fuente")
        || q.contains("source code")
        || q.contains("plataformas")
        || q.contains("integraciones")
    {
        return Some(ChatQuery::GuidedHelp);
    }

    None
}

fn is_secret_exfiltration_request(question: &str) -> bool {
    let q = question.to_lowercase();
    let asks_secret_object = q.contains("api key")
        || q.contains("apikey")
        || q.contains("token")
        || q.contains("secret")
        || q.contains("clave")
        || q.contains("key")
        || q.contains("password")
        || q.contains("contraseña")
        || q.contains("credential")
        || q.contains("credencial")
        || q.contains("jwt")
        || q.contains("hash");
    let asks_secret_value = q.contains("dame")
        || q.contains("mostrar")
        || q.contains("muéstr")
        || q.contains("muestr")
        || q.contains("cual es")
        || q.contains("cuál es")
        || q.contains("dime")
        || q.contains("reveal")
        || q.contains("show")
        || q.contains("give")
        || q.contains("get")
        || q.contains("obten")
        || q.contains("obtén")
        || q.contains("extrae")
        || q.contains("extraer")
        || q.contains("lista")
        || q.contains("listar")
        || q.contains("copiar")
        || q.contains("valor")
        || q.contains("texto plano")
        || q.contains("plain text")
        || q.contains("clave");
    asks_secret_object && asks_secret_value
}

fn sanitize_chat_answer_text(input: &str) -> String {
    static UUID_RE: OnceLock<Regex> = OnceLock::new();
    static BEARER_RE: OnceLock<Regex> = OnceLock::new();
    static JWT_RE: OnceLock<Regex> = OnceLock::new();
    static GH_TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static SK_TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static KV_SECRET_RE: OnceLock<Regex> = OnceLock::new();

    let uuid_re = UUID_RE.get_or_init(|| {
        Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b")
            .expect("valid uuid regex")
    });
    let bearer_re = BEARER_RE.get_or_init(|| {
        Regex::new(r"(?i)\bbearer\s+[a-z0-9\-\._]{20,}\b").expect("valid bearer regex")
    });
    let jwt_re = JWT_RE.get_or_init(|| {
        Regex::new(r"\beyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\b")
            .expect("valid jwt regex")
    });
    let gh_token_re = GH_TOKEN_RE.get_or_init(|| {
        Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{20,}\b").expect("valid github token regex")
    });
    let sk_token_re = SK_TOKEN_RE.get_or_init(|| {
        Regex::new(r"\bsk-[A-Za-z0-9]{20,}\b").expect("valid sk token regex")
    });
    let kv_secret_re = KV_SECRET_RE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(api[_-]?key|token|secret|password|contraseña|jwt|hash)\b\s*[:=]\s*['"]?[A-Za-z0-9_\-\.]{8,}"#,
        )
        .expect("valid key-value secret regex")
    });

    let redacted_uuid = uuid_re.replace_all(input, "[REDACTED_SECRET]");
    let redacted_bearer = bearer_re.replace_all(redacted_uuid.as_ref(), "Bearer [REDACTED_SECRET]");
    let redacted_jwt = jwt_re.replace_all(redacted_bearer.as_ref(), "[REDACTED_SECRET]");
    let redacted_gh = gh_token_re.replace_all(redacted_jwt.as_ref(), "[REDACTED_SECRET]");
    let redacted_sk = sk_token_re.replace_all(redacted_gh.as_ref(), "[REDACTED_SECRET]");
    kv_secret_re
        .replace_all(redacted_sk.as_ref(), "$1: [REDACTED_SECRET]")
        .to_string()
}

fn weekday_es(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "lunes",
        chrono::Weekday::Tue => "martes",
        chrono::Weekday::Wed => "miércoles",
        chrono::Weekday::Thu => "jueves",
        chrono::Weekday::Fri => "viernes",
        chrono::Weekday::Sat => "sábado",
        chrono::Weekday::Sun => "domingo",
    }
}

fn build_guided_help_answer(question: &str) -> String {
    let q = question.to_lowercase();

    if q.contains("precio")
        || q.contains("costo")
        || q.contains("pricing")
        || q.contains("plan")
        || q.contains("licencia")
        || q.contains("gratis")
        || q.contains("free")
    {
        return "Hoy GitGov no publica una tabla de precios cerrada dentro del asistente. Para planes y licenciamiento, la referencia oficial es https://git-gov.vercel.app y el contacto comercial directo del equipo. Si quieres, te doy un checklist para evaluar plan enterprise (usuarios, repos, integraciones y retención).".to_string();
    }
    if q.contains("descarga")
        || q.contains("download")
        || q.contains("instalar")
        || q.contains("installer")
    {
        return "La descarga de GitGov Desktop se distribuye por los canales oficiales del proyecto. Guía corta: 1) descarga el instalador oficial, 2) instala la app, 3) configura servidor y API key en Settings, 4) valida conexión con /health, 5) prueba commit + push para confirmar el Golden Path.".to_string();
    }
    if (q.contains("warning") || q.contains("advertencia") || q.contains("smartscreen"))
        && (q.contains("windows") || q.contains("firma") || q.contains("signed"))
    {
        return "Ese warning en Windows suele aparecer cuando el instalador no está firmado con un certificado de code-signing reconocido por Microsoft SmartScreen. No implica automáticamente malware, pero sí menor reputación de editor. Para quitarlo en distribución enterprise, se recomienda firmar el instalador/binario (Authenticode) y mantener una cadena de release verificable.".to_string();
    }
    if q.contains("linux")
        || q.contains("mac")
        || q.contains("macos")
        || q.contains("windows")
        || q.contains("sistema operativo")
        || q.contains("sistemas operativos")
    {
        return "GitGov Desktop soporta Windows, macOS y Linux (stack Tauri). Si quieres, te detallo pasos de instalación/actualización por plataforma.".to_string();
    }
    if q.contains("soporte")
        || q.contains("support")
        || q.contains("contacto")
        || q.contains("contactar")
    {
        return "Para soporte y dudas comerciales, el canal oficial es el equipo de GitGov vía https://git-gov.vercel.app (sección de contacto). Si quieres, te ayudo a redactar un mensaje de soporte con versión, sistema operativo, error y pasos para reproducir.".to_string();
    }

    if q.contains("jira") {
        return "Pasos para conectar Jira con GitGov: 1) Configura `JIRA_WEBHOOK_SECRET` en el server. 2) Crea webhook de Jira a `POST /integrations/jira`. 3) Envía `Authorization: Bearer <admin_key>` y, si aplica, `x-gitgov-jira-secret`. 4) Verifica estado con `GET /integrations/jira/status`. 5) Ejecuta `POST /integrations/jira/correlate` y valida cobertura en `GET /integrations/jira/ticket-coverage`.".to_string();
    }
    if q.contains("jenkins") {
        return "Pasos para conectar Jenkins con GitGov: 1) Configura `JENKINS_WEBHOOK_SECRET` (opcional). 2) Desde pipeline envía POST a `/integrations/jenkins` con `Authorization: Bearer <admin_key>`. 3) Si usas secreto, añade `x-gitgov-jenkins-secret`. 4) Revisa `GET /integrations/jenkins/status`. 5) Valida correlaciones en `GET /integrations/jenkins/correlations`.".to_string();
    }
    if q.contains("github") {
        return "Pasos para conectar GitHub: 1) Define `GITHUB_WEBHOOK_SECRET` en el server. 2) En GitHub crea webhook a `/webhooks/github` con JSON. 3) Usa el mismo secret para firma HMAC. 4) Activa eventos push/create. 5) Verifica `GET /health` y revisa eventos en `/logs`.".to_string();
    }
    if q.contains("settings") || q.contains("onboarding") || q.contains("organizacion") || q.contains("organización") {
        return "Pasos de onboarding admin en Settings: 1) Crear/Upsert organización. 2) Definir org activa. 3) Provisionar miembros o generar invitaciones por rol. 4) Emitir API keys por usuario. 5) Revisar actividad en Gestión de Equipo (developers/repos/eventos).".to_string();
    }

    if let Some(answer) = build_grounded_knowledge_answer(question, &detect_language(question)) {
        return answer;
    }

    "Puedo ayudarte en cuatro frentes: 1) analítica (commits/pushes/bloqueos), 2) integraciones (GitHub/Jenkins/Jira), 3) configuración de org/roles/settings, 4) troubleshooting técnico. Dime cuál de esos 4 quieres y te doy pasos exactos.".to_string()
}

fn rank_project_knowledge(question: &str) -> Vec<RankedKnowledgeSnippet> {
    let q = question.to_lowercase();
    let mut ranked: Vec<RankedKnowledgeSnippet> = Vec::new();

    let mut collect = |entries: &[(&'static str, &[&'static str], &'static str)], source: &'static str| {
        for (title, keywords, content) in entries {
            let mut score = 0;
            if q.contains(&title.to_lowercase()) {
                score += 4;
            }
            for kw in *keywords {
                if q.contains(kw) {
                    score += 2;
                }
            }
            if score > 0 {
                ranked.push(RankedKnowledgeSnippet {
                    score,
                    title,
                    content,
                    source,
                });
            }
        }
    };

    collect(PROJECT_KNOWLEDGE_BASE, "project_docs_kb");
    collect(WEB_FAQ_KNOWLEDGE_BASE, "web_docs_faq");

    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.title.cmp(b.title))
            .then_with(|| a.source.cmp(b.source))
    });
    ranked
}

fn is_live_data_question(question: &str) -> bool {
    let q = question.to_lowercase();
    let markers = [
        "cuanto",
        "cuánto",
        "cuantos",
        "cuántos",
        "how many",
        "who",
        "quien",
        "quién",
        "muestrame",
        "muéstrame",
        "show me",
        "lista",
        "listar",
        "this month",
        "this week",
        "este mes",
        "esta semana",
        "entre ",
        " from ",
        "count",
        "conteo",
        "rango",
        "historial",
    ];
    markers.iter().any(|m| q.contains(m))
}

fn build_grounded_knowledge_answer(question: &str, language: &str) -> Option<String> {
    if is_live_data_question(question) {
        return None;
    }

    let ranked = rank_project_knowledge(question);
    let top = ranked.first()?;
    if top.score < 2 {
        return None;
    }

    let mut answer = top.content.to_string();
    if let Some(related) = ranked
        .iter()
        .skip(1)
        .find(|s| s.score >= top.score - 1 && s.title != top.title)
    {
        if language == "en" {
            answer.push_str(&format!(
                "\n\nRelated: {} {}",
                related.title,
                first_sentence(related.content)
            ));
        } else {
            answer.push_str(&format!(
                "\n\nRelacionado: {} {}",
                related.title,
                first_sentence(related.content)
            ));
        }
    }
    Some(answer)
}

fn should_override_llm_answer_with_kb(response: &ChatAskResponse, question: &str) -> bool {
    let ranked = rank_project_knowledge(question);
    let top_score = ranked.first().map(|s| s.score).unwrap_or(0);
    if top_score < 2 || is_live_data_question(question) {
        return false;
    }

    if response.status == "insufficient_data" {
        return true;
    }

    if response.status != "ok" {
        return false;
    }

    let answer = response.answer.to_lowercase();
    let generic_markers = [
        "puedo guiarte paso a paso",
        "opciones frecuentes",
        "información detallada",
        "la información detallada",
        "i can guide you step by step",
        "common options",
        "detailed information is available",
    ];
    generic_markers.iter().any(|m| answer.contains(m))
}

fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let sentence = trimmed.split(". ").next().unwrap_or(trimmed).trim();
    if sentence.ends_with('.') {
        sentence.to_string()
    } else {
        format!("{sentence}.")
    }
}

fn build_knowledge_fallback_answer(question: &str, language: &str) -> Option<String> {
    let ranked = rank_project_knowledge(question);
    if ranked.is_empty() {
        return None;
    }

    let mut answer = if language == "en" {
        "I answered using local project context (fallback mode):\n".to_string()
    } else {
        "Respondí usando contexto local del proyecto (modo fallback):\n".to_string()
    };

    for (i, snippet) in ranked.iter().take(3).enumerate() {
        answer.push_str(&format!(
            "{}. {} [{}]: {}\n",
            i + 1,
            snippet.title,
            snippet.source,
            first_sentence(snippet.content)
        ));
    }

    if language == "en" {
        answer.push_str("\nIf you want, I can continue with exact steps for your specific case.");
    } else {
        answer.push_str("\nSi quieres, continúo con pasos exactos para tu caso.");
    }
    Some(answer)
}

