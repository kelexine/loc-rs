// Author: kelexine (https://github.com/kelexine)
// counter.rs — File discovery, line counting, and parallel processing

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rayon::prelude::*;

use crate::cli::Args;
use crate::extractors;
use crate::language::{BINARY_EXTENSIONS, EXCLUDED_DIRS};
use crate::models::{Breakdown, FileInfo, ScanResult};

/// Configuration for a scan run.
#[derive(Clone)]
pub struct ScanConfig {
    pub target_dir: PathBuf,
    pub allowed_extensions: Option<HashSet<String>>,
    pub warn_size: Option<usize>,
    pub use_git_dates: bool,
    pub parallel: bool,
    pub extract_functions: bool,
    pub is_git_repo: bool,
    pub custom_ignore: HashSet<String>,
    pub include_hidden: bool,
    pub git_dates_cache: Option<HashMap<PathBuf, DateTime<Utc>>>,
}

impl ScanConfig {
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
                    // Check if it actually resolves to something known
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

    let mut sorted_files = files;
    sorted_files.sort_unstable();

    let mut runner_config = config.clone();
    if runner_config.use_git_dates && runner_config.is_git_repo {
        runner_config.git_dates_cache = Some(get_all_git_dates(&runner_config.target_dir));
    }

    let file_infos: Vec<FileInfo> = if runner_config.parallel && sorted_files.len() > 50 {
        sorted_files
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
        sorted_files
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

    let mut file_infos = file_infos;
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

    // Skip binary files if we're type-filtering
    if is_binary && config.allowed_extensions.is_some() {
        return Ok(None);
    }

    let (total, code, comment, blank) = if is_binary {
        (0, 0, 0, 0)
    } else {
        analyze_file(path)
    };

    let last_modified = if config.use_git_dates {
        if let Some(ref cache) = config.git_dates_cache {
            cache.get(path).copied()
        } else {
            get_fs_last_modified(path)
        }
    } else {
        get_fs_last_modified(path)
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

    if config.extract_functions && !is_binary {
        let functions = extract_file_functions(path);
        fi = fi.with_functions(functions);
    }

    Ok(Some(fi))
}

fn analyze_file(path: &Path) -> (usize, usize, usize, usize) {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return (0, 0, 0, 0),
    };

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    let spec = crate::language::COMMENT_REGISTRY.get(ext.as_str());

    let mut total = 0;
    let mut code = 0;
    let mut comment = 0;
    let mut blank = 0;

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
                if let Some((_, end)) = s.multi {
                    if trimmed.contains(end) {
                        in_multi_comment = false;
                    }
                }
                continue;
            }

            if let Some((start, end)) = s.multi {
                if trimmed.starts_with(start) {
                    comment += 1;
                    if !trimmed.contains(end) || trimmed.find(start) == trimmed.find(end) {
                         in_multi_comment = true;
                    }
                    continue;
                }
            }

            if let Some(single) = s.single {
                if trimmed.starts_with(single) {
                    comment += 1;
                    continue;
                }
            }
        }

        code += 1;
    }

    // Handle files that don't end with a newline (lines() ignores trailing empty line)
    if content.ends_with('\n') {
        // Correct, lines() gave us the right count
    } else if !content.is_empty() {
        // lines() also gave us the right count for a single line with no newline
    }

    (total, code, comment, blank)
}

fn is_binary_file(path: &Path) -> bool {
    // Check extension first (fast path)
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

            // BOM Check for UTF-16/32 to avoid false positive on null bytes
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

fn extract_file_functions(path: &Path) -> Vec<crate::models::FunctionInfo> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if let Some(ext) = extractors::get_extractor(path) {
                ext.extract(&content)
            } else {
                vec![]
            }
        }
        Err(_) => vec![],
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
        .args(["ls-files", "-z"])
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
    let mut map = std::collections::HashMap::new();
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
                // Insert only if not present (since git log is newest-first)
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

    #[test]
    fn test_count_lines_basic() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line1\nline2\nline3\n").unwrap();
        assert_eq!(count_lines(&file_path), 3);
    }

    #[test]
    fn test_count_lines_no_trailing_newline() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line1\nline2").unwrap();
        assert_eq!(count_lines(&file_path), 2);
    }

    #[test]
    fn test_count_lines_empty() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("empty.txt");
        fs::write(&file_path, "").unwrap();
        assert_eq!(count_lines(&file_path), 0);
    }

    #[test]
    fn test_count_lines_single_line_no_newline() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("single.txt");
        fs::write(&file_path, "only one line").unwrap();
        assert_eq!(count_lines(&file_path), 1);
    }

    #[test]
    fn test_is_binary_file_detection() {
        let dir = tempdir().unwrap();

        let txt_path = dir.path().join("plain.txt");
        fs::write(&txt_path, "just some text").unwrap();
        assert!(!is_binary_file(&txt_path));

        let bin_path = dir.path().join("blob.bin");
        fs::write(&bin_path, vec![0u8, 1u8, 2u8]).unwrap();
        assert!(is_binary_file(&bin_path));

        let ext_bin_path = dir.path().join("image.png");
        fs::write(&ext_bin_path, "pretend PNG").unwrap();
        assert!(is_binary_file(&ext_bin_path));
    }

    #[test]
    fn test_is_binary_bom_detection() {
        let dir = tempdir().unwrap();

        // UTF-16 BE
        let u16be_path = dir.path().join("utf16be.txt");
        fs::write(&u16be_path, vec![0xFE, 0xFF, 0x00, 0x61]).unwrap();
        assert!(
            !is_binary_file(&u16be_path),
            "UTF-16BE should not be binary"
        );

        // UTF-16 LE
        let u16le_path = dir.path().join("utf16le.txt");
        fs::write(&u16le_path, vec![0xFF, 0xFE, 0x61, 0x00]).unwrap();
        assert!(
            !is_binary_file(&u16le_path),
            "UTF-16LE should not be binary"
        );

        // UTF-32 LE
        let u32le_path = dir.path().join("utf32le.txt");
        fs::write(
            &u32le_path,
            vec![0xFF, 0xFE, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00],
        )
        .unwrap();
        assert!(
            !is_binary_file(&u32le_path),
            "UTF-32LE should not be binary"
        );
    }

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
        assert!(!names.contains("index.js")); // should be ignored by hardcoded node_modules exclusion
    }
}
