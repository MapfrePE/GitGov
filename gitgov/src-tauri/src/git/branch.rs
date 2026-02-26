use crate::git::GitError;
use git2::{Branch, BranchType, Repository};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub last_commit_hash: Option<String>,
    pub last_commit_message: Option<String>,
}

pub fn list_branches(repo: &Repository) -> Result<Vec<BranchInfo>, GitError> {
    let current_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    let mut branches: Vec<BranchInfo> = Vec::new();

    let branch_iter = repo
        .branches(None)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    for branch_result in branch_iter {
        let (branch, branch_type) =
            branch_result.map_err(|e| GitError::GitError(e.message().to_string()))?;

        let name = branch
            .name()
            .map_err(|e| GitError::GitError(e.message().to_string()))?
            .unwrap_or("unknown")
            .to_string();

        let is_remote = branch_type == BranchType::Remote;
        let is_current = current_branch.as_ref() == Some(&name);

        let (last_commit_hash, last_commit_message) = branch
            .get()
            .target()
            .map(|oid| {
                repo.find_commit(oid)
                    .ok()
                    .map(|commit| {
                        let hash = commit.id().to_string()[..7].to_string();
                        let message = commit
                            .message()
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("")
                            .to_string();
                        (Some(hash), Some(message))
                    })
                    .unwrap_or((None, None))
            })
            .unwrap_or((None, None));

        branches.push(BranchInfo {
            name,
            is_current,
            is_remote,
            last_commit_hash,
            last_commit_message,
        });
    }

    branches.sort_by(|a, b| {
        a.is_current
            .cmp(&b.is_current)
            .reverse()
            .then(a.is_remote.cmp(&b.is_remote))
            .then(a.name.cmp(&b.name))
    });

    Ok(branches)
}

pub fn create_branch(repo: &Repository, name: &str, from_branch: &str) -> Result<(), GitError> {
    let from_ref = if from_branch == "HEAD" || from_branch.is_empty() {
        repo.head()
            .map_err(|e| GitError::BranchNotFound(format!("HEAD: {}", e.message())))?
            .peel_to_commit()
            .map_err(|e| GitError::GitError(e.message().to_string()))?
    } else {
        let branch = repo
            .find_branch(from_branch, BranchType::Local)
            .or_else(|_| repo.find_branch(&format!("origin/{}", from_branch), BranchType::Remote))
            .map_err(|_| GitError::BranchNotFound(from_branch.to_string()))?;

        branch
            .get()
            .peel_to_commit()
            .map_err(|e| GitError::GitError(e.message().to_string()))?
    };

    repo.branch(name, &from_ref, false).map_err(|e| {
        if e.message().contains("already exists") {
            GitError::BranchExists(name.to_string())
        } else {
            GitError::GitError(e.message().to_string())
        }
    })?;

    Ok(())
}

pub fn checkout_branch(repo: &Repository, name: &str) -> Result<(), GitError> {
    let statuses = repo
        .statuses(None)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    if !statuses.is_empty() {
        return Err(GitError::UncommittedChanges);
    }

    let branch = repo
        .find_branch(name, BranchType::Local)
        .map_err(|_| GitError::BranchNotFound(name.to_string()))?;

    let commit = branch
        .get()
        .peel_to_commit()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    repo.checkout_tree(commit.as_object(), None)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    repo.set_head(&format!("refs/heads/{}", name))
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(())
}

pub fn push_to_remote(repo: &Repository, branch: &str, token: &str) -> Result<(), GitError> {
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| GitError::GitError(format!("Remote 'origin' not found: {}", e.message())))?;

    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, _allowed_types| {
        let username = if url.contains("github.com") {
            username_from_url.unwrap_or("x-access-token")
        } else {
            username_from_url.unwrap_or("git")
        };

        git2::Cred::userpass_plaintext(username, token)
    });

    let push_status_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let push_status_errors_cb = Arc::clone(&push_status_errors);
    callbacks.push_update_reference(move |refname, status| {
        if let Some(status) = status {
            let message = format!("{}: {}", refname, status);
            tracing::warn!(%message, "Remote reported push rejection");
            if let Ok(mut errors) = push_status_errors_cb.lock() {
                errors.push(message);
            }
        }
        Ok(())
    });

    let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

    let mut push_options = git2::PushOptions::new();
    push_options.remote_callbacks(callbacks);

    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|e| GitError::PushFailed(e.message().to_string()))?;

    let status_errors = push_status_errors
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    if !status_errors.is_empty() {
        return Err(GitError::PushFailed(status_errors.join("; ")));
    }

    Ok(())
}

pub fn get_remote_url(repo: &Repository) -> Result<String, GitError> {
    let remote = repo
        .find_remote("origin")
        .map_err(|e| GitError::GitError(format!("Remote 'origin' not found: {}", e.message())))?;

    remote
        .url()
        .map(|s| s.to_string())
        .ok_or_else(|| GitError::GitError("No remote URL configured".to_string()))
}
