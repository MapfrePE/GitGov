use crate::models::{ChangeStatus, FileChange};
use git2::{Repository, Status, StatusOptions, StatusShow};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepoNotFound(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Empty staging area")]
    EmptyStaging,
    #[error("Push failed: {0}")]
    PushFailed(String),
    #[error("Branch already exists: {0}")]
    BranchExists(String),
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    #[error("Uncommitted changes detected")]
    UncommittedChanges,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Git error: {0}")]
    GitError(String),
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

pub fn open_repository(path: &str) -> Result<Repository, GitError> {
    Repository::open(path).map_err(|e| {
        let msg = e.message();
        if msg.contains("does not exist") || msg.contains("not found") {
            GitError::RepoNotFound(format!(
                "La ruta no existe o no es un repositorio Git: {}",
                path
            ))
        } else if msg.contains("permission") {
            GitError::Unauthorized
        } else {
            GitError::GitError(msg.to_string())
        }
    })
}

pub fn get_working_tree_changes(repo: &Repository) -> Result<Vec<FileChange>, GitError> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .recurse_untracked_dirs(true)
        .show(StatusShow::Workdir);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let changes: Vec<FileChange> = statuses
        .iter()
        .filter_map(|entry| {
            let path = entry.path()?.to_string();
            let status = entry.status();

            let change_status = if status.contains(Status::INDEX_MODIFIED)
                || status.contains(Status::WT_MODIFIED)
            {
                ChangeStatus::Modified
            } else if status.contains(Status::INDEX_NEW) || status.contains(Status::WT_NEW) {
                ChangeStatus::Added
            } else if status.contains(Status::INDEX_DELETED) || status.contains(Status::WT_DELETED)
            {
                ChangeStatus::Deleted
            } else if status.contains(Status::INDEX_RENAMED) || status.contains(Status::WT_RENAMED)
            {
                ChangeStatus::Renamed
            } else {
                ChangeStatus::Untracked
            };

            let staged = status.contains(Status::INDEX_MODIFIED)
                || status.contains(Status::INDEX_NEW)
                || status.contains(Status::INDEX_DELETED)
                || status.contains(Status::INDEX_RENAMED);

            Some(FileChange {
                path,
                status: change_status,
                staged,
                diff: None,
            })
        })
        .collect();

    Ok(changes)
}

pub fn get_current_branch(repo: &Repository) -> Result<String, GitError> {
    let head = repo
        .head()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let name = head
        .shorthand()
        .ok_or_else(|| GitError::GitError("Could not get branch name".to_string()))?;

    Ok(name.to_string())
}

pub fn get_head_commit_hash(repo: &Repository) -> Result<String, GitError> {
    let head = repo
        .head()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let commit = head
        .peel_to_commit()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(commit.id().to_string())
}
