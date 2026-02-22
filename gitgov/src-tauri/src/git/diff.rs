use crate::git::GitError;
use git2::Repository;

pub fn get_file_diff(repo: &Repository, file_path: &str) -> Result<String, GitError> {
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(file_path);
    diff_opts.context_lines(3);

    let diff = repo
        .diff_index_to_workdir(None, Some(&mut diff_opts))
        .map_err(|e| GitError::GitError(e.message().to_string()))?;

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            ' ' | '+' | '-' | '@' => {
                diff_text.push_str(&format!(
                    "{}{}\n",
                    line.origin(),
                    std::str::from_utf8(line.content()).unwrap_or("")
                ));
            }
            _ => {}
        }
        true
    })
    .map_err(|e| GitError::GitError(e.message().to_string()))?;

    if diff_text.is_empty() {
        let mut diff_opts = git2::DiffOptions::new();
        diff_opts.pathspec(file_path);
        diff_opts.context_lines(3);
        diff_opts.show_binary(false);

        let head = repo
            .head()
            .map_err(|e| GitError::GitError(e.message().to_string()))?;
        let commit = head
            .peel_to_commit()
            .map_err(|e| GitError::GitError(e.message().to_string()))?;
        let tree = commit
            .tree()
            .map_err(|e| GitError::GitError(e.message().to_string()))?;

        let diff = repo
            .diff_tree_to_workdir(Some(&tree), Some(&mut diff_opts))
            .map_err(|e| GitError::GitError(e.message().to_string()))?;

        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            match line.origin() {
                ' ' | '+' | '-' | '@' => {
                    diff_text.push_str(&format!(
                        "{}{}\n",
                        line.origin(),
                        std::str::from_utf8(line.content()).unwrap_or("")
                    ));
                }
                _ => {}
            }
            true
        })
        .map_err(|e| GitError::GitError(e.message().to_string()))?;
    }

    Ok(diff_text)
}
