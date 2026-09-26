// Author: kelexine (https://github.com/kelexine)
// counter/mod.rs — Scan configuration and parallel pipeline orchestrator

pub mod discovery;
pub mod embedded;
pub mod git;
pub mod lines;
pub mod process;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::cli::Args;
use crate::locignore::LocIgnore;
use crate::models::{Breakdown, FileInfo, ScanResult};

use self::discovery::get_manual_files;
use self::process::process_file;

/// Configuration for a scan run.
#[derive(Clone)]
pub struct ScanConfig {
    /// Canonicalized base directory for relative path display and git repo checks.
    pub target_dir: PathBuf,
    /// Explicit target files or directories to scan.
    pub target_paths: Vec<PathBuf>,
    /// Optional extension allowlist (for `-t/--type` filters), including leading dots.
    pub allowed_extensions: Option<HashSet<String>>,
    /// Optional line threshold for "large file" warnings.
    pub warn_size: Option<usize>,
    /// Whether parallel file processing is enabled.
    pub parallel: bool,
    /// Whether function extraction is enabled.
    pub extract_functions: bool,
    /// Compiled `.locignore` ruleset — glob-capable, per-directory, with negation.
    /// Takes precedence over `.gitignore` rules.
    pub locignore: LocIgnore,
    /// Whether hidden files/directories should be included.
    pub include_hidden: bool,
}

impl ScanConfig {
    /// Build a scan configuration from parsed CLI arguments and global config.
    pub fn from_args(args: &Args) -> Result<Self> {
        let mut target_paths = Vec::new();
        for p in &args.paths {
            let canon = Path::new(p)
                .canonicalize()
                .with_context(|| format!("Cannot resolve path: {}", p))?;
            target_paths.push(canon);
        }

        // Determine base target_dir for relative paths and git checks:
        // If single directory is provided, use it directly.
        // Otherwise, use current working directory (or common ancestor).
        let target_dir = if target_paths.len() == 1 && target_paths[0].is_dir() {
            target_paths[0].clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| {
                target_paths
                    .first()
                    .and_then(|p| p.parent())
                    .unwrap_or(Path::new("."))
                    .to_path_buf()
            })
        };

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

        let locignore = LocIgnore::build(&target_dir);
        let warn_size = args.warn_size.or(global_config.warn_size);
        let extract_functions = args.functions
            || args.func_analysis
            || global_config.always_extract_functions.unwrap_or(false);

        Ok(Self {
            target_dir,
            target_paths,
            allowed_extensions,
            warn_size,
            parallel: !args.no_parallel,
            extract_functions,
            locignore,
            include_hidden: args.include_hidden,
        })
    }
}

/// Run the full scan and return a ScanResult.
pub fn run_scan(config: &ScanConfig) -> Result<ScanResult> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();

    let paths: &[PathBuf] = if config.target_paths.is_empty() {
        std::slice::from_ref(&config.target_dir)
    } else {
        &config.target_paths
    };

    for path in paths {
        if path.is_file() {
            if seen.insert(path.clone()) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            let dir_files = get_manual_files(path, &config.locignore, config.include_hidden);
            for f in dir_files {
                if seen.insert(f.clone()) {
                    files.push(f);
                }
            }
        }
    }

    let mut file_infos: Vec<FileInfo> = if config.parallel && files.len() > 50 {
        files
            .par_iter()
            .filter_map(|path| match process_file(path, config) {
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
            .filter_map(|path| match process_file(path, config) {
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
        if fi.is_binary || fi.is_lockfile {
            continue;
        }

        let container_ext = fi.language_key().to_string();

        if fi.embedded.is_empty() {
            let stats = breakdown.entry(container_ext).or_default();
            stats.lines += fi.lines;
            stats.code += fi.code;
            stats.comment += fi.comment;
            stats.blank += fi.blank;
            stats.files += 1;
            stats.functions += fi.function_count();
        } else {
            // Container file count
            breakdown.entry(container_ext).or_default().files += 1;

            for chunk in &fi.embedded {
                let chunk_lang =
                    crate::language::canonical_language_name(&chunk.extension).to_string();
                let stats = breakdown.entry(chunk_lang).or_default();
                stats.lines += chunk.lines;
                stats.code += chunk.code;
                stats.comment += chunk.comment;
                stats.blank += chunk.blank;
            }
        }
    }

    Ok(ScanResult {
        files: file_infos,
        breakdown,
    })
}

#[cfg(test)]
mod tests {
    use super::discovery::get_manual_files;
    use super::git::check_git_repo;
    use super::lines::analyze_file;
    use super::process::is_binary_file;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
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

    #[test]
    fn test_count_lines_non_utf8_encoding() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("latin1.c");
        // Non-UTF-8 bytes (e.g. ISO-8859-1 'é' = 0xE9) without null bytes
        fs::write(
            &p,
            vec![
                b'/', b'/', b' ', 0xE9, b'\n', b'i', b'n', b't', b' ', b'x', b';', b'\n',
            ],
        )
        .unwrap();
        let (total, code, comment, _) = analyze_file(&p);
        assert_eq!(total, 2);
        assert_eq!(comment, 1);
        assert_eq!(code, 1);
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
        fs::write(&u32le, vec![0xFF, 0xFE, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00]).unwrap();
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
        fs::write(dir.path().join(".locignore"), "ignore_me.txt\n").unwrap();

        let locignore = crate::locignore::LocIgnore::build(dir.path());
        let files = get_manual_files(dir.path(), &locignore, false);
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
        assert_eq!(total, 6);
        assert_eq!(code, 2);
        assert_eq!(comment, 4);
        assert_eq!(blank, 0);
    }

    #[test]
    fn test_python_triple_quote_single_liner() {
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
        assert_eq!(comment, 2);
        assert_eq!(code, 3);
    }

    // ── Git integration ──────────────────────────────────────────────────────

    #[test]
    fn test_check_git_repo() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // Not a repo initially
        assert!(!check_git_repo(path));

        // Create .git directory
        let git_dir = path.join(".git");
        fs::create_dir(&git_dir).unwrap();
        assert!(check_git_repo(path));

        // Subdirectories should also detect the parent git repo
        let subdir = path.join("src").join("nested");
        fs::create_dir_all(&subdir).unwrap();
        assert!(check_git_repo(&subdir));

        // Files within the repo should detect the git repo
        let file = subdir.join("main.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        assert!(check_git_repo(&file));
    }

    // ── Embedded Languages & Jupyter Notebooks ────────────────────────────────

    #[test]
    fn test_html_embedded_script_and_style() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("index.html");
        fs::write(
            &p,
            r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body { color: red; }
        /* css comment */
    </style>
</head>
<body>
    <script>
        // js comment
        const greeting = "Hello";
        console.log(greeting);
    </script>
</body>
</html>
"#,
        )
        .unwrap();

        let config = super::ScanConfig {
            target_dir: dir.path().to_path_buf(),
            target_paths: Vec::new(),
            allowed_extensions: None,
            warn_size: None,
            parallel: false,
            extract_functions: false,
            locignore: crate::locignore::LocIgnore::build(dir.path()),
            include_hidden: false,
        };

        let result = super::run_scan(&config).unwrap();
        assert!(result.breakdown.contains_key("HTML"));
        assert!(result.breakdown.contains_key("JavaScript"));
        assert!(result.breakdown.contains_key("CSS"));

        let js_stats = &result.breakdown["JavaScript"];
        assert_eq!(js_stats.code, 2);
        assert_eq!(js_stats.comment, 1);
        assert_eq!(js_stats.blank, 0);

        let css_stats = &result.breakdown["CSS"];
        assert_eq!(css_stats.code, 1);
        assert_eq!(css_stats.comment, 1);
        assert_eq!(css_stats.blank, 0);
    }

    #[test]
    fn test_jupyter_notebook_parsing() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("notebook.ipynb");
        fs::write(
            &p,
            r##"{
 "cells": [
  {
   "cell_type": "markdown",
   "metadata": {},
   "source": [
    "# Notebook Title\n",
    "Description of notebook"
   ]
  },
  {
   "cell_type": "code",
   "execution_count": 1,
   "metadata": {},
   "outputs": [],
   "source": [
    "# Python cell comment\n",
    "def add(a, b):\n",
    "    return a + b\n"
   ]
  }
 ],
 "metadata": {
  "language_info": {
   "name": "python"
  }
 },
 "nbformat": 4,
 "nbformat_minor": 2
}"##,
        )
        .unwrap();

        let config = super::ScanConfig {
            target_dir: dir.path().to_path_buf(),
            target_paths: Vec::new(),
            allowed_extensions: None,
            warn_size: None,
            parallel: false,
            extract_functions: false,
            locignore: crate::locignore::LocIgnore::build(dir.path()),
            include_hidden: false,
        };

        let result = super::run_scan(&config).unwrap();
        assert!(result.breakdown.contains_key("Jupyter"));
        assert!(result.breakdown.contains_key("Python"));
        assert!(result.breakdown.contains_key("Markdown"));

        let py_stats = &result.breakdown["Python"];
        assert_eq!(py_stats.code, 2);
        assert_eq!(py_stats.comment, 1);

        let md_stats = &result.breakdown["Markdown"];
        assert_eq!(md_stats.code, 2);
    }

    #[test]
    fn test_utf_encodings_accurate_counting() {
        use crate::counter::process::process_file;
        let dir = tempdir().unwrap();

        let source =
            "// comment 1\n// comment 2\n/* comment 3 */\nlet code1 = 1;\nlet code2 = 2;\n\n";

        let config = super::ScanConfig {
            target_dir: dir.path().to_path_buf(),
            target_paths: Vec::new(),
            allowed_extensions: None,
            warn_size: None,
            parallel: false,
            extract_functions: false,
            locignore: crate::locignore::LocIgnore::build(dir.path()),
            include_hidden: false,
        };

        // 1. UTF-8 with BOM
        let p_utf8_bom = dir.path().join("test_bom.rs");
        let mut utf8_bom_bytes = vec![0xEF, 0xBB, 0xBF];
        utf8_bom_bytes.extend_from_slice(source.as_bytes());
        fs::write(&p_utf8_bom, &utf8_bom_bytes).unwrap();
        let fi = process_file(&p_utf8_bom, &config).unwrap().unwrap();
        assert_eq!((fi.code, fi.comment, fi.blank), (2, 3, 1));

        // 2. UTF-16LE with BOM
        let p_utf16le = dir.path().join("test_le.rs");
        let mut utf16le_bytes = vec![0xFF, 0xFE];
        for c in source.encode_utf16() {
            utf16le_bytes.extend_from_slice(&c.to_le_bytes());
        }
        fs::write(&p_utf16le, &utf16le_bytes).unwrap();
        let fi = process_file(&p_utf16le, &config).unwrap().unwrap();
        assert_eq!((fi.code, fi.comment, fi.blank), (2, 3, 1));

        // 3. UTF-16BE with BOM
        let p_utf16be = dir.path().join("test_be.rs");
        let mut utf16be_bytes = vec![0xFE, 0xFF];
        for c in source.encode_utf16() {
            utf16be_bytes.extend_from_slice(&c.to_be_bytes());
        }
        fs::write(&p_utf16be, &utf16be_bytes).unwrap();
        let fi = process_file(&p_utf16be, &config).unwrap().unwrap();
        assert_eq!((fi.code, fi.comment, fi.blank), (2, 3, 1));

        // 4. UTF-32LE with BOM
        let p_utf32le = dir.path().join("test_32le.rs");
        let mut utf32le_bytes = vec![0xFF, 0xFE, 0x00, 0x00];
        for c in source.chars() {
            utf32le_bytes.extend_from_slice(&(c as u32).to_le_bytes());
        }
        fs::write(&p_utf32le, &utf32le_bytes).unwrap();
        let fi = process_file(&p_utf32le, &config).unwrap().unwrap();
        assert_eq!((fi.code, fi.comment, fi.blank), (2, 3, 1));

        // 5. UTF-32BE with BOM
        let p_utf32be = dir.path().join("test_32be.rs");
        let mut utf32be_bytes = vec![0x00, 0x00, 0xFE, 0xFF];
        for c in source.chars() {
            utf32be_bytes.extend_from_slice(&(c as u32).to_be_bytes());
        }
        fs::write(&p_utf32be, &utf32be_bytes).unwrap();
        let fi = process_file(&p_utf32be, &config).unwrap().unwrap();
        assert_eq!((fi.code, fi.comment, fi.blank), (2, 3, 1));

        // 6. UTF-16LE without BOM (heuristic)
        let p_utf16le_nobom = dir.path().join("test_nobom_le.rs");
        let mut utf16le_nobom = Vec::new();
        for c in source.encode_utf16() {
            utf16le_nobom.extend_from_slice(&c.to_le_bytes());
        }
        fs::write(&p_utf16le_nobom, &utf16le_nobom).unwrap();
        let fi = process_file(&p_utf16le_nobom, &config).unwrap().unwrap();
        assert_eq!((fi.code, fi.comment, fi.blank), (2, 3, 1));
    }

    #[test]
    fn test_remote_agent_accuracy_fixtures() {
        use crate::counter::lines::analyze_content_with_spec;
        use crate::language::COMMENT_REGISTRY;

        let rs_spec = COMMENT_REGISTRY.get(".rs");

        // Fixture: Code, then /* start on same line (Expected: 2 code, 2 comment, 0 blank)
        let fixture_code_then_block = "let a = 1; /* start\ncontinue comment */\nlet b = 2; /* start 2\ncontinue comment 2 */";
        let (total, code, comment, blank) =
            analyze_content_with_spec(fixture_code_then_block, rs_spec);
        assert_eq!((code, comment, blank, total), (2, 2, 0, 4));

        // Fixture: /* c */ code on one line (Expected: 1 code, 0 comment, 0 blank)
        let fixture_inline_block = "/* inline comment */ let x = 42;";
        let (total, code, comment, blank) =
            analyze_content_with_spec(fixture_inline_block, rs_spec);
        assert_eq!((code, comment, blank, total), (1, 0, 0, 1));

        // Fixture: /* at line start inside a string (Expected: 4 code, 0 comment, 0 blank)
        let fixture_string_comment = "let s = \"\n/* not comment\nstill string\n\";";
        let (total, code, comment, blank) =
            analyze_content_with_spec(fixture_string_comment, rs_spec);
        assert_eq!((code, comment, blank, total), (4, 0, 0, 4));

        // Fixture: Nested block comments (Expected: 1 code, 4 comment, 0 blank)
        let fixture_nested = "/* outer\n   /* nested */\n   still comment\n*/\nfn main() {}";
        let (total, code, comment, blank) = analyze_content_with_spec(fixture_nested, rs_spec);
        assert_eq!((code, comment, blank, total), (1, 4, 0, 5));
    }
}
