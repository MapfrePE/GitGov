use crate::audit::AuditDatabase;
use crate::github::{
    delete_token, get_authenticated_user, load_token, poll_for_token, save_token,
    start_device_flow, DeviceFlowResponse,
};
use crate::models::AuthenticatedUser;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

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

    Ok(AuthenticatedUser {
        login: gh_user.login.clone(),
        name: gh_user.name.unwrap_or_else(|| "Unknown".to_string()),
        avatar_url: gh_user.avatar_url,
        group: None,
        is_admin: false,
    })
}

#[tauri::command]
pub fn cmd_get_current_user() -> Result<Option<AuthenticatedUser>, String> {
    let stored_login: Option<String> = None;

    if let Some(login) = stored_login {
        if let Ok(token) = load_token(&login) {
            if let Ok(gh_user) = get_authenticated_user(&token) {
                return Ok(Some(AuthenticatedUser {
                    login: gh_user.login,
                    name: gh_user.name.unwrap_or_else(|| "Unknown".to_string()),
                    avatar_url: gh_user.avatar_url,
                    group: None,
                    is_admin: false,
                }));
            }
        }
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
    let _ = (login, name, avatar_url, group, is_admin);
    Ok(())
}

#[tauri::command]
pub fn cmd_logout(username: String) -> Result<(), String> {
    delete_token(&username).map_err(|e| to_command_error(e, "KEYRING_ERROR"))?;
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
