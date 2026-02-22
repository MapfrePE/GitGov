pub mod validator;

pub use validator::*;

use crate::models::GitGovConfig;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    FileNotFound(String),
    #[error("Failed to parse config: {0}")]
    ParseError(String),
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),
}

pub fn load_config(repo_path: &str) -> Result<GitGovConfig, ConfigError> {
    let config_path = Path::new(repo_path).join("gitgov.toml");

    if !config_path.exists() {
        return Err(ConfigError::FileNotFound(
            config_path.to_string_lossy().to_string(),
        ));
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| ConfigError::ParseError(format!("Failed to read file: {}", e)))?;

    let config: GitGovConfig = toml::from_str(&content)
        .map_err(|e| ConfigError::ParseError(format!("TOML parse error: {}", e)))?;

    Ok(config)
}
