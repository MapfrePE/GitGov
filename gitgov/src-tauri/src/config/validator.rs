use crate::models::{AuthenticatedUser, GitGovConfig};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Protected branch: {0}")]
    ProtectedBranch(String),
    #[error("No group assigned")]
    NoGroup,
    #[error("Branch name does not match allowed patterns")]
    BranchPatternMismatch,
    #[error("Path not allowed: {0}")]
    PathNotAllowed(String),
    #[error("Invalid commit message format")]
    InvalidCommitMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum ValidationResult {
    Valid,
    Blocked(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationResult {
    pub path: String,
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMessageValidation {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const VALID_COMMIT_PREFIXES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "test", "chore", "hotfix",
];

pub fn validate_branch_name(
    name: &str,
    config: &GitGovConfig,
    user: &AuthenticatedUser,
) -> ValidationResult {
    if config.branches.protected.iter().any(|p| p == name) {
        return ValidationResult::Blocked(format!(
            "Rama protegida. No puedes operar directamente en {}.",
            name
        ));
    }

    if user.is_admin {
        return ValidationResult::Valid;
    }

    let group = match &user.group {
        Some(g) => g,
        None => {
            return ValidationResult::Blocked(
                "No perteneces a ningún grupo configurado.".to_string(),
            );
        }
    };

    let group_config = match config.groups.get(group) {
        Some(gc) => gc,
        None => {
            return ValidationResult::Blocked(format!(
                "El grupo '{}' no existe en la configuración.",
                group
            ));
        }
    };

    let matches_allowed = group_config.allowed_branches.iter().any(|pattern| {
        Pattern::new(pattern)
            .map(|p| p.matches(name))
            .unwrap_or(false)
    });

    if !matches_allowed {
        return ValidationResult::Blocked(
            "El nombre de rama no coincide con los patrones permitidos para tu grupo.".to_string(),
        );
    }

    let matches_global = config.branches.patterns.iter().any(|pattern| {
        Pattern::new(pattern)
            .map(|p| p.matches(name))
            .unwrap_or(false)
    });

    if !matches_global {
        return ValidationResult::Blocked(
            "El nombre de rama no cumple con la nomenclatura establecida.".to_string(),
        );
    }

    ValidationResult::Valid
}

pub fn validate_file_paths(
    files: &[String],
    config: &GitGovConfig,
    user: &AuthenticatedUser,
) -> Vec<PathValidationResult> {
    if user.is_admin {
        return files
            .iter()
            .map(|p| PathValidationResult {
                path: p.clone(),
                allowed: true,
                reason: None,
            })
            .collect();
    }

    let group = match &user.group {
        Some(g) => g,
        None => {
            return files
                .iter()
                .map(|p| PathValidationResult {
                    path: p.clone(),
                    allowed: false,
                    reason: Some("No tienes grupo asignado".to_string()),
                })
                .collect();
        }
    };

    let group_config = match config.groups.get(group) {
        Some(gc) => gc,
        None => {
            return files
                .iter()
                .map(|p| PathValidationResult {
                    path: p.clone(),
                    allowed: false,
                    reason: Some(format!("Grupo '{}' no encontrado", group)),
                })
                .collect();
        }
    };

    files
        .iter()
        .map(|path| {
            let allowed = group_config.allowed_paths.iter().any(|pattern| {
                Pattern::new(pattern)
                    .map(|p| p.matches(path))
                    .unwrap_or(false)
            });

            PathValidationResult {
                path: path.clone(),
                allowed,
                reason: if allowed {
                    None
                } else {
                    Some("Path no permitido para tu grupo".to_string())
                },
            }
        })
        .collect()
}

pub fn validate_commit_message(message: &str) -> CommitMessageValidation {
    let message = message.trim();

    if message.is_empty() {
        return CommitMessageValidation {
            valid: false,
            error: Some("El mensaje de commit no puede estar vacío".to_string()),
        };
    }

    let has_valid_prefix = VALID_COMMIT_PREFIXES.iter().any(|prefix| {
        message.starts_with(&format!("{}:", prefix))
            || message.starts_with(&format!("{}: ", prefix))
    });

    if has_valid_prefix {
        CommitMessageValidation {
            valid: true,
            error: None,
        }
    } else {
        CommitMessageValidation {
            valid: false,
            error: Some(format!(
                "El mensaje debe comenzar con uno de: {}",
                VALID_COMMIT_PREFIXES.join(", ")
            )),
        }
    }
}

pub fn find_user_group(login: &str, config: &GitGovConfig) -> Option<String> {
    config
        .groups
        .iter()
        .find(|(_, group_config)| group_config.members.iter().any(|m| m == login))
        .map(|(name, _)| name.clone())
}

pub fn is_admin(login: &str, config: &GitGovConfig) -> bool {
    config.admins.iter().any(|admin| admin == login)
}
