use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const GITHUB_CLIENT_ID: &str = "Ov23livabbc30nXBY0KF";
const TOKEN_EXPIRATION_SECONDS: i64 = 28 * 24 * 60 * 60; // 28 days (GitHub default)

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Access denied")]
    AccessDenied,
    #[error("Keyring error: {0}")]
    KeyringError(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Authorization pending")]
    Pending,
    #[error("Slow down")]
    SlowDown,
    #[error("Token not found")]
    TokenNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
    pub expires_in: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl StoredToken {
    pub fn new(access_token: String, expires_in: Option<i64>) -> Self {
        let now = chrono::Utc::now().timestamp();
        let expiration = expires_in.unwrap_or(TOKEN_EXPIRATION_SECONDS);
        Self {
            access_token,
            created_at: now,
            expires_at: now + expiration,
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at - 300 // 5 minute buffer
    }

    pub fn needs_refresh(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at - 3600 // 1 hour buffer
    }
}

pub fn start_device_flow() -> Result<DeviceFlowResponse, AuthError> {
    let client = Client::new();

    let response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("User-Agent", "GitGov/1.0")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", "repo user")])
        .send()
        .map_err(|e| AuthError::NetworkError(e.to_string()))?;

    let flow_response: DeviceFlowResponse = response
        .json()
        .map_err(|e| AuthError::NetworkError(e.to_string()))?;

    Ok(flow_response)
}

pub fn poll_for_token(device_code: &str, interval: u64) -> Result<String, AuthError> {
    std::thread::sleep(std::time::Duration::from_secs(interval));

    let client = Client::new();

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "GitGov/1.0")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .map_err(|e| AuthError::NetworkError(e.to_string()))?;

    let token_response: TokenResponse = response
        .json()
        .map_err(|e| AuthError::NetworkError(e.to_string()))?;

    if let Some(error) = token_response.error {
        match error.as_str() {
            "authorization_pending" => return Err(AuthError::Pending),
            "slow_down" => return Err(AuthError::SlowDown),
            "expired_token" => return Err(AuthError::TokenExpired),
            "access_denied" => return Err(AuthError::AccessDenied),
            _ => return Err(AuthError::NetworkError(error)),
        }
    }

    token_response.access_token.ok_or(AuthError::Unauthorized)
}

pub fn poll_and_save_token(
    device_code: &str,
    interval: u64,
    username: &str,
) -> Result<String, AuthError> {
    std::thread::sleep(std::time::Duration::from_secs(interval));

    let client = Client::new();

    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "GitGov/1.0")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .map_err(|e| AuthError::NetworkError(e.to_string()))?;

    let token_response: TokenResponse = response
        .json()
        .map_err(|e| AuthError::NetworkError(e.to_string()))?;

    if let Some(error) = token_response.error {
        match error.as_str() {
            "authorization_pending" => return Err(AuthError::Pending),
            "slow_down" => return Err(AuthError::SlowDown),
            "expired_token" => return Err(AuthError::TokenExpired),
            "access_denied" => return Err(AuthError::AccessDenied),
            _ => return Err(AuthError::NetworkError(error)),
        }
    }

    let access_token = token_response.access_token.ok_or(AuthError::Unauthorized)?;
    save_token(username, &access_token, token_response.expires_in)?;

    Ok(access_token)
}

pub fn save_token(username: &str, token: &str, expires_in: Option<i64>) -> Result<(), AuthError> {
    let stored = StoredToken::new(token.to_string(), expires_in);
    let json =
        serde_json::to_string(&stored).map_err(|e| AuthError::KeyringError(e.to_string()))?;

    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    entry
        .set_password(&json)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    // Cleanup insecure legacy token file if it exists.
    if let Err(e) = delete_legacy_token_file(username) {
        tracing::warn!(username = %username, error = %e, "Token saved to keyring but failed to delete legacy token file");
    }

    tracing::info!(username = %username, "Token saved to keyring");
    Ok(())
}

fn legacy_token_file_path(username: &str) -> std::path::PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gitgov");
    data_dir.join(format!("{}.token", username))
}

fn load_legacy_token_from_file(username: &str) -> Result<String, AuthError> {
    let token_file = legacy_token_file_path(username);
    std::fs::read_to_string(&token_file)
        .map_err(|e| AuthError::KeyringError(format!("Failed to read token file: {}", e)))
}

fn delete_legacy_token_file(username: &str) -> Result<(), AuthError> {
    let token_file = legacy_token_file_path(username);
    if !token_file.exists() {
        return Ok(());
    }
    std::fs::remove_file(&token_file)
        .map_err(|e| AuthError::KeyringError(format!("Failed to delete token file: {}", e)))
}

pub fn load_token(username: &str) -> Result<String, AuthError> {
    tracing::debug!(username = %username, "Attempting to load token");

    // Try keyring first
    let keyring_result: Result<String, AuthError> = (|| {
        let entry = keyring::Entry::new("gitgov", username).map_err(|e| {
            tracing::error!(username = %username, error = %e, "Failed to create keyring entry");
            AuthError::KeyringError(format!("Failed to create keyring entry: {}", e))
        })?;

        let json = entry
            .get_password()
            .map_err(|e| {
                tracing::error!(username = %username, error = %e, "Failed to get password from keyring");
                AuthError::KeyringError(format!("Failed to get password: {}", e))
            })?;

        tracing::debug!(username = %username, "Token loaded from keyring");
        Ok(json)
    })();

    // If keyring fails, try one-time migration from legacy file (insecure old behavior).
    let json = match keyring_result {
        Ok(json) => json,
        Err(keyring_err) => {
            tracing::warn!(username = %username, keyring_error = %keyring_err, "Keyring failed, trying legacy token file migration");
            let legacy_json = load_legacy_token_from_file(username)?;
            tracing::warn!(username = %username, "Legacy token file detected. Migrating token to keyring and deleting file.");
            legacy_json
        }
    };

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        if stored.is_expired() {
            let _ = delete_legacy_token_file(username);
            return Err(AuthError::TokenExpired);
        }
        // If source was legacy file, this re-saves to keyring and deletes file.
        let _ = save_token(username, &stored.access_token, Some(stored.expires_at - stored.created_at));
        tracing::info!(username = %username, "Token loaded and valid");
        return Ok(stored.access_token);
    }

    // Fallback: old format (plain token)
    let _ = save_token(username, &json, None);
    tracing::info!(username = %username, "Token loaded (plain format)");
    Ok(json)
}

pub fn load_token_with_expiry(username: &str) -> Result<StoredToken, AuthError> {
    let json = match keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))
        .and_then(|entry| entry.get_password().map_err(|e| AuthError::KeyringError(e.to_string())))
    {
        Ok(json) => json,
        Err(keyring_err) => {
            tracing::warn!(username = %username, keyring_error = %keyring_err, "Keyring token unavailable, attempting legacy token migration");
            let legacy_json = load_legacy_token_from_file(username)?;
            legacy_json
        }
    };

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        let _ = save_token(username, &stored.access_token, Some(stored.expires_at - stored.created_at));
        return Ok(stored);
    }

    // Fallback: old format (plain token) - assume no expiration
    let stored = StoredToken::new(json, None);
    let _ = save_token(username, &stored.access_token, Some(stored.expires_at - stored.created_at));
    Ok(stored)
}

pub fn check_token_valid(username: &str) -> Result<TokenStatus, AuthError> {
    let stored = load_token_with_expiry(username)?;

    Ok(TokenStatus {
        valid: !stored.is_expired(),
        needs_refresh: stored.needs_refresh(),
        expires_at: stored.expires_at,
    })
}

#[derive(Debug, Clone)]
pub struct TokenStatus {
    pub valid: bool,
    pub needs_refresh: bool,
    pub expires_at: i64,
}

pub fn delete_token(username: &str) -> Result<(), AuthError> {
    let mut keyring_error: Option<AuthError> = None;

    match keyring::Entry::new("gitgov", username) {
        Ok(entry) => {
            if let Err(e) = entry.delete_credential() {
                tracing::warn!(username = %username, error = %e, "Failed to delete token from keyring");
                keyring_error = Some(AuthError::KeyringError(e.to_string()));
            }
        }
        Err(e) => {
            tracing::warn!(username = %username, error = %e, "Failed to open keyring entry for deletion");
            keyring_error = Some(AuthError::KeyringError(e.to_string()));
        }
    }

    let file_delete_result = delete_legacy_token_file(username);
    if let Err(e) = &file_delete_result {
        tracing::warn!(username = %username, error = %e, "Failed to delete legacy token file");
    }

    match (keyring_error, file_delete_result) {
        (None, _) => Ok(()),
        (Some(_), Ok(())) => Ok(()),
        (Some(err), Err(_)) => Err(err),
    }
}
