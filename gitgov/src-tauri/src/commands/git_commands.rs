use crate::audit::AuditDatabase;
use crate::config::{load_config, validate_commit_message};
use crate::git::{
    create_commit, get_file_diff, get_working_tree_changes, has_staged_changes, open_repository,
    stage_files, unstage_all, unstage_files,
};
use crate::models::AuditStatus;
use crate::models::FileChange;
use crate::outbox::{Outbox, OutboxEvent};
use git2::Repository;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tauri::State;

const MAX_STAGE_FILES_EVENT_LIST: usize = 500;

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

fn summarize_stage_files_for_event(files: &[String]) -> (Vec<String>, Option<serde_json::Value>) {
    if files.len() <= MAX_STAGE_FILES_EVENT_LIST {
        return (files.to_vec(), None);
    }

    let preview = files
        .iter()
        .take(MAX_STAGE_FILES_EVENT_LIST)
        .cloned()
        .collect::<Vec<_>>();

    (
        preview,
        Some(serde_json::json!({
            "file_count": files.len(),
            "files_preview_count": MAX_STAGE_FILES_EVENT_LIST,
            "files_truncated": true
        })),
    )
}

fn repo_workdir(repo: &Repository) -> Result<&std::path::Path, String> {
    repo.workdir()
        .ok_or_else(|| to_command_error("Repositorio bare no soportado", "GIT_ERROR"))
}

fn normalize_ignore_rule(rule: &str) -> Option<String> {
    let trimmed = rule.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return None;
    }
    Some(trimmed.to_string())
}

fn resolve_ignore_target_path(repo: &Repository, target: &str) -> Result<std::path::PathBuf, String> {
    let workdir = repo_workdir(repo)?;
    let normalized_target = target.trim().to_lowercase();
    match normalized_target.as_str() {
        "gitignore" => Ok(workdir.join(".gitignore")),
        "gitgovignore" | "gitgov_ignore" => Ok(workdir.join(".gitgovignore")),
        "exclude" | "git_info_exclude" => Ok(repo.path().join("info").join("exclude")),
        _ => Err(to_command_error(
            format!("Target de ignore no soportado: {}", target),
            "VALIDATION_ERROR",
        )),
    }
}

#[tauri::command]
pub fn cmd_apply_ignore_rules(
    repo_path: String,
    target: String,
    rules: Vec<String>,
) -> Result<serde_json::Value, String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;
    let normalized_target = target.trim().to_lowercase();
    let target_path = resolve_ignore_target_path(&repo, &target)?;

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| to_command_error(e, "IO_ERROR"))?;
    }

    let file_existed = target_path.exists();
    let existing_raw = if file_existed {
        fs::read_to_string(&target_path).map_err(|e| to_command_error(e, "IO_ERROR"))?
    } else {
        String::new()
    };

    let mut existing_lines = HashSet::new();
    for line in existing_raw.lines() {
        if let Some(norm) = normalize_ignore_rule(line) {
            existing_lines.insert(norm);
        }
    }

    let mut to_append = Vec::new();
    for rule in rules.iter().filter_map(|r| normalize_ignore_rule(r)) {
        if existing_lines.insert(rule.clone()) {
            to_append.push(rule);
        }
    }

    if !to_append.is_empty() {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target_path)
            .map_err(|e| to_command_error(e, "IO_ERROR"))?;

        let needs_leading_newline = file_existed && !existing_raw.is_empty() && !existing_raw.ends_with('\n');
        if needs_leading_newline {
            writeln!(file).map_err(|e| to_command_error(e, "IO_ERROR"))?;
        }

        for rule in &to_append {
            writeln!(file, "{}", rule).map_err(|e| to_command_error(e, "IO_ERROR"))?;
        }
    }

    Ok(serde_json::json!({
        "target": normalized_target,
        "target_path": target_path.to_string_lossy(),
        "rules_requested": rules.len(),
        "rules_added": to_append.len(),
        "file_existed": file_existed,
    }))
}

#[tauri::command]
pub fn cmd_read_gitgovignore(repo_path: String) -> Result<serde_json::Value, String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;
    let workdir = repo_workdir(&repo)?;
    let target_path = workdir.join(".gitgovignore");

    let exists = target_path.exists();
    let content = if exists {
        fs::read_to_string(&target_path).map_err(|e| to_command_error(e, "IO_ERROR"))?
    } else {
        String::new()
    };

    Ok(serde_json::json!({
        "exists": exists,
        "path": target_path.to_string_lossy(),
        "content": content
    }))
}

#[tauri::command]
pub fn cmd_remove_ignore_rules(
    repo_path: String,
    target: String,
    rules: Vec<String>,
) -> Result<serde_json::Value, String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;
    let target_path = resolve_ignore_target_path(&repo, &target)?;
    let normalized_target = target.trim().to_lowercase();

    let file_existed = target_path.exists();
    if !file_existed {
        return Ok(serde_json::json!({
            "target": normalized_target,
            "target_path": target_path.to_string_lossy(),
            "rules_requested": rules.len(),
            "rules_removed": 0,
            "file_existed": false,
        }));
    }

    let existing_raw = fs::read_to_string(&target_path).map_err(|e| to_command_error(e, "IO_ERROR"))?;
    let to_remove: HashSet<String> = rules.into_iter().filter_map(|r| normalize_ignore_rule(&r)).collect();
    if to_remove.is_empty() {
        return Ok(serde_json::json!({
            "target": normalized_target,
            "target_path": target_path.to_string_lossy(),
            "rules_requested": 0,
            "rules_removed": 0,
            "file_existed": true,
        }));
    }

    let mut removed_count = 0usize;
    let mut kept_lines: Vec<String> = Vec::new();
    for line in existing_raw.lines() {
        if let Some(norm) = normalize_ignore_rule(line) {
            if to_remove.contains(&norm) {
                removed_count += 1;
                continue;
            }
        }
        kept_lines.push(line.to_string());
    }

    let mut rebuilt = kept_lines.join("\n");
    if existing_raw.ends_with('\n') && !rebuilt.is_empty() {
        rebuilt.push('\n');
    }

    fs::write(&target_path, rebuilt).map_err(|e| to_command_error(e, "IO_ERROR"))?;

    Ok(serde_json::json!({
        "target": normalized_target,
        "target_path": target_path.to_string_lossy(),
        "rules_requested": to_remove.len(),
        "rules_removed": removed_count,
        "file_existed": true,
    }))
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
    let (event_files, stage_metadata) = summarize_stage_files_for_event(&files_to_stage);

    match stage_files(&repo, &files_to_stage) {
        Ok(()) => {
            let mut event = OutboxEvent::new(
                "stage_files".to_string(),
                developer_login,
                None,
                AuditStatus::Success,
            )
            .with_files(event_files.clone());

            if let Some(metadata) = stage_metadata.clone() {
                event = event.with_metadata(metadata);
            }

            let _ = outbox.add(event);
            trigger_flush(&outbox);

            Ok(serde_json::json!({
                "staged": files_to_stage,
                "warnings": []
            }))
        }
        Err(e) => {
            let mut event = OutboxEvent::new(
                "stage_files".to_string(),
                developer_login,
                None,
                AuditStatus::Failed,
            )
            .with_files(event_files)
            .with_reason(e.to_string());

            if let Some(metadata) = stage_metadata {
                event = event.with_metadata(metadata);
            }

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
pub fn cmd_unstage_files(repo_path: String, files: Vec<String>) -> Result<(), String> {
    let repo = open_repository(&repo_path).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

    unstage_files(&repo, &files).map_err(|e| to_command_error(e, "GIT_ERROR"))?;

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
