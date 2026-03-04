use crate::github::{
    delete_token, get_authenticated_user, load_token, poll_for_token, save_token, start_device_flow,
};
use crate::models::AuthenticatedUser;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        CommandError {
            code: "ERROR".to_string(),
            message,
        }
    }
}

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&CommandError {
        code: code.to_string(),
        message: e.to_string(),
    })
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

fn current_user_file_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitgov")
        .join("current_user.json")
}

fn save_current_user_session(user: &AuthenticatedUser) -> Result<(), String> {
    let path = current_user_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| to_command_error(e, "SESSION_ERROR"))?;
    }

    let json = serde_json::to_string(user)
        .map_err(|e| to_command_error(e, "SESSION_ERROR"))?;

    std::fs::write(&path, json).map_err(|e| to_command_error(e, "SESSION_ERROR"))?;
    Ok(())
}

fn load_current_user_session() -> Option<AuthenticatedUser> {
    let path = current_user_file_path();
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<AuthenticatedUser>(&json).ok()
}

fn clear_current_user_session() {
    let path = current_user_file_path();
    let _ = std::fs::remove_file(path);
}

#[tauri::command]
pub fn cmd_start_auth() -> Result<DeviceFlowInfo, String> {
    let flow = start_device_flow().map_err(|e| to_command_error(e, "AUTH_ERROR"))?;

    Ok(DeviceFlowInfo {
        user_code: flow.user_code,
        verification_uri: flow.verification_uri,
        device_code: flow.device_code,
        interval: flow.interval,
    })
}

#[tauri::command]
pub fn cmd_poll_auth(device_code: String, interval: u64) -> Result<AuthenticatedUser, String> {
    let token = match poll_for_token(&device_code, interval) {
        Ok(t) => t,
        Err(e) => {
            let error_code = match &e {
                crate::github::AuthError::Pending => "PENDING",
                crate::github::AuthError::SlowDown => "SLOW_DOWN",
                _ => "AUTH_ERROR",
            };
            return Err(to_command_error(e, error_code));
        }
    };

    let gh_user = get_authenticated_user(&token).map_err(|e| to_command_error(e, "API_ERROR"))?;

    tracing::info!(
        login = %gh_user.login,
        "Saving token to keyring for user"
    );

    save_token(&gh_user.login, &token, None).map_err(|e| {
        tracing::error!(error = %e, "Failed to save token to keyring");
        to_command_error(e, "KEYRING_ERROR")
    })?;

    tracing::info!(
        login = %gh_user.login,
        "Token saved successfully"
    );

    let user = AuthenticatedUser {
        login: gh_user.login.clone(),
        name: gh_user.name.unwrap_or_else(|| "Unknown".to_string()),
        avatar_url: gh_user.avatar_url,
        group: None,
        is_admin: false,
    };

    save_current_user_session(&user)?;

    Ok(user)
}

#[tauri::command]
pub fn cmd_get_current_user() -> Result<Option<AuthenticatedUser>, String> {
    if let Some(session_user) = load_current_user_session() {
        if let Ok(token) = load_token(&session_user.login) {
            if let Ok(gh_user) = get_authenticated_user(&token) {
                let refreshed_user = AuthenticatedUser {
                    login: gh_user.login,
                    name: gh_user.name.unwrap_or(session_user.name),
                    avatar_url: gh_user.avatar_url,
                    group: session_user.group,
                    is_admin: session_user.is_admin,
                };
                let _ = save_current_user_session(&refreshed_user);
                return Ok(Some(refreshed_user));
            }
        }

        // Session exists but token is invalid/expired; clear local session marker.
        clear_current_user_session();
    }

    Ok(None)
}

#[tauri::command]
pub fn cmd_set_current_user(
    login: String,
    name: String,
    avatar_url: String,
    group: Option<String>,
    is_admin: bool,
) -> Result<(), String> {
    let user = AuthenticatedUser {
        login,
        name,
        avatar_url,
        group,
        is_admin,
    };
    save_current_user_session(&user)
}

#[tauri::command]
pub fn cmd_logout(username: String) -> Result<(), String> {
    if let Err(e) = delete_token(&username) {
        tracing::warn!(username = %username, error = %e, "Token cleanup failed during logout");
    }
    clear_current_user_session();
    Ok(())
}

#[tauri::command]
pub fn cmd_validate_token(token: String) -> Result<AuthenticatedUser, String> {
    let gh_user = get_authenticated_user(&token).map_err(|e| to_command_error(e, "API_ERROR"))?;

    Ok(AuthenticatedUser {
        login: gh_user.login,
        name: gh_user.name.unwrap_or_else(|| "Unknown".to_string()),
        avatar_url: gh_user.avatar_url,
        group: None,
        is_admin: false,
    })
}

#[tauri::command]
pub fn cmd_open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(to_command_error("URL vacía", "INVALID_URL"));
    }
    let parsed =
        reqwest::Url::parse(trimmed).map_err(|e| to_command_error(e, "INVALID_URL"))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(to_command_error(
            "Solo se permiten URLs http/https",
            "INVALID_URL",
        ));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", parsed.as_str()])
            .spawn()
            .map_err(|e| to_command_error(e, "OPEN_URL_ERROR"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(parsed.as_str())
            .spawn()
            .map_err(|e| to_command_error(e, "OPEN_URL_ERROR"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(parsed.as_str())
            .spawn()
            .map_err(|e| to_command_error(e, "OPEN_URL_ERROR"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(to_command_error(
        "Plataforma no soportada para abrir enlaces externos",
        "OPEN_URL_ERROR",
    ))
}

pub fn get_token_for_user(username: &str) -> Option<String> {
    tracing::debug!(username = %username, "Attempting to load token from keyring");
    match load_token(username) {
        Ok(token) => {
            tracing::debug!(username = %username, "Token loaded successfully");
            Some(token)
        }
        Err(crate::github::AuthError::TokenExpired) => {
            tracing::warn!(username = %username, "Token expired");
            None
        }
        Err(e) => {
            tracing::warn!(username = %username, error = %e, "Failed to load token");
            None
        }
    }
}
