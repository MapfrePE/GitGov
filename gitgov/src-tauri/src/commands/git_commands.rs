use crate::audit::AuditDatabase;
use crate::config::{load_config, validate_commit_message};
use crate::git::{
    create_commit, get_file_diff, get_working_tree_changes, has_staged_changes, open_repository,
    stage_files, unstage_all,
};
use crate::models::AuditStatus;
use crate::models::FileChange;
use crate::outbox::{Outbox, OutboxEvent};
use std::sync::Arc;
use tauri::State;

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "message": e.to_string()
    }))
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

fn trigger_flush(outbox: &Arc<Outbox>) {
    let outbox_clone = Arc::clone(outbox);
    std::thread::spawn(move || {
        let _ = outbox_clone.flush();
    });
}

#[tauri::command]
pub fn cmd_get_status(repo_path: String) -> Result<Vec<FileChange>, String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    let changes = get_working_tree_changes(&repo).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    Ok(changes)
}

#[tauri::command]
pub fn cmd_get_file_diff(repo_path: String, file_path: String) -> Result<String, String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    let diff = crate::git::get_file_diff(&repo, &file_path)
        .map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    Ok(diff)
}

#[tauri::command]
pub fn cmd_stage_files(
    repo_path: String,
    files: Vec<String>,
    developer_login: String,
    outbox: State<'_, Arc<Outbox>>,
) -> Result<serde_json::Value, String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    let files_to_stage: Vec<String> = files.clone();

    match stage_files(&repo, &files_to_stage) {
        Ok(()) => {
            let event = OutboxEvent::new(
                "stage_files".to_string(),
                developer_login,
                None,
                AuditStatus::Success,
            )
            .with_files(files_to_stage.clone());

            let _ = outbox.add(event);
            trigger_flush(&outbox);

            Ok(serde_json::json!({
                "staged": files_to_stage,
                "warnings": []
            }))
        }
        Err(e) => {
            let event = OutboxEvent::new(
                "stage_files".to_string(),
                developer_login,
                None,
                AuditStatus::Failed,
            )
            .with_files(files_to_stage)
            .with_reason(e.to_string());

            let _ = outbox.add(event);
            trigger_flush(&outbox);

            Err(to_command_error(e, "GIT_ERROR"))
        }
    }
}

#[tauri::command]
pub fn cmd_unstage_all(repo_path: String) -> Result<(), String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    unstage_all(&repo).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    Ok(())
}

#[tauri::command]
pub fn cmd_commit(
    repo_path: String,
    message: String,
    author_name: String,
    author_email: String,
    developer_login: String,
    outbox: State<'_, Arc<Outbox>>,
) -> Result<String, String> {
    let validation = validate_commit_message(&message);
    if !validation.valid {
        return Err(to_command_error(
            validation
                .error
                .unwrap_or_else(|| "Invalid commit message".to_string()),
            "VALIDATION_ERROR",
        ));
    }

    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    let has_staged = has_staged_changes(&repo).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    if !has_staged {
        return Err(to_command_error(
            "No hay archivos en el staging area. Selecciona al menos un archivo antes de commitear.",
            "EMPTY_STAGING"
        ));
    }

    let current_branch = crate::git::get_current_branch(&repo).ok();

    match create_commit(&repo, &message, &author_name, &author_email) {
        Ok(commit_hash) => {
            let event = OutboxEvent::new(
                "commit".to_string(),
                developer_login,
                current_branch,
                AuditStatus::Success,
            )
            .with_commit_sha(commit_hash.clone())
            .with_metadata(serde_json::json!({
                "commit_message": message.clone()
            }))
            .with_user_name(author_name);

            let _ = outbox.add(event);
            trigger_flush(&outbox);

            Ok(commit_hash)
        }
        Err(e) => {
            let event = OutboxEvent::new(
                "commit".to_string(),
                developer_login,
                current_branch,
                AuditStatus::Failed,
            )
            .with_metadata(serde_json::json!({
                "commit_message": message.clone()
            }))
            .with_reason(e.to_string());

            let _ = outbox.add(event);
            trigger_flush(&outbox);

            Err(to_command_error(e, "GIT_ERROR"))
        }
    }
}

#[tauri::command]
pub fn cmd_push(
    repo_path: String,
    branch: String,
    developer_login: String,
    audit_db: State<'_, Arc<AuditDatabase>>,
    outbox: State<'_, Arc<Outbox>>,
) -> Result<(), String> {
    use crate::git::push_to_remote;
    use crate::models::{AuditAction, AuditLogEntry};
    use uuid::Uuid;

    let attempt_event = OutboxEvent::new(
        "attempt_push".to_string(),
        developer_login.clone(),
        Some(branch.clone()),
        AuditStatus::Success,
    );
    let attempt_uuid = outbox.add(attempt_event).ok();

    let config = load_config(&repo_path);

    if let Ok(cfg) = &config {
        if cfg.branches.protected.iter().any(|p| p == &branch) {
            if let Some(uuid) = &attempt_uuid {
                let blocked_event = OutboxEvent::new(
                    "blocked_push".to_string(),
                    developer_login.clone(),
                    Some(branch.clone()),
                    AuditStatus::Blocked,
                )
                .with_reason(format!("Rama protegida: {}", branch));
                let _ = outbox.add(blocked_event);
            }

            let entry = AuditLogEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                developer_login: developer_login.clone(),
                developer_name: developer_login.clone(),
                action: AuditAction::BlockedPush,
                branch: branch.clone(),
                files: vec![],
                commit_hash: None,
                status: AuditStatus::Blocked,
                reason: Some(format!("Rama protegida: {}", branch)),
            };
            let _ = audit_db.insert(&entry);

            trigger_flush(&outbox);

            return Err(to_command_error(
                format!("Rama protegida. No puedes hacer push a '{}'.", branch),
                "BLOCKED",
            ));
        }
    }

    let token = match crate::commands::get_token_for_user(&developer_login) {
        Some(t) => t,
        None => {
            tracing::error!(
                developer_login = %developer_login,
                "Token not found in keyring for user"
            );
            trigger_flush(&outbox);
            return Err(to_command_error(
                format!("No hay token guardado para el usuario '{}'. Intenta re-autenticarte cerrando y abriendo la app.", developer_login),
                "AUTH_ERROR",
            ));
        }
    };

    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    match push_to_remote(&repo, &branch, &token) {
        Ok(()) => {
            let success_event = OutboxEvent::new(
                "successful_push".to_string(),
                developer_login.clone(),
                Some(branch.clone()),
                AuditStatus::Success,
            );
            let _ = outbox.add(success_event);

            let entry = AuditLogEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                developer_login: developer_login.clone(),
                developer_name: developer_login.clone(),
                action: AuditAction::Push,
                branch: branch.clone(),
                files: vec![],
                commit_hash: None,
                status: AuditStatus::Success,
                reason: None,
            };
            let _ = audit_db.insert(&entry);

            trigger_flush(&outbox);

            Ok(())
        }
        Err(e) => {
            let status = if e.to_string().contains("rejected") || e.to_string().contains("conflict")
            {
                AuditStatus::Failed
            } else {
                AuditStatus::Blocked
            };

            let event = OutboxEvent::new(
                "push_failed".to_string(),
                developer_login.clone(),
                Some(branch.clone()),
                status.clone(),
            )
            .with_reason(e.to_string());
            let _ = outbox.add(event);

            let entry = AuditLogEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                developer_login: developer_login.clone(),
                developer_name: developer_login.clone(),
                action: AuditAction::Push,
                branch: branch.clone(),
                files: vec![],
                commit_hash: None,
                status,
                reason: Some(e.to_string()),
            };
            let _ = audit_db.insert(&entry);

            trigger_flush(&outbox);

            Err(to_command_error(e, "PUSH_ERROR"))
        }
    }
}
