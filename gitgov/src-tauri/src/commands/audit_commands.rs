use crate::audit::AuditDatabase;
use crate::models::{AuditFilter, AuditLogEntry, AuditStats};
use std::sync::Arc;
use tauri::State;

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "message": e.to_string()
    }))
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

#[tauri::command]
pub fn cmd_get_audit_logs(
    filter: AuditFilter,
    _is_admin: bool,
    audit_db: State<'_, Arc<AuditDatabase>>,
) -> Result<Vec<AuditLogEntry>, String> {
    if !_is_admin {
        return Err(to_command_error(
            "UNAUTHORIZED",
            "Solo administradores pueden ver todos los logs",
        ));
    }

    let logs = audit_db
        .query(&filter)
        .map_err(|e| to_command_error(e, "DB_ERROR"))?;

    Ok(logs)
}

#[tauri::command]
pub fn cmd_get_audit_stats(audit_db: State<'_, Arc<AuditDatabase>>) -> Result<AuditStats, String> {
    let stats = audit_db
        .get_stats()
        .map_err(|e| to_command_error(e, "DB_ERROR"))?;

    Ok(stats)
}

#[tauri::command]
pub fn cmd_get_my_logs(
    developer_login: String,
    limit: usize,
    audit_db: State<'_, Arc<AuditDatabase>>,
) -> Result<Vec<AuditLogEntry>, String> {
    let filter = AuditFilter {
        developer_login: Some(developer_login),
        limit,
        ..Default::default()
    };

    let logs = audit_db
        .query(&filter)
        .map_err(|e| to_command_error(e, "DB_ERROR"))?;

    Ok(logs)
}
