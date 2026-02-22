use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const GITHUB_CLIENT_ID: &str = "YOUR_GITHUB_CLIENT_ID";
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

    Ok(())
}

pub fn load_token(username: &str) -> Result<String, AuthError> {
    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    let json = entry
        .get_password()
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        if stored.is_expired() {
            return Err(AuthError::TokenExpired);
        }
        return Ok(stored.access_token);
    }

    // Fallback: old format (plain token)
    Ok(json)
}

pub fn load_token_with_expiry(username: &str) -> Result<StoredToken, AuthError> {
    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    let json = entry
        .get_password()
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        return Ok(stored);
    }

    // Fallback: old format (plain token) - assume no expiration
    Ok(StoredToken::new(json, None))
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
    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    entry
        .delete_credential()
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;

    Ok(())
}
