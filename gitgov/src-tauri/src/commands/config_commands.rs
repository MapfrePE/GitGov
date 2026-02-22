use crate::config::load_config;
use crate::git::{get_remote_url, open_repository};
use crate::models::GitGovConfig;
use serde::{Deserialize, Serialize};

fn to_command_error(e: impl std::fmt::Display, code: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "code": code,
        "message": e.to_string()
    }))
    .unwrap_or_else(|_| format!("{{\"code\":\"{}\",\"message\":\"{}\"}}", code, e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoValidation {
    pub path_exists: bool,
    pub is_git_repo: bool,
    pub has_remote_origin: bool,
    pub has_gitgov_toml: bool,
    pub remote_url: Option<String>,
}

#[tauri::command]
pub fn cmd_load_repo_config(repo_path: String) -> Result<GitGovConfig, String> {
    let config = load_config(&repo_path).map_err(|e| to_command_error(e, "CONFIG_ERROR"))?;

    Ok(config)
}

#[tauri::command]
pub fn cmd_validate_repo(repo_path: String) -> Result<RepoValidation, String> {
    use std::path::Path;

    let path = Path::new(&repo_path);

    let path_exists = path.exists();

    let (is_git_repo, has_remote_origin, remote_url) = if path_exists {
        match open_repository(&repo_path) {
            Ok(repo) => {
                let remote = get_remote_url(&repo).ok();
                (true, remote.is_some(), remote)
            }
            Err(_) => (false, false, None),
        }
    } else {
        (false, false, None)
    };

    let has_gitgov_toml = path.join("gitgov.toml").exists();

    Ok(RepoValidation {
        path_exists,
        is_git_repo,
        has_remote_origin,
        has_gitgov_toml,
        remote_url,
    })
}

#[tauri::command]
pub fn cmd_validate_branch_name(
    name: String,
    repo_path: String,
    developer_login: String,
    is_admin: bool,
    user_group: Option<String>,
) -> Result<crate::config::ValidationResult, String> {
    use crate::config::validate_branch_name;
    use crate::models::AuthenticatedUser;

    let config = match load_config(&repo_path) {
        Ok(c) => c,
        Err(e) => return Err(to_command_error(e, "CONFIG_ERROR")),
    };

    let user = AuthenticatedUser {
        login: developer_login,
        name: String::new(),
        avatar_url: String::new(),
        group: user_group,
        is_admin,
    };

    Ok(validate_branch_name(&name, &config, &user))
}
