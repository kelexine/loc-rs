// Author: kelexine (https://github.com/kelexine)
// counter/git.rs — Git repository discovery, index traversal, and commit dates

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};

use super::discovery::get_manual_files;
use crate::locignore::LocIgnore;

/// Check whether `dir` is inside a git working tree.
pub fn check_git_repo(dir: &Path) -> bool {
    git2::Repository::discover(dir)
        .map(|repo| repo.workdir().is_some())
        .unwrap_or(false)
}

/// Enumerate all files that git knows about (tracked + untracked non-ignored),
/// post-filtered by the `.locignore` ruleset.
///
/// When `.locignore` contains negation patterns (`!`), we also query the list
/// of git-ignored files so that a negation can re-include them — giving
/// `.locignore` full precedence over `.gitignore`.
pub fn get_git_files(dir: &Path, locignore: &LocIgnore) -> Vec<PathBuf> {
    let repo = match git2::Repository::discover(dir) {
        Ok(r) => r,
        Err(_) => return get_manual_files(dir, locignore, false),
    };

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return get_manual_files(dir, locignore, false),
    };

    let include_ignored = locignore.has_negations();
    let mut files = match git_discover_files(&repo, workdir, dir, include_ignored) {
        Ok(f) => f,
        Err(_) => return get_manual_files(dir, locignore, false),
    };

    if files.is_empty() {
        return get_manual_files(dir, locignore, false);
    }

    // Apply .locignore excludes to the full combined set.
    files.retain(|p| !locignore.is_excluded(p));
    files
}

/// Discovers repository files (tracked, untracked, and optionally ignored)
/// located under `target_dir` using libgit2.
fn git_discover_files(
    repo: &git2::Repository,
    workdir: &Path,
    target_dir: &Path,
    include_ignored: bool,
) -> Result<Vec<PathBuf>> {
    let index = repo.index().context("Failed to open git index")?;
    let mut file_set = HashSet::with_capacity(index.len());

    let is_repo_root = target_dir == workdir;
    let rel_target = if !is_repo_root {
        target_dir.strip_prefix(workdir).ok()
    } else {
        None
    };
    let rel_target_bytes = rel_target.and_then(|p| p.to_str()).map(|s| s.as_bytes());

    for entry in index.iter() {
        // Skip git submodules (0o160000) or directory tree entries (0o040000)
        let file_type = entry.mode & 0o170000;
        if file_type == 0o160000 || file_type == 0o040000 {
            continue;
        }

        // Fast-path: filter by directory before UTF-8 decoding and PathBuf allocation
        if let Some(target_bytes) = rel_target_bytes {
            if !entry.path.starts_with(target_bytes) {
                continue;
            }
            let prefix_len = target_bytes.len();
            if entry.path.len() > prefix_len && entry.path[prefix_len] != b'/' {
                continue;
            }
        }

        if let Ok(rel_str) = std::str::from_utf8(&entry.path) {
            let full_path = workdir.join(rel_str);
            if is_repo_root || full_path.starts_with(target_dir) {
                // S_IFREG (0o100000): known regular file in git index, avoid stat
                if file_type == 0o100000 || full_path.is_file() {
                    file_set.insert(full_path);
                }
            }
        }
    }

    // 2. Untracked (and optionally ignored) files in working directory
    let mut status_opts = git2::StatusOptions::new();
    status_opts
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(include_ignored)
        .recurse_ignored_dirs(include_ignored);

    let statuses = repo
        .statuses(Some(&mut status_opts))
        .context("Failed to query repository status")?;

    for entry in statuses.iter() {
        let status = entry.status();
        let is_untracked = status.contains(git2::Status::WT_NEW);
        let is_ignored = status.contains(git2::Status::IGNORED);

        if (is_untracked || (include_ignored && is_ignored))
            && let Ok(path_str) = entry.path()
        {
            let full_path = workdir.join(path_str);
            if full_path.starts_with(target_dir) && full_path.is_file() {
                file_set.insert(full_path);
            }
        }
    }

    Ok(file_set.into_iter().collect())
}

pub fn get_all_git_dates(root: &Path) -> HashMap<PathBuf, DateTime<Utc>> {
    let mut map = HashMap::new();
    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return map,
    };

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return map,
    };

    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(_) => return map,
    };

    // Newest commits first (topological + time order)
    if revwalk
        .set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
        .is_err()
    {
        return map;
    }
    if revwalk.push_head().is_err() {
        return map;
    }

    for oid_res in revwalk {
        let oid = match oid_res {
            Ok(id) => id,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let commit_time = commit.time().seconds();
        let commit_date = match Utc.timestamp_opt(commit_time, 0).single() {
            Some(d) => d,
            None => continue,
        };

        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        // For initial root commit (no parent), inspect all entries in the tree
        if commit.parent_count() == 0 {
            let _ = tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
                if entry.kind() == Some(git2::ObjectType::Blob)
                    && let Ok(name) = entry.name()
                {
                    let rel_path = if dir.is_empty() {
                        PathBuf::from(name)
                    } else {
                        Path::new(dir).join(name)
                    };
                    let full_path = workdir.join(rel_path);
                    if full_path.starts_with(root) {
                        map.entry(full_path).or_insert(commit_date);
                    }
                }
                git2::TreeWalkResult::Ok
            });
            continue;
        }

        // Compare diff against first parent (matches `git log --name-only`)
        let parent = match commit.parent(0) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let parent_tree = match parent.tree() {
            Ok(pt) => pt,
            Err(_) => continue,
        };

        let diff = match repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for delta in diff.deltas() {
            if let Some(new_file) = delta.new_file().path() {
                let full_path = workdir.join(new_file);
                if full_path.starts_with(root) {
                    map.entry(full_path).or_insert(commit_date);
                }
            }
        }
    }

    map
}
