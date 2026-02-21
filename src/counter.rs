// Author: kelexine (https://github.com/kelexine)
// counter.rs — File discovery, line counting, and parallel processing

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::process::Command;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc, TimeZone};
use rayon::prelude::*;

use crate::cli::Args;
use crate::extractors;
use crate::language::{BINARY_EXTENSIONS, EXCLUDED_DIRS};
use crate::models::{Breakdown, ExtensionStats, FileInfo, ScanResult};

/// Configuration for a scan run.
pub struct ScanConfig {
    pub target_dir: PathBuf,
    pub allowed_extensions: Option<HashSet<String>>,
    pub warn_size: Option<usize>,
    pub use_git_dates: bool,
    pub parallel: bool,
    pub extract_functions: bool,
    pub is_git_repo: bool,
    pub custom_ignore: HashSet<String>,
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

        // Build allowed extension set from language filter flags
        let allowed_extensions = if args.file_types.is_empty() {
            None
        } else {
            let mut exts = HashSet::new();
            for lang in &args.file_types {
                let resolved = crate::language::resolve_extensions(lang);
                if resolved.is_empty() || (resolved.len() == 1 && resolved[0] == format!(".{}", lang)) {
                    // Check if it actually resolves to something known
                    eprintln!("[WARNING] Unknown language filter: {}", lang);
                }
                exts.extend(resolved);
            }
            Some(exts)
        };

        let custom_ignore = load_locignore(&target_dir);

        Ok(Self {
            target_dir,
            allowed_extensions,
            warn_size: args.warn_size,
            use_git_dates: args.git_dates,
            parallel: !args.no_parallel,
            extract_functions: args.functions || args.func_analysis,
            is_git_repo,
            custom_ignore,
        })
    }
}

/// Run the full scan and return a ScanResult.
pub fn run_scan(config: &ScanConfig) -> Result<ScanResult> {
    let files = if config.is_git_repo {
        get_git_files(&config.target_dir)
    } else {
        get_manual_files(&config.target_dir, &config.custom_ignore)
    };

    let mut sorted_files = files;
    sorted_files.sort_unstable();

    let file_infos: Vec<FileInfo> = if config.parallel && sorted_files.len() > 50 {
        sorted_files
            .par_iter()
            .filter_map(|path| process_file(path, config).ok().flatten())
            .collect()
    } else {
        sorted_files
            .iter()
            .filter_map(|path| process_file(path, config).ok().flatten())
            .collect()
    };

    let mut file_infos = file_infos;
    file_infos.sort_by(|a, b| a.path.cmp(&b.path));

    // Build breakdown
    let mut breakdown: Breakdown = std::collections::HashMap::new();
    for fi in &file_infos {
        if fi.is_binary { continue; }
        let ext = if fi.extension().is_empty() {
            fi.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("(no ext)")
                .to_string()
        } else {
            fi.extension().to_string()
        };
        let stats = breakdown.entry(ext).or_insert_with(ExtensionStats::default);
        stats.lines += fi.lines;
        stats.files += 1;
        stats.functions += fi.function_count();
    }

    Ok(ScanResult { files: file_infos, breakdown })
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
        let ext = path.extension()
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

    let lines = if is_binary { 0 } else { count_lines(path) };
    let last_modified = if config.use_git_dates {
        get_git_last_modified(path, &config.target_dir)
    } else {
        get_fs_last_modified(path)
    };

    let mut fi = FileInfo::new(path.to_path_buf(), lines, is_binary, last_modified);

    if config.extract_functions && !is_binary {
        let functions = extract_file_functions(path);
        fi = fi.with_functions(functions);
    }

    Ok(Some(fi))
}

fn count_lines(path: &Path) -> usize {
    let content = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    // Fast byte-level newline count
    content.iter().filter(|&&b| b == b'\n').count() + {
        // Count last line if it doesn't end with newline
        if content.last().map(|&b| b != b'\n').unwrap_or(false) { 1 } else { 0 }
    }
}

fn is_binary_file(path: &Path) -> bool {
    // Check extension first (fast path)
    let ext = path.extension()
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
            if n >= 2 && ((buf[0] == 0xFE && buf[1] == 0xFF) || (buf[0] == 0xFF && buf[1] == 0xFE)) {
                return false; // UTF-16
            }
            if n >= 4 && ((buf[0] == 0x00 && buf[1] == 0x00 && buf[2] == 0xFE && buf[3] == 0xFF) || 
                          (buf[0] == 0xFF && buf[1] == 0xFE && buf[2] == 0x00 && buf[3] == 0x00)) {
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
        _ => get_manual_files(dir, &HashSet::new()),
    }
}

fn load_locignore(dir: &Path) -> HashSet<String> {
    let path = dir.join(".locignore");
    if let Ok(content) = std::fs::read_to_string(path) {
        content.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.to_string())
            .collect()
    } else {
        HashSet::new()
    }
}

fn get_manual_files(dir: &Path, custom_ignore: &HashSet<String>) -> Vec<PathBuf> {
    use walkdir::WalkDir;
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                if EXCLUDED_DIRS.contains(name.as_ref()) || custom_ignore.contains(name.as_ref()) {
                    return false;
                }
                name == ".well-known" || !name.starts_with('.')
            } else {
                let name = e.file_name().to_string_lossy();
                !custom_ignore.contains(name.as_ref())
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn get_git_last_modified(path: &Path, root: &Path) -> Option<DateTime<Utc>> {
    let rel = path.strip_prefix(root).ok()?;
    let output = Command::new("git")
        .args(["log", "-1", "--format=%ct", "--", rel.to_str()?])
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() { return None; }
    let ts: i64 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;
    Utc.timestamp_opt(ts, 0).single()
}

fn get_fs_last_modified(path: &Path) -> Option<DateTime<Utc>> {
    path.metadata().ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

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
        assert!(!is_binary_file(&u16be_path), "UTF-16BE should not be binary");

        // UTF-16 LE
        let u16le_path = dir.path().join("utf16le.txt");
        fs::write(&u16le_path, vec![0xFF, 0xFE, 0x61, 0x00]).unwrap();
        assert!(!is_binary_file(&u16le_path), "UTF-16LE should not be binary");

        // UTF-32 LE
        let u32le_path = dir.path().join("utf32le.txt");
        fs::write(&u32le_path, vec![0xFF, 0xFE, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00]).unwrap();
        assert!(!is_binary_file(&u32le_path), "UTF-32LE should not be binary");
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
        
        let files = get_manual_files(dir.path(), &custom_ignore);
        let names: HashSet<_> = files.iter().map(|f| f.file_name().unwrap().to_str().unwrap()).collect();
        
        assert!(names.contains("keep.rs"));
        assert!(!names.contains("ignore_me.txt"));
        assert!(!names.contains("index.js")); // should be ignored by hardcoded node_modules exclusion
    }
}
