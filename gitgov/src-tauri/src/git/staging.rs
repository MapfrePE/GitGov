use crate::git::GitError;
use git2::{Repository, Signature};

pub fn stage_files(repo: &Repository, files: &[String]) -> Result<(), GitError> {
    let mut index = repo
        .index()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    for file in files {
        let file_path = repo.path().parent().unwrap().join(file);
        if file_path.exists() {
            if file_path.is_file() {
                index.add_path(std::path::Path::new(file)).map_err(|e| {
                    GitError::GitError(format!("Failed to stage {}: {}", file, e.message()))
                })?;
            }
        } else {
            index.remove_path(std::path::Path::new(file)).map_err(|e| {
                GitError::GitError(format!("Failed to remove {}: {}", file, e.message()))
            })?;
        }
    }

    index
        .write()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(())
}

pub fn unstage_files(repo: &Repository, files: &[String]) -> Result<(), GitError> {
    let head = repo
        .head()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let commit = head
        .peel_to_commit()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    repo.reset_default(Some(commit.as_object()), files.iter().map(|s| s.as_str()))
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(())
}

pub fn unstage_all(repo: &Repository) -> Result<(), GitError> {
    let head = repo
        .head()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let commit = head
        .peel_to_commit()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let tree = commit
        .tree()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let mut index = repo
        .index()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    index
        .read_tree(&tree)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    index
        .write()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(())
}

pub fn create_commit(
    repo: &Repository,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, GitError> {
    let mut index = repo
        .index()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let tree_id = index
        .write_tree()
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let signature = Signature::now(author_name, author_email)
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    let parents: Vec<_> = parent_commit.iter().collect();

    if parents.is_empty() && index.is_empty() {
        return Err(GitError::EmptyStaging);
    }

    let commit_id = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(commit_id.to_string())
}

pub fn has_staged_changes(repo: &Repository) -> Result<bool, GitError> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false).show(git2::StatusShow::Index);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    Ok(!statuses.is_empty())
}
