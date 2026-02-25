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

fn is_keyring_no_entry_error(error: &keyring::Error) -> bool {
    match error {
        #[allow(unreachable_patterns)]
        keyring::Error::NoEntry => true,
        _ => {
            let msg = error.to_string().to_ascii_lowercase();
            msg.contains("no matching entry found")
                || msg.contains("no entry")
                || msg.contains("credential not found")
        }
    }
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

    // Compatibility hotfix:
    // Keep a local backup file in addition to keyring so existing Windows setups do not lose push
    // capability if the keyring backend is unavailable/intermittent.
    let keyring_result: Result<(), AuthError> = (|| {
        let entry = keyring::Entry::new("gitgov", username)
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;
        entry
            .set_password(&json)
            .map_err(|e| AuthError::KeyringError(e.to_string()))?;
        Ok(())
    })();

    let file_result = save_legacy_token_to_file(username, &json);

    match (&keyring_result, &file_result) {
        (Ok(_), Ok(_)) => {
            tracing::info!(username = %username, "Token saved to keyring and local backup file");
        }
        (Ok(_), Err(e)) => {
            tracing::warn!(username = %username, error = %e, "Token saved to keyring, but local backup file save failed");
        }
        (Err(e), Ok(_)) => {
            tracing::warn!(username = %username, error = %e, "Token saved to local backup file (keyring save failed)");
        }
        (Err(ke), Err(fe)) => {
            tracing::error!(username = %username, keyring_error = %ke, file_error = %fe, "Failed to save token to keyring and local backup file");
        }
    }

    if keyring_result.is_ok() || file_result.is_ok() {
        Ok(())
    } else {
        Err(keyring_result.unwrap_err())
    }
}

fn save_legacy_token_to_file(username: &str, json: &str) -> Result<(), AuthError> {
    let token_file = legacy_token_file_path(username);
    if let Some(parent) = token_file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AuthError::KeyringError(format!("Failed to create token dir: {}", e)))?;
    }
    std::fs::write(&token_file, json)
        .map_err(|e| AuthError::KeyringError(format!("Failed to write token file: {}", e)))
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
                if is_keyring_no_entry_error(&e) {
                    tracing::debug!(username = %username, "No token found in keyring for user");
                    AuthError::TokenNotFound
                } else {
                    tracing::error!(username = %username, error = %e, "Failed to get password from keyring");
                    AuthError::KeyringError(format!("Failed to get password: {}", e))
                }
            })?;

        tracing::debug!(username = %username, "Token loaded from keyring");
        Ok(json)
    })();

    // If keyring fails, fall back to the local backup file (compatibility path).
    let (json, from_keyring) = match keyring_result {
        Ok(json) => (json, true),
        Err(AuthError::TokenNotFound) => {
            tracing::debug!(username = %username, "Token not present in keyring, trying local backup file");
            let legacy_json = load_legacy_token_from_file(username)?;
            if !legacy_json.trim().is_empty() {
                tracing::info!(username = %username, "Local backup token found. Rehydrating keyring from backup file.");
            }
            (legacy_json, false)
        }
        Err(keyring_err) => {
            tracing::warn!(username = %username, keyring_error = %keyring_err, "Keyring failed, trying legacy token file migration");
            let legacy_json = load_legacy_token_from_file(username)?;
            tracing::warn!(username = %username, "Legacy/local token file detected. Rehydrating keyring from backup file.");
            (legacy_json, false)
        }
    };

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        if stored.is_expired() {
            if from_keyring {
                if let Ok(fallback_json) = load_legacy_token_from_file(username) {
                    if let Ok(fallback_stored) = serde_json::from_str::<StoredToken>(&fallback_json) {
                        if !fallback_stored.is_expired() {
                            let _ = save_token(
                                username,
                                &fallback_stored.access_token,
                                Some(fallback_stored.expires_at - fallback_stored.created_at),
                            );
                            tracing::warn!(username = %username, "Keyring token expired; recovered valid token from local backup file");
                            return Ok(fallback_stored.access_token);
                        }
                    } else if !fallback_json.trim().is_empty() {
                        let plain = fallback_json.trim().to_string();
                        let _ = save_token(username, &plain, None);
                        tracing::warn!(username = %username, "Keyring token expired; recovered plain token from local backup file");
                        return Ok(plain);
                    }
                }
            }
            return Err(AuthError::TokenExpired);
        }
        // If source was backup file, re-save to keyring (and keep backup for compatibility).
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
    let keyring_load_result = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))
        .and_then(|entry| {
            entry.get_password().map_err(|e| {
                if is_keyring_no_entry_error(&e) {
                    AuthError::TokenNotFound
                } else {
                    AuthError::KeyringError(e.to_string())
                }
            })
        });

    let (json, from_keyring) = match keyring_load_result {
        Ok(json) => (json, true),
        Err(AuthError::TokenNotFound) => {
            tracing::debug!(username = %username, "Token not present in keyring, attempting local backup token");
            let legacy_json = load_legacy_token_from_file(username)?;
            (legacy_json, false)
        }
        Err(keyring_err) => {
            tracing::warn!(username = %username, keyring_error = %keyring_err, "Keyring token unavailable, attempting legacy token migration");
            let legacy_json = load_legacy_token_from_file(username)?;
            (legacy_json, false)
        }
    };

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        if from_keyring && stored.is_expired() {
            if let Ok(fallback_json) = load_legacy_token_from_file(username) {
                if let Ok(fallback_stored) = serde_json::from_str::<StoredToken>(&fallback_json) {
                    if !fallback_stored.is_expired() {
                        let _ = save_token(
                            username,
                            &fallback_stored.access_token,
                            Some(fallback_stored.expires_at - fallback_stored.created_at),
                        );
                        return Ok(fallback_stored);
                    }
                } else if !fallback_json.trim().is_empty() {
                    let fallback_stored = StoredToken::new(fallback_json.trim().to_string(), None);
                    let _ = save_token(
                        username,
                        &fallback_stored.access_token,
                        Some(fallback_stored.expires_at - fallback_stored.created_at),
                    );
                    return Ok(fallback_stored);
                }
            }
        }
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
                if is_keyring_no_entry_error(&e) {
                    tracing::debug!(username = %username, "No keyring token found to delete");
                } else {
                    tracing::warn!(username = %username, error = %e, "Failed to delete token from keyring");
                    keyring_error = Some(AuthError::KeyringError(e.to_string()));
                }
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
