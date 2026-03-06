use crate::git::GitError;
use git2::{BranchType, Repository};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const MAX_PENDING_PUSH_FILES: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub last_commit_hash: Option<String>,
    pub last_commit_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSyncStatus {
    pub branch: String,
    pub upstream: Option<String>,
    pub has_upstream: bool,
    pub ahead: usize,
    pub behind: usize,
    #[serde(default)]
    pub pending_local_commits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPushFile {
    pub path: String,
    pub commits_touching: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPushPreview {
    pub branch: String,
    pub commit_count: usize,
    pub files: Vec<PendingPushFile>,
    #[serde(default)]
    pub truncated: bool,
}

fn collect_local_only_commit_oids(repo: &Repository, local_oid: git2::Oid) -> Vec<git2::Oid> {
    let mut revwalk = match repo.revwalk() {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };

    let _ = revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL);

    if revwalk.push(local_oid).is_err() {
        return Vec::new();
    }

    if let Ok(remote_branches) = repo.branches(Some(BranchType::Remote)) {
        for branch_result in remote_branches {
            let (branch, _) = match branch_result {
                Ok(pair) => pair,
                Err(_) => continue,
            };
            if let Some(remote_oid) = branch.get().target() {
                let _ = revwalk.hide(remote_oid);
            }
        }
    }

    revwalk.filter_map(Result::ok).collect()
}

fn count_local_commits_not_on_any_remote(repo: &Repository, local_oid: git2::Oid) -> usize {
    collect_local_only_commit_oids(repo, local_oid).len()
}

fn normalize_git_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn get_pending_push_preview(
    repo: &Repository,
    branch: Option<&str>,
) -> Result<PendingPushPreview, GitError> {
    let branch_name = match branch {
        Some(name) if !name.trim().is_empty() => name.trim().to_string(),
        _ => repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "HEAD".to_string()),
    };

    if branch_name == "HEAD" {
        return Ok(PendingPushPreview {
            branch: branch_name,
            commit_count: 0,
            files: Vec::new(),
            truncated: false,
        });
    }

    let local_branch = repo
        .find_branch(&branch_name, BranchType::Local)
        .map_err(|_| GitError::BranchNotFound(branch_name.clone()))?;

    let local_oid = local_branch
        .get()
        .target()
        .ok_or_else(|| GitError::GitError("Local branch has no target commit".to_string()))?;

    let local_only_commits = collect_local_only_commit_oids(repo, local_oid);
    if local_only_commits.is_empty() {
        return Ok(PendingPushPreview {
            branch: branch_name,
            commit_count: 0,
            files: Vec::new(),
            truncated: false,
        });
    }

    let mut touched_paths: BTreeMap<String, usize> = BTreeMap::new();
    for oid in &local_only_commits {
        let commit = repo
            .find_commit(*oid)
            .map_err(|e| GitError::GitError(e.message().to_string()))?;
        let commit_tree = commit
            .tree()
            .map_err(|e| GitError::GitError(e.message().to_string()))?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(
                commit
                    .parent(0)
                    .map_err(|e| GitError::GitError(e.message().to_string()))?
                    .tree()
                    .map_err(|e| GitError::GitError(e.message().to_string()))?,
            )
        } else {
            None
        };

        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
            .map_err(|e| GitError::GitError(e.message().to_string()))?;

        for delta in diff.deltas() {
            let path_opt = delta.new_file().path().or(delta.old_file().path());
            let Some(path) = path_opt else {
                continue;
            };
            let normalized = normalize_git_path(path);
            if normalized.is_empty() {
                continue;
            }
            *touched_paths.entry(normalized).or_insert(0) += 1;
        }
    }

    let mut files: Vec<PendingPushFile> = touched_paths
        .into_iter()
        .map(|(path, commits_touching)| PendingPushFile {
            path,
            commits_touching,
        })
        .collect();
    files.sort_by(|a, b| {
        b.commits_touching
            .cmp(&a.commits_touching)
            .then(a.path.cmp(&b.path))
    });

    let truncated = files.len() > MAX_PENDING_PUSH_FILES;
    if truncated {
        files.truncate(MAX_PENDING_PUSH_FILES);
    }

    Ok(PendingPushPreview {
        branch: branch_name,
        commit_count: local_only_commits.len(),
        files,
        truncated,
    })
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

pub fn get_branch_sync_status(
    repo: &Repository,
    branch: Option<&str>,
) -> Result<BranchSyncStatus, GitError> {
    let branch_name = match branch {
        Some(name) if !name.trim().is_empty() => name.trim().to_string(),
        _ => repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "HEAD".to_string()),
    };

    if branch_name == "HEAD" {
        return Ok(BranchSyncStatus {
            branch: branch_name,
            upstream: None,
            has_upstream: false,
            ahead: 0,
            behind: 0,
            pending_local_commits: 0,
        });
    }

    let local_branch = repo
        .find_branch(&branch_name, BranchType::Local)
        .map_err(|_| GitError::BranchNotFound(branch_name.clone()))?;

    let local_oid = local_branch
        .get()
        .target()
        .ok_or_else(|| GitError::GitError("Local branch has no target commit".to_string()))?;

    let upstream_branch = match local_branch.upstream() {
        Ok(upstream) => upstream,
        Err(_) => {
            let pending_local_commits = count_local_commits_not_on_any_remote(repo, local_oid);
            return Ok(BranchSyncStatus {
                branch: branch_name,
                upstream: None,
                has_upstream: false,
                ahead: 0,
                behind: 0,
                pending_local_commits,
            });
        }
    };

    let upstream_name = upstream_branch.name().ok().flatten().map(|s| s.to_string());

    let upstream_oid = match upstream_branch.get().target() {
        Some(oid) => oid,
        None => {
            return Ok(BranchSyncStatus {
                branch: branch_name,
                upstream: upstream_name,
                has_upstream: true,
                ahead: 0,
                behind: 0,
                pending_local_commits: 0,
            });
        }
    };

    let (ahead, behind) = repo
        .graph_ahead_behind(local_oid, upstream_oid)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(BranchSyncStatus {
        branch: branch_name,
        upstream: upstream_name,
        has_upstream: true,
        ahead,
        behind,
        pending_local_commits: ahead,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("{}-{}", prefix, stamp));
        let _ = fs::create_dir_all(&path);
        path
    }

    fn write_file(path: &Path, name: &str, content: &str) {
        let file_path = path.join(name);
        fs::write(file_path, content).expect("failed to write test file");
    }

    fn commit_all(repo: &Repository, message: &str) {
        let mut index = repo.index().expect("failed to open index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to add files");
        index.write().expect("failed to write index");
        let tree_id = index.write_tree().expect("failed to write tree");
        let tree = repo.find_tree(tree_id).expect("failed to find tree");
        let sig = git2::Signature::now("GitGov Test", "gitgov-test@example.com")
            .expect("failed to build signature");

        let parent = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .and_then(|oid| repo.find_commit(oid).ok());

        match parent {
            Some(parent_commit) => {
                repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    message,
                    &tree,
                    &[&parent_commit],
                )
                .expect("failed to commit with parent");
            }
            None => {
                repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])
                    .expect("failed to initial commit");
            }
        }
    }

    fn current_branch_name(repo: &Repository) -> String {
        repo.head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "master".to_string())
    }

    #[test]
    fn branch_sync_reports_pending_local_commits_without_upstream() {
        let repo_dir = unique_temp_dir("gitgov-branch-sync-no-upstream");
        let repo = Repository::init(&repo_dir).expect("failed to init repo");

        write_file(&repo_dir, "a.txt", "one");
        commit_all(&repo, "initial");
        write_file(&repo_dir, "b.txt", "two");
        commit_all(&repo, "second");

        let branch = current_branch_name(&repo);
        let status = get_branch_sync_status(&repo, Some(&branch)).expect("sync status failed");

        assert!(!status.has_upstream);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert!(
            status.pending_local_commits >= 1,
            "pending_local_commits must remain visible without upstream"
        );

        let _ = fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn branch_sync_reports_pending_as_ahead_when_upstream_exists() {
        let remote_dir = unique_temp_dir("gitgov-remote-bare");
        let local_dir = unique_temp_dir("gitgov-local-repo");
        let remote = Repository::init_bare(&remote_dir).expect("failed to init bare repo");
        let _ = remote;
        let repo = Repository::init(&local_dir).expect("failed to init local repo");

        write_file(&local_dir, "a.txt", "one");
        commit_all(&repo, "initial");

        let remote_url = remote_dir.to_string_lossy().to_string();
        repo.remote("origin", &remote_url)
            .expect("failed to create remote");

        let branch = current_branch_name(&repo);
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        let mut remote = repo.find_remote("origin").expect("missing remote");
        remote
            .push(&[&refspec], None)
            .expect("failed to push initial branch");

        let mut local_branch = repo
            .find_branch(&branch, BranchType::Local)
            .expect("failed to find local branch");
        local_branch
            .set_upstream(Some(&format!("origin/{}", branch)))
            .expect("failed to set upstream");

        write_file(&local_dir, "b.txt", "two");
        commit_all(&repo, "second");

        let status = get_branch_sync_status(&repo, Some(&branch)).expect("sync status failed");

        assert!(status.has_upstream);
        assert!(status.ahead >= 1);
        assert_eq!(status.pending_local_commits, status.ahead);

        let _ = fs::remove_dir_all(local_dir);
        let _ = fs::remove_dir_all(remote_dir);
    }

    #[test]
    fn pending_push_preview_lists_files_from_local_unpushed_commits() {
        let remote_dir = unique_temp_dir("gitgov-pending-preview-remote");
        let local_dir = unique_temp_dir("gitgov-pending-preview-local");
        let remote = Repository::init_bare(&remote_dir).expect("failed to init bare repo");
        let _ = remote;
        let repo = Repository::init(&local_dir).expect("failed to init local repo");

        write_file(&local_dir, "base.txt", "base");
        commit_all(&repo, "initial");

        let remote_url = remote_dir.to_string_lossy().to_string();
        repo.remote("origin", &remote_url)
            .expect("failed to create remote");

        let branch = current_branch_name(&repo);
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        let mut remote = repo.find_remote("origin").expect("missing remote");
        remote
            .push(&[&refspec], None)
            .expect("failed to push initial branch");

        let mut local_branch = repo
            .find_branch(&branch, BranchType::Local)
            .expect("failed to find local branch");
        local_branch
            .set_upstream(Some(&format!("origin/{}", branch)))
            .expect("failed to set upstream");

        write_file(&local_dir, "pending-a.txt", "A1");
        commit_all(&repo, "pending a");
        write_file(&local_dir, "pending-b.txt", "B1");
        commit_all(&repo, "pending b");

        let preview = get_pending_push_preview(&repo, Some(&branch)).expect("preview failed");
        assert_eq!(preview.commit_count, 2);
        assert!(
            preview.files.iter().any(|f| f.path == "pending-a.txt"),
            "pending-a.txt must be visible in pending preview"
        );
        assert!(
            preview.files.iter().any(|f| f.path == "pending-b.txt"),
            "pending-b.txt must be visible in pending preview"
        );

        let _ = fs::remove_dir_all(local_dir);
        let _ = fs::remove_dir_all(remote_dir);
    }
}
