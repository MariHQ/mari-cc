//! Repository discovery for Mari's repository-local workflow.

use std::path::PathBuf;

/// Walk up from the current directory until a Git repository is found.
pub fn repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Return the repository root, or the current directory outside Git.
pub fn work_root() -> PathBuf {
    repo_root().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
