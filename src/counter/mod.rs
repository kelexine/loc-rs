// Author: kelexine (https://github.com/kelexine)
// counter/mod.rs — File discovery, line counting, and parallel processing

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rayon::prelude::*;

use crate::cli::Args;
use crate::extractors;
use crate::language::{BINARY_EXTENSIONS, EXCLUDED_DIRS};
use crate::models::{Breakdown, FileInfo, ScanResult};

/// Configuration for a scan run.
///
/// `git_dates_cache` is wrapped in `Arc` so that `clone()` is O(1) regardless
/// of how many files are tracked — the heavy HashMap is shared, not copied.
#[derive(Clone)]
pub struct ScanConfig {
    /// Canonicalized target directory to scan.
    pub target_dir: PathBuf,
    /// Optional extension allowlist (for `-t/--type` filters), including leading dots.
    pub allowed_extensions: Option<HashSet<String>>,
    /// Optional line threshold for "large file" warnings.
    pub warn_size: Option<usize>,
    /// Whether git commit dates should be resolved for each file.
    pub use_git_dates: bool,
    /// Whether parallel file processing is enabled.
    pub parallel: bool,
    /// Whether function extraction is enabled.
    pub extract_functions: bool,
    /// Whether the target directory is inside a git work tree.
    pub is_git_repo: bool,
    /// Patterns loaded from `.locignore`.
    pub custom_ignore: HashSet<String>,
    /// Whether hidden files/directories should be included.
    pub include_hidden: bool,
    /// Optional precomputed git date map for fast file timestamp lookups.
    pub git_dates_cache: Option<Arc<HashMap<PathBuf, DateTime<Utc>>>>,
}

impl ScanConfig {
    /// Build a scan configuration from parsed CLI arguments and global config.
    pub fn from_args(args: &Args) -> Result<Self> {
        let target_dir = Path::new(&args.directory)
            .canonicalize()
            .with_context(|| format!("Cannot resolve directory: {}", args.directory))?;

        if !target_dir.is_dir() {
            anyhow::bail!("Not a directory: {}", target_dir.display());
        }

        let is_git_repo = check_git_repo(&target_dir);
        let global_config = crate::config::GlobalConfig::load();

        // Build allowed extension set from language filter flags
        let mut types_to_use = args.file_types.clone();
        if types_to_use.is_empty()
            && let Some(ref default_types) = global_config.default_types
        {
            types_to_use = default_types.clone();
        }

        let allowed_extensions = if types_to_use.is_empty() {
            None
        } else {
            let mut exts = HashSet::new();
            for lang in &types_to_use {
                let resolved = crate::language::resolve_extensions(lang);
                if resolved.is_empty()
                    || (resolved.len() == 1 && resolved[0] == format!(".{}", lang))
                {
                    eprintln!("[WARNING] Unknown language filter: {}", lang);
                }
                exts.extend(resolved);
            }
            Some(exts)
        };

        let custom_ignore = load_locignore(&target_dir);
        let warn_size = args.warn_size.or(global_config.warn_size);
        let extract_functions = args.functions
            || args.func_analysis
            || global_config.always_extract_functions.unwrap_or(false);

        Ok(Self {
            target_dir,
            allowed_extensions,
            warn_size,
            use_git_dates: args.git_dates,
            parallel: !args.no_parallel,
            extract_functions,
            is_git_repo,
            custom_ignore,
            include_hidden: args.include_hidden,
            git_dates_cache: None,
        })
    }
}

/// Run the full scan and return a ScanResult.
pub fn run_scan(config: &ScanConfig) -> Result<ScanResult> {
    let files = if config.is_git_repo && !config.include_hidden {
        get_git_files(&config.target_dir)
    } else {
        get_manual_files(
            &config.target_dir,
            &config.custom_ignore,
            config.include_hidden,
        )
    };

    // Populate git dates cache *before* cloning config into runner_config.
    // Wrapping in Arc makes the subsequent clone() O(1).
    let git_dates_cache: Option<Arc<HashMap<PathBuf, DateTime<Utc>>>> =
        if config.use_git_dates && config.is_git_repo {
            Some(Arc::new(get_all_git_dates(&config.target_dir)))
        } else {
            None
        };

    // Build a runner config that owns an Arc reference to the cache.
    // No pre-sort: rayon doesn't preserve order, so sorting before dispatch
    // is pointless — we sort the output instead.
    let mut runner_config = config.clone();
    runner_config.git_dates_cache = git_dates_cache;

    let mut file_infos: Vec<FileInfo> = if runner_config.parallel && files.len() > 50 {
        files
            .par_iter()
            .filter_map(|path| match process_file(path, &runner_config) {
                Ok(opt) => opt,
                Err(e) => {
                    eprintln!("[WARN] Skipped {}: {}", path.display(), e);
                    None
                }
            })
            .collect()
    } else {
        files
            .iter()
            .filter_map(|path| match process_file(path, &runner_config) {
                Ok(opt) => opt,
                Err(e) => {
                    eprintln!("[WARN] Skipped {}: {}", path.display(), e);
                    None
                }
            })
            .collect()
    };

    file_infos.sort_by(|a, b| a.path.cmp(&b.path));

    // Build breakdown
    let mut breakdown: Breakdown = std::collections::HashMap::new();
    for fi in &file_infos {
        if fi.is_binary {
            continue;
        }
        let ext = if fi.extension().is_empty() {
            fi.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("(no ext)")
                .to_string()
        } else {
            fi.extension().to_string()
        };
        let stats = breakdown.entry(ext).or_default();
        stats.lines += fi.lines;
        stats.code += fi.code;
        stats.comment += fi.comment;
        stats.blank += fi.blank;
        stats.files += 1;
        stats.functions += fi.function_count();
    }

    Ok(ScanResult {
        files: file_infos,
        breakdown,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// File processing
// ─────────────────────────────────────────────────────────────────────────────

fn process_file(path: &Path, config: &ScanConfig) -> Result<Option<FileInfo>> {
    if !path.is_file() {
        return Ok(None);
    }

    // Extension filter
    if let Some(allowed) = &config.allowed_extensions {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();
        if !allowed.contains(&ext) {
            return Ok(None);
        }
    }

    let is_binary = is_binary_file(path);

    // Skip binary files when type-filtering is active
    if is_binary && config.allowed_extensions.is_some() {
        return Ok(None);
    }

    // Read the file content once; reuse for both analysis and extraction.
    let content: Option<String> = if !is_binary {
        match std::fs::read_to_string(path) {
            Ok(s) => Some(s),
            Err(e) => {
                return Err(anyhow::anyhow!("read error: {}", e));
            }
        }
    } else {
        None
    };

    let (total, code, comment, blank) = match &content {
        Some(s) => analyze_content(s, path),
        None => (0, 0, 0, 0),
    };

    // Only populate last_modified when --git-dates is active.
    // Without it, the tree view is cleaner with no date column.
    let last_modified: Option<DateTime<Utc>> = if config.use_git_dates {
        if let Some(ref cache) = config.git_dates_cache {
            cache.get(path).copied()
        } else {
            // git-dates requested but cache not ready (shouldn't happen); fallback
            get_fs_last_modified(path)
        }
    } else {
        None
    };

    let mut fi = FileInfo::new(
        path.to_path_buf(),
        total,
        code,
        comment,
        blank,
        is_binary,
        last_modified,
    );

    if config.extract_functions
        && !is_binary
        && let Some(ref s) = content
    {
        if let Some(extractor) = extractors::get_extractor(path) {
            fi = fi.with_functions(extractor.extract(s));
        }
    }

    Ok(Some(fi))
}

/// Count lines in an already-loaded content string.
///
/// Multi-line comment tracking is language-aware via the comment registry.
/// Handles the Python triple-quote single-liner bug: for equal start/end
/// delimiters (e.g. `"""`), we verify a *second* occurrence exists on the same
/// line before deciding the block closes immediately.
fn analyze_content(content: &str, path: &Path) -> (usize, usize, usize, usize) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    let spec = crate::language::COMMENT_REGISTRY.get(ext.as_str());

    let mut total = 0usize;
    let mut code = 0usize;
    let mut comment = 0usize;
    let mut blank = 0usize;
    let mut in_multi_comment = false;

    for line in content.lines() {
        total += 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if in_multi_comment {
                comment += 1;
            } else {
                blank += 1;
            }
            continue;
        }

        if let Some(s) = spec {
            if in_multi_comment {
                comment += 1;
                if let Some((_, end)) = s.multi
                    && trimmed.contains(end)
                {
                    in_multi_comment = false;
                }
                continue;
            }

            if let Some((start, end)) = s.multi
                && trimmed.starts_with(start)
            {
                comment += 1;

                // Determine whether the multi-line block closes on this same line.
                let ends_on_same_line = if start == end {
                    // Same delimiter on both sides (e.g. Python """...""").
                    // A second occurrence must exist *after* the opening delimiter.
                    trimmed[start.len()..].contains(end)
                } else {
                    // Different delimiters: block closes if end marker appears anywhere
                    // on the line (and it's not just the opening marker itself).
                    trimmed.contains(end)
                };

                if !ends_on_same_line {
                    in_multi_comment = true;
                }
                continue;
            }

            if let Some(single) = s.single
                && trimmed.starts_with(single)
            {
                comment += 1;
                continue;
            }
        }

        code += 1;
    }

    (total, code, comment, blank)
}

// Thin wrapper used by unit tests.
#[cfg(test)]
fn analyze_file(path: &Path) -> (usize, usize, usize, usize) {
    match std::fs::read_to_string(path) {
        Ok(s) => analyze_content(&s, path),
        Err(_) => (0, 0, 0, 0),
    }
}

fn is_binary_file(path: &Path) -> bool {
    // Fast path: extension lookup
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    if BINARY_EXTENSIONS.contains(ext.as_str()) {
        return true;
    }

    // Read first 8 KiB and look for null bytes
    let mut buf = [0u8; 8192];
    match std::fs::File::open(path) {
        Ok(mut f) => {
            use std::io::Read;
            let n = f.read(&mut buf).unwrap_or(0);

            // BOM Check: UTF-16/32 files contain null bytes but are not binary
            if n >= 2 && ((buf[0] == 0xFE && buf[1] == 0xFF) || (buf[0] == 0xFF && buf[1] == 0xFE))
            {
                return false; // UTF-16
            }
            if n >= 4
                && ((buf[0] == 0x00 && buf[1] == 0x00 && buf[2] == 0xFE && buf[3] == 0xFF)
                    || (buf[0] == 0xFF && buf[1] == 0xFE && buf[2] == 0x00 && buf[3] == 0x00))
            {
                return false; // UTF-32
            }

            buf[..n].contains(&0u8)
        }
        Err(_) => true,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Git integration
// ─────────────────────────────────────────────────────────────────────────────

fn check_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn get_git_files(dir: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
        .current_dir(dir)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .split('\0')
                .filter(|s| !s.is_empty())
                .map(|s| dir.join(s))
                .collect()
        }
        _ => get_manual_files(dir, &HashSet::new(), false),
    }
}

fn load_locignore(dir: &Path) -> HashSet<String> {
    let path = dir.join(".locignore");
    if let Ok(content) = std::fs::read_to_string(path) {
        content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.to_string())
            .collect()
    } else {
        HashSet::new()
    }
}

fn get_manual_files(
    dir: &Path,
    custom_ignore: &HashSet<String>,
    include_hidden: bool,
) -> Vec<PathBuf> {
    use walkdir::WalkDir;
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_dir() {
                if EXCLUDED_DIRS.contains(name.as_ref()) || custom_ignore.contains(name.as_ref()) {
                    return false;
                }
                if name == ".git" {
                    return false;
                }
                include_hidden || name == ".well-known" || !name.starts_with('.')
            } else {
                !custom_ignore.contains(name.as_ref()) && (include_hidden || !name.starts_with('.'))
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn get_all_git_dates(root: &Path) -> HashMap<PathBuf, DateTime<Utc>> {
    let mut map = HashMap::new();
    let output = Command::new("git")
        .args(["log", "--format=commit %ct", "--name-only"])
        .current_dir(root)
        .output();

    if let Ok(out) = output
        && out.status.success()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut current_ts = None;
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("commit ") {
                if let Ok(ts) = rest.parse::<i64>() {
                    current_ts = Utc.timestamp_opt(ts, 0).single();
                }
            } else if let Some(ts) = current_ts {
                let path = root.join(line);
                // git log is newest-first; only insert the first (most-recent) date
                map.entry(path).or_insert(ts);
            }
        }
    }
    map
}

fn get_fs_last_modified(path: &Path) -> Option<DateTime<Utc>> {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn count_lines(path: &Path) -> usize {
        analyze_file(path).0
    }

    // ── Basic line counting ──────────────────────────────────────────────────

    #[test]
    fn test_count_lines_basic() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("test.txt");
        fs::write(&p, "line1\nline2\nline3\n").unwrap();
        assert_eq!(count_lines(&p), 3);
    }

    #[test]
    fn test_count_lines_no_trailing_newline() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("test.txt");
        fs::write(&p, "line1\nline2").unwrap();
        assert_eq!(count_lines(&p), 2);
    }

    #[test]
    fn test_count_lines_empty() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("empty.txt");
        fs::write(&p, "").unwrap();
        assert_eq!(count_lines(&p), 0);
    }

    #[test]
    fn test_count_lines_single_line_no_newline() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("single.txt");
        fs::write(&p, "only one line").unwrap();
        assert_eq!(count_lines(&p), 1);
    }

    // ── Binary detection ────────────────────────────────────────────────────

    #[test]
    fn test_is_binary_file_detection() {
        let dir = tempdir().unwrap();

        let txt = dir.path().join("plain.txt");
        fs::write(&txt, "just some text").unwrap();
        assert!(!is_binary_file(&txt));

        let bin = dir.path().join("blob.bin");
        fs::write(&bin, vec![0u8, 1u8, 2u8]).unwrap();
        assert!(is_binary_file(&bin));

        let ext_bin = dir.path().join("image.png");
        fs::write(&ext_bin, "pretend PNG").unwrap();
        assert!(is_binary_file(&ext_bin));
    }

    #[test]
    fn test_is_binary_bom_detection() {
        let dir = tempdir().unwrap();

        let u16be = dir.path().join("utf16be.txt");
        fs::write(&u16be, vec![0xFE, 0xFF, 0x00, 0x61]).unwrap();
        assert!(!is_binary_file(&u16be), "UTF-16BE should not be binary");

        let u16le = dir.path().join("utf16le.txt");
        fs::write(&u16le, vec![0xFF, 0xFE, 0x61, 0x00]).unwrap();
        assert!(!is_binary_file(&u16le), "UTF-16LE should not be binary");

        let u32le = dir.path().join("utf32le.txt");
        fs::write(
            &u32le,
            vec![0xFF, 0xFE, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00],
        )
        .unwrap();
        assert!(!is_binary_file(&u32le), "UTF-32LE should not be binary");
    }

    // ── Manual file walking ──────────────────────────────────────────────────

    #[test]
    fn test_manual_files_with_ignore() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/index.js"), "js").unwrap();
        fs::write(dir.path().join("keep.rs"), "rust").unwrap();
        fs::write(dir.path().join("ignore_me.txt"), "text").unwrap();

        let mut custom_ignore = HashSet::new();
        custom_ignore.insert("ignore_me.txt".to_string());

        let files = get_manual_files(dir.path(), &custom_ignore, false);
        let names: HashSet<_> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(names.contains("keep.rs"));
        assert!(!names.contains("ignore_me.txt"));
        assert!(!names.contains("index.js"));
    }

    // ── Comment classification ───────────────────────────────────────────────

    #[test]
    fn test_python_multiline_comment_counts() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("test.py");
        fs::write(
            &p,
            r#"def foo():
    """
    This is a docstring.
    It spans multiple lines.
    """
    return 42
"#,
        )
        .unwrap();
        let (total, code, comment, blank) = analyze_file(&p);
        // lines() in Rust does not produce a trailing empty element for a
        // terminating newline, so the file yields 6 lines, not 7.
        assert_eq!(total, 6);
        // def foo(): + return 42 = 2 code lines
        assert_eq!(code, 2);
        // opening """, two body lines, closing """ = 4 comment lines
        assert_eq!(comment, 4);
        assert_eq!(blank, 0);
    }

    #[test]
    fn test_python_triple_quote_single_liner() {
        // Regression: """one liner""" must NOT open in_multi_comment
        let dir = tempdir().unwrap();
        let p = dir.path().join("test.py");
        fs::write(
            &p,
            r#"def foo():
    """One liner docstring."""
    x = 1
    y = 2
"#,
        )
        .unwrap();
        let (_total, code, _comment, _blank) = analyze_file(&p);
        // def + x + y = 3 code lines; single-line docstring = 1 comment
        assert_eq!(code, 3, "x = 1 and y = 2 must not be swallowed as comments");
    }

    #[test]
    fn test_rust_comment_classification() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("test.rs");
        fs::write(
            &p,
            r#"// single line comment
fn main() {
    /* block comment */
    let x = 1; // inline not a comment line
}
"#,
        )
        .unwrap();
        let (total, code, comment, _blank) = analyze_file(&p);
        assert_eq!(total, 5);
        assert_eq!(comment, 2); // single-line + block-comment line
        assert_eq!(code, 3); // fn, let x, closing brace
    }
}
