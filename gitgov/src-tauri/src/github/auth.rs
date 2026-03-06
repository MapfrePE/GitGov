use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const GITHUB_CLIENT_ID: &str = "Ov23livabbc30nXBY0KF";
const TOKEN_EXPIRATION_SECONDS: i64 = 28 * 24 * 60 * 60; // 28 days (GitHub default)
const GITHUB_DEVICE_FLOW_SCOPE: &str = "repo user workflow";
const LEGACY_TOKEN_FILE_COMPAT_ENV: &str = "GITGOV_ALLOW_LEGACY_TOKEN_FILE";
const LEGACY_TOKEN_DIR_ENV: &str = "GITGOV_LEGACY_TOKEN_DIR";
const SIMULATE_KEYRING_FAILURE_ENV: &str = "GITGOV_SIMULATE_KEYRING_FAILURE";
const SIMULATE_KEYRING_MEMORY_ENV: &str = "GITGOV_SIMULATE_KEYRING_MEMORY";

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

#[derive(Debug, Clone, Default)]
pub struct LegacyTokenMigrationReport {
    pub scanned_files: usize,
    pub migrated_tokens: usize,
    pub skipped_files: usize,
    pub failed_files: usize,
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

fn parse_bool_like(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn legacy_token_file_compat_enabled() -> bool {
    std::env::var(LEGACY_TOKEN_FILE_COMPAT_ENV)
        .ok()
        .as_deref()
        .map(parse_bool_like)
        .unwrap_or(false)
}

fn keyring_failure_simulation_enabled() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    std::env::var(SIMULATE_KEYRING_FAILURE_ENV)
        .ok()
        .as_deref()
        .map(parse_bool_like)
        .unwrap_or(false)
}

fn keyring_memory_simulation_enabled() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    std::env::var(SIMULATE_KEYRING_MEMORY_ENV)
        .ok()
        .as_deref()
        .map(parse_bool_like)
        .unwrap_or(false)
}

fn simulated_keyring_store() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>>
{
    static STORE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        std::sync::OnceLock::new();
    STORE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

#[cfg(test)]
fn clear_simulated_keyring_store() {
    if let Ok(mut store) = simulated_keyring_store().lock() {
        store.clear();
    }
}

fn keyring_set_password(username: &str, json: &str) -> Result<(), AuthError> {
    if keyring_memory_simulation_enabled() {
        let mut store = simulated_keyring_store()
            .lock()
            .map_err(|_| AuthError::KeyringError("Simulated keyring lock poisoned".to_string()))?;
        store.insert(username.to_string(), json.to_string());
        return Ok(());
    }
    if keyring_failure_simulation_enabled() {
        return Err(AuthError::KeyringError(
            "Simulated keyring failure".to_string(),
        ));
    }
    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;
    entry
        .set_password(json)
        .map_err(|e| AuthError::KeyringError(e.to_string()))
}

fn keyring_get_password(username: &str) -> Result<String, AuthError> {
    if keyring_memory_simulation_enabled() {
        let store = simulated_keyring_store()
            .lock()
            .map_err(|_| AuthError::KeyringError("Simulated keyring lock poisoned".to_string()))?;
        return store.get(username).cloned().ok_or(AuthError::TokenNotFound);
    }
    if keyring_failure_simulation_enabled() {
        return Err(AuthError::KeyringError(
            "Simulated keyring failure".to_string(),
        ));
    }
    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;
    entry.get_password().map_err(|e| {
        if is_keyring_no_entry_error(&e) {
            AuthError::TokenNotFound
        } else {
            AuthError::KeyringError(format!("Failed to get password: {}", e))
        }
    })
}

fn keyring_delete_password(username: &str) -> Result<(), AuthError> {
    if keyring_memory_simulation_enabled() {
        let mut store = simulated_keyring_store()
            .lock()
            .map_err(|_| AuthError::KeyringError("Simulated keyring lock poisoned".to_string()))?;
        store.remove(username);
        return Ok(());
    }
    if keyring_failure_simulation_enabled() {
        return Err(AuthError::KeyringError(
            "Simulated keyring failure".to_string(),
        ));
    }
    let entry = keyring::Entry::new("gitgov", username)
        .map_err(|e| AuthError::KeyringError(e.to_string()))?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(e) if is_keyring_no_entry_error(&e) => Ok(()),
        Err(e) => Err(AuthError::KeyringError(e.to_string())),
    }
}

fn legacy_token_base_dir() -> std::path::PathBuf {
    if let Ok(raw) = std::env::var(LEGACY_TOKEN_DIR_ENV) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gitgov")
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
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("scope", GITHUB_DEVICE_FLOW_SCOPE),
        ])
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

    keyring_set_password(username, &json)?;

    if legacy_token_file_compat_enabled() {
        match save_legacy_token_to_file(username, &json) {
            Ok(_) => {
                tracing::warn!(
                    username = %username,
                    "Token saved to keyring and legacy backup file (compat mode enabled)"
                );
            }
            Err(e) => {
                tracing::warn!(
                    username = %username,
                    error = %e,
                    "Token saved to keyring, but legacy backup file save failed"
                );
            }
        }
    } else {
        if let Err(e) = delete_legacy_token_file(username) {
            tracing::warn!(
                username = %username,
                error = %e,
                "Token saved to keyring, but legacy token file cleanup failed"
            );
        }
    }

    tracing::info!(username = %username, "Token saved to keyring");
    Ok(())
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
    legacy_token_base_dir().join(format!("{}.token", username))
}

fn load_legacy_token_from_file(username: &str) -> Result<String, AuthError> {
    let token_file = legacy_token_file_path(username);
    match std::fs::read_to_string(&token_file) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AuthError::TokenNotFound),
        Err(e) => Err(AuthError::KeyringError(format!(
            "Failed to read token file: {}",
            e
        ))),
    }
}

fn delete_legacy_token_file(username: &str) -> Result<(), AuthError> {
    let token_file = legacy_token_file_path(username);
    if !token_file.exists() {
        return Ok(());
    }
    std::fs::remove_file(&token_file)
        .map_err(|e| AuthError::KeyringError(format!("Failed to delete token file: {}", e)))
}

pub fn migrate_legacy_tokens_from_disk() -> LegacyTokenMigrationReport {
    let mut report = LegacyTokenMigrationReport::default();
    let legacy_dir = legacy_token_base_dir();
    let entries = match std::fs::read_dir(&legacy_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return report,
        Err(e) => {
            tracing::warn!(
                path = %legacy_dir.display(),
                error = %e,
                "Failed to read legacy token directory"
            );
            report.failed_files += 1;
            return report;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                report.failed_files += 1;
                tracing::warn!(error = %e, "Failed to read entry in legacy token directory");
                continue;
            }
        };
        let path = entry.path();
        let is_token_file = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("token"));
        if !is_token_file {
            continue;
        }
        report.scanned_files += 1;

        let username = match path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            Some(name) => name.to_string(),
            None => {
                report.skipped_files += 1;
                tracing::warn!(
                    path = %path.display(),
                    "Skipping legacy token file with invalid username stem"
                );
                continue;
            }
        };

        let raw = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                report.failed_files += 1;
                tracing::warn!(
                    path = %path.display(),
                    username = %username,
                    error = %e,
                    "Failed to read legacy token file"
                );
                continue;
            }
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            report.skipped_files += 1;
            tracing::warn!(
                path = %path.display(),
                username = %username,
                "Skipping empty legacy token file"
            );
            continue;
        }

        let (token, expires_in) = match serde_json::from_str::<StoredToken>(trimmed) {
            Ok(stored) => (
                stored.access_token,
                Some(stored.expires_at.saturating_sub(stored.created_at)),
            ),
            Err(_) => (trimmed.to_string(), None),
        };

        match save_token(&username, &token, expires_in) {
            Ok(_) => {
                report.migrated_tokens += 1;
                tracing::info!(
                    username = %username,
                    path = %path.display(),
                    "Migrated legacy token file to keyring"
                );
            }
            Err(e) => {
                report.failed_files += 1;
                tracing::warn!(
                    username = %username,
                    path = %path.display(),
                    error = %e,
                    "Failed to migrate legacy token file"
                );
            }
        }
    }

    if report.scanned_files > 0 {
        tracing::info!(
            scanned_files = report.scanned_files,
            migrated_tokens = report.migrated_tokens,
            skipped_files = report.skipped_files,
            failed_files = report.failed_files,
            "Legacy token migration sweep completed"
        );
    }

    report
}

pub fn load_token(username: &str) -> Result<String, AuthError> {
    tracing::debug!(username = %username, "Attempting to load token");
    let legacy_file_compat = legacy_token_file_compat_enabled();

    // Try keyring first
    let keyring_result = match keyring_get_password(username) {
        Ok(json) => {
            tracing::debug!(username = %username, "Token loaded from keyring");
            Ok(json)
        }
        Err(AuthError::TokenNotFound) => {
            tracing::debug!(username = %username, "No token found in keyring for user");
            Err(AuthError::TokenNotFound)
        }
        Err(err) => {
            tracing::warn!(username = %username, keyring_error = %err, "Failed to load token from keyring");
            Err(err)
        }
    };

    // If token is not in keyring, attempt one-shot migration from legacy token file.
    let (json, from_keyring) = match keyring_result {
        Ok(json) => (json, true),
        Err(AuthError::TokenNotFound) => {
            tracing::warn!(
                username = %username,
                "Token not present in keyring; attempting legacy token migration"
            );
            let legacy_json = load_legacy_token_from_file(username)?;
            if !legacy_json.trim().is_empty() {
                tracing::warn!(
                    username = %username,
                    "Legacy/local token file detected. Migrating to keyring."
                );
            }
            (legacy_json, false)
        }
        Err(keyring_err) => {
            if !legacy_file_compat {
                return Err(keyring_err);
            }
            tracing::warn!(
                username = %username,
                keyring_error = %keyring_err,
                "Keyring unavailable; using legacy token file in explicit compatibility mode"
            );
            let legacy_json = load_legacy_token_from_file(username)?;
            (legacy_json, false)
        }
    };

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        if stored.is_expired() {
            if from_keyring && legacy_file_compat {
                if let Ok(fallback_json) = load_legacy_token_from_file(username) {
                    if let Ok(fallback_stored) = serde_json::from_str::<StoredToken>(&fallback_json)
                    {
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
        if !from_keyring {
            if legacy_file_compat {
                let _ = save_token(
                    username,
                    &stored.access_token,
                    Some(stored.expires_at - stored.created_at),
                );
            } else {
                save_token(
                    username,
                    &stored.access_token,
                    Some(stored.expires_at - stored.created_at),
                )?;
            }
        }
        tracing::info!(username = %username, "Token loaded and valid");
        return Ok(stored.access_token);
    }

    // Fallback: old format (plain token)
    if !from_keyring {
        if legacy_file_compat {
            let _ = save_token(username, &json, None);
        } else {
            save_token(username, &json, None)?;
        }
    }
    tracing::info!(username = %username, "Token loaded (plain format)");
    Ok(json)
}

pub fn load_token_with_expiry(username: &str) -> Result<StoredToken, AuthError> {
    let legacy_file_compat = legacy_token_file_compat_enabled();
    let keyring_load_result = keyring_get_password(username);

    let (json, from_keyring) = match keyring_load_result {
        Ok(json) => (json, true),
        Err(AuthError::TokenNotFound) => {
            tracing::warn!(
                username = %username,
                "Token not present in keyring; attempting legacy token migration"
            );
            let legacy_json = load_legacy_token_from_file(username)?;
            (legacy_json, false)
        }
        Err(keyring_err) => {
            if !legacy_file_compat {
                return Err(keyring_err);
            }
            tracing::warn!(
                username = %username,
                keyring_error = %keyring_err,
                "Keyring token unavailable; using legacy token file in explicit compatibility mode"
            );
            let legacy_json = load_legacy_token_from_file(username)?;
            (legacy_json, false)
        }
    };

    // Try to parse as StoredToken (new format)
    if let Ok(stored) = serde_json::from_str::<StoredToken>(&json) {
        if from_keyring && stored.is_expired() && legacy_file_compat {
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
        if !from_keyring {
            if legacy_file_compat {
                let _ = save_token(
                    username,
                    &stored.access_token,
                    Some(stored.expires_at - stored.created_at),
                );
            } else {
                save_token(
                    username,
                    &stored.access_token,
                    Some(stored.expires_at - stored.created_at),
                )?;
            }
        }
        return Ok(stored);
    }

    // Fallback: old format (plain token) - assume no expiration
    let stored = StoredToken::new(json, None);
    if !from_keyring {
        if legacy_file_compat {
            let _ = save_token(
                username,
                &stored.access_token,
                Some(stored.expires_at - stored.created_at),
            );
        } else {
            save_token(
                username,
                &stored.access_token,
                Some(stored.expires_at - stored.created_at),
            )?;
        }
    }
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
    if let Err(e) = keyring_delete_password(username) {
        tracing::warn!(username = %username, error = %e, "Failed to delete token from keyring");
        keyring_error = Some(e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env_var(key: &str, value: &str) {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var(key);
        }
    }

    fn set_or_clear_env(key: &str, value: Option<&str>) {
        match value {
            Some(v) => set_env_var(key, v),
            None => remove_env_var(key),
        }
    }

    struct EnvGuard {
        legacy_compat: Option<String>,
        legacy_dir: Option<String>,
        simulate_keyring_failure: Option<String>,
        simulate_keyring_memory: Option<String>,
    }

    impl EnvGuard {
        fn apply(
            legacy_compat: &str,
            legacy_dir: &Path,
            simulate_keyring_failure: &str,
            simulate_keyring_memory: &str,
        ) -> Self {
            let guard = Self {
                legacy_compat: std::env::var(LEGACY_TOKEN_FILE_COMPAT_ENV).ok(),
                legacy_dir: std::env::var(LEGACY_TOKEN_DIR_ENV).ok(),
                simulate_keyring_failure: std::env::var(SIMULATE_KEYRING_FAILURE_ENV).ok(),
                simulate_keyring_memory: std::env::var(SIMULATE_KEYRING_MEMORY_ENV).ok(),
            };
            set_env_var(LEGACY_TOKEN_FILE_COMPAT_ENV, legacy_compat);
            set_env_var(
                LEGACY_TOKEN_DIR_ENV,
                &legacy_dir.as_os_str().to_string_lossy(),
            );
            set_env_var(SIMULATE_KEYRING_FAILURE_ENV, simulate_keyring_failure);
            set_env_var(SIMULATE_KEYRING_MEMORY_ENV, simulate_keyring_memory);
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            set_or_clear_env(LEGACY_TOKEN_FILE_COMPAT_ENV, self.legacy_compat.as_deref());
            set_or_clear_env(LEGACY_TOKEN_DIR_ENV, self.legacy_dir.as_deref());
            set_or_clear_env(
                SIMULATE_KEYRING_FAILURE_ENV,
                self.simulate_keyring_failure.as_deref(),
            );
            set_or_clear_env(
                SIMULATE_KEYRING_MEMORY_ENV,
                self.simulate_keyring_memory.as_deref(),
            );
        }
    }

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn create() -> Self {
            let path = std::env::temp_dir().join(format!("gitgov-auth-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("failed to create temp auth test directory");
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn load_token_uses_legacy_file_when_keyring_fails_and_compat_enabled() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        clear_simulated_keyring_store();
        let legacy_dir = TempDirGuard::create();
        let _env_guard = EnvGuard::apply("true", &legacy_dir.path, "true", "false");
        let username = format!("legacy-user-{}", Uuid::new_v4());
        let legacy_token = format!("legacy-token-{}", Uuid::new_v4());

        let token_file = legacy_token_file_path(&username);
        std::fs::write(&token_file, &legacy_token).expect("failed to write legacy token file");

        let loaded = load_token(&username).expect("expected legacy token fallback to succeed");
        assert_eq!(loaded, legacy_token);
    }

    #[test]
    fn load_token_fails_closed_when_keyring_fails_and_compat_disabled() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        clear_simulated_keyring_store();
        let legacy_dir = TempDirGuard::create();
        let _env_guard = EnvGuard::apply("false", &legacy_dir.path, "true", "false");
        let username = format!("legacy-user-{}", Uuid::new_v4());

        let token_file = legacy_token_file_path(&username);
        std::fs::write(&token_file, "legacy-token").expect("failed to write legacy token file");

        let result = load_token(&username);
        assert!(
            matches!(result, Err(AuthError::KeyringError(_))),
            "expected fail-closed keyring error, got: {:?}",
            result
        );
    }

    #[test]
    fn load_token_with_expiry_uses_legacy_file_when_keyring_fails_and_compat_enabled() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        clear_simulated_keyring_store();
        let legacy_dir = TempDirGuard::create();
        let _env_guard = EnvGuard::apply("true", &legacy_dir.path, "true", "false");
        let username = format!("legacy-user-{}", Uuid::new_v4());
        let legacy_token = format!("legacy-token-{}", Uuid::new_v4());
        let stored = StoredToken::new(legacy_token.clone(), Some(3600));
        let stored_json =
            serde_json::to_string(&stored).expect("failed to serialize stored token test fixture");

        let token_file = legacy_token_file_path(&username);
        std::fs::write(&token_file, stored_json).expect("failed to write legacy token file");

        let loaded = load_token_with_expiry(&username)
            .expect("expected legacy token fallback with expiry to succeed");
        assert_eq!(loaded.access_token, legacy_token);
        assert!(loaded.expires_at > loaded.created_at);
    }

    #[test]
    fn migrate_legacy_tokens_from_disk_moves_token_to_simulated_keyring() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        clear_simulated_keyring_store();
        let legacy_dir = TempDirGuard::create();
        let _env_guard = EnvGuard::apply("false", &legacy_dir.path, "false", "true");
        let username = format!("legacy-user-{}", Uuid::new_v4());
        let legacy_token = format!("legacy-token-{}", Uuid::new_v4());
        let token_file = legacy_token_file_path(&username);
        std::fs::write(&token_file, &legacy_token).expect("failed to write legacy token file");

        let report = migrate_legacy_tokens_from_disk();
        assert_eq!(report.scanned_files, 1);
        assert_eq!(report.migrated_tokens, 1);
        assert_eq!(report.failed_files, 0);
        assert!(!token_file.exists(), "legacy token file should be cleaned");

        let loaded = load_token(&username).expect("expected token to be present after migration");
        assert_eq!(loaded, legacy_token);
    }

    #[test]
    fn migrate_legacy_tokens_from_disk_reports_failure_when_keyring_unavailable() {
        let _env_lock = env_lock().lock().expect("env lock poisoned");
        clear_simulated_keyring_store();
        let legacy_dir = TempDirGuard::create();
        let _env_guard = EnvGuard::apply("false", &legacy_dir.path, "true", "false");
        let username = format!("legacy-user-{}", Uuid::new_v4());
        let token_file = legacy_token_file_path(&username);
        std::fs::write(&token_file, "legacy-token").expect("failed to write legacy token file");

        let report = migrate_legacy_tokens_from_disk();
        assert_eq!(report.scanned_files, 1);
        assert_eq!(report.migrated_tokens, 0);
        assert_eq!(report.failed_files, 1);
        assert!(
            token_file.exists(),
            "legacy token file should remain when migration fails"
        );
    }
}
