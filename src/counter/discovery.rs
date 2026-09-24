// Author: kelexine (https://github.com/kelexine)
// counter/discovery.rs — Filesystem traversal and manual file discovery

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, TimeZone, Utc};
use walkdir::WalkDir;

use crate::language::EXCLUDED_DIRS;
use crate::locignore::LocIgnore;

/// Walk `dir` with WalkDir, applying `.locignore` rules at both the directory
/// pruning and file inclusion stages.
pub fn get_manual_files(dir: &Path, locignore: &LocIgnore, include_hidden: bool) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_dir() {
                // Always prune hard-excluded and hidden dirs.
                if EXCLUDED_DIRS.contains(name.as_ref()) || name == ".git" {
                    return false;
                }
                if !include_hidden && name != ".well-known" && name.starts_with('.') {
                    return false;
                }
                // Prune via locignore only when there are no negation patterns.
                // If negations exist we must descend to check each file individually,
                // because a !pattern inside an excluded dir should still take effect.
                if !locignore.has_negations() && locignore.is_excluded(e.path()) {
                    return false;
                }
                true
            } else {
                include_hidden || !name.starts_with('.')
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        // Final per-file locignore check (handles both the no-negation and
        // the has-negation cases uniformly).
        .filter(|e| !locignore.is_excluded(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect()
}

pub fn get_fs_last_modified(path: &Path) -> Option<DateTime<Utc>> {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single())
}
