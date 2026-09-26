// Author: kelexine (https://github.com/kelexine)
// counter/git.rs — Git repository discovery and working tree checks

use std::path::Path;

/// Check whether `dir` is inside a git working tree.
#[allow(dead_code)]
pub fn check_git_repo(dir: &Path) -> bool {
    let mut current = if dir.is_file() {
        dir.parent()
    } else {
        Some(dir)
    };
    while let Some(p) = current {
        let git_dir = p.join(".git");
        if git_dir.exists() {
            return true;
        }
        current = p.parent();
    }
    false
}
