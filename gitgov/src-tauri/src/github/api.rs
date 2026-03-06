use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::time::Duration;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("API error: {0}")]
    ApiError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubUser {
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub has_write_access: bool,
}

fn build_http_client() -> Result<Client, ApiError> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))
}

pub fn get_authenticated_user(token: &str) -> Result<GithubUser, ApiError> {
    let client = build_http_client()?;

    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .header("User-Agent", "GitGov/1.0")
        .send()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;

    if response.status() == 401 {
        return Err(ApiError::Unauthorized);
    }

    let user: GithubUser = response
        .json()
        .map_err(|e| ApiError::ApiError(e.to_string()))?;

    Ok(user)
}

pub fn get_repository_info(token: &str, owner: &str, repo: &str) -> Result<RepoInfo, ApiError> {
    let client = build_http_client()?;

    let response = client
        .get(format!("https://api.github.com/repos/{}/{}", owner, repo))
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .header("User-Agent", "GitGov/1.0")
        .send()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;

    if response.status() == 401 {
        return Err(ApiError::Unauthorized);
    }

    if response.status() == 404 {
        return Err(ApiError::NotFound(format!("{}/{}", owner, repo)));
    }

    #[derive(Deserialize)]
    struct RepoResponse {
        name: String,
        full_name: String,
        description: Option<String>,
        permissions: Option<RepoPermissions>,
    }

    #[derive(Deserialize)]
    struct RepoPermissions {
        push: bool,
    }

    let repo_response: RepoResponse = response
        .json()
        .map_err(|e| ApiError::ApiError(e.to_string()))?;

    Ok(RepoInfo {
        name: repo_response.name,
        full_name: repo_response.full_name,
        description: repo_response.description,
        has_write_access: repo_response.permissions.map(|p| p.push).unwrap_or(false),
    })
}

pub fn setup_branch_protection(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<(), ApiError> {
    let client = build_http_client()?;

    let protection_config = serde_json::json!({
        "required_status_checks": {
            "strict": true,
            "contexts": []
        },
        "enforce_admins": false,
        "required_pull_request_reviews": {
            "dismiss_stale_reviews": true,
            "require_code_owner_reviews": false
        },
        "restrictions": null
    });

    let response = client
        .put(format!(
            "https://api.github.com/repos/{}/{}/branches/{}/protection",
            owner, repo, branch
        ))
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "GitGov/1.0")
        .json(&protection_config)
        .send()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;

    if response.status() == 401 {
        return Err(ApiError::Unauthorized);
    }

    if !response.status().is_success() {
        let error_text = response.text().unwrap_or_default();
        return Err(ApiError::ApiError(error_text));
    }

    Ok(())
}
