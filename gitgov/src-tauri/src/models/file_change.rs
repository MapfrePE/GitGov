use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
}

impl fmt::Display for ChangeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeStatus::Modified => write!(f, "M"),
            ChangeStatus::Added => write!(f, "A"),
            ChangeStatus::Deleted => write!(f, "D"),
            ChangeStatus::Renamed => write!(f, "R"),
            ChangeStatus::Untracked => write!(f, "?"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub status: ChangeStatus,
    pub staged: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}
