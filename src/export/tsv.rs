// Author: kelexine (https://github.com/kelexine)
// export/tsv.rs — TSV export logic, shared by agent-mode stdout and `-e out.tsv`
//
// Both surfaces (stdout `--format agent` and file export) delegate to the
// same section writers below so schema drift between the two is
// structurally impossible — same flags in, byte-identical section layout
// out. The only difference is *which* sections get written: stdout
// respects the interactive `-d`/`-t`/`--func-analysis` flags, while file
// export always writes every section since there's no live terminal to
// re-run against.

use crate::models::{FunctionInfo, ScanResult};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export scan results as TSV to `path`.
///
/// Always writes `# SUMMARY`, `# BREAKDOWN`, `# FILES`, and — when
/// `func_analysis` is true and functions were extracted — the full
/// `# FUNCTION_STATS` / `# LARGEST_FUNCTIONS` / `# HIGH_COMPLEXITY` /
/// `# TOP_FILES` block. Unlike stdout agent mode, file export is not
/// gated by `-d`/`-t`: a file with only half the picture because a flag
/// was missing is worse than a slightly larger file.
///
/// ```text
/// # SUMMARY
/// metric\tvalue
/// total_lines\t12345
/// ...
/// scan_dir\t/abs/path/to/project
///
/// # BREAKDOWN
/// extension\tfiles\tlines\tcode\tcomment\tblank\tfunctions\tpct_lines
/// rs\t15\t8000\t6000\t1500\t500\t42\t64.90%
/// ...
///
/// # FILES
/// path\tlines\tcode\tcomment\tblank\textension\tis_binary\tis_lockfile\tfunctions\tclasses\tavg_fn_length\tlast_modified
/// src/main.rs\t97\t80\t5\t12\trs\tfalse\tfalse\t3\t0\t18.50\t2025-01-01T00:00:00Z
/// ...
///
/// # FUNCTION_STATS
/// ...
/// ```
pub fn export_tsv(
    result: &ScanResult,
    path: &Path,
    root: &Path,
    extract_functions: bool,
    func_analysis: bool,
    warn_size: Option<usize>,
) -> Result<()> {
    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut w = BufWriter::new(f);

    write_summary_section(&mut w, result, root, warn_size)?;
    writeln!(w)?;
    write_breakdown_section(&mut w, result, extract_functions)?;
    writeln!(w)?;
    write_files_section(&mut w, result, root, extract_functions)?;

    if func_analysis {
        writeln!(w)?;
        write_function_analysis_sections(&mut w, result, root)?;
    }

    println!("[SUCCESS] Exported TSV → {}", path.display());
    Ok(())
}

/// Write the `# SUMMARY` section.
///
/// `root` is always recorded as `scan_dir` so a file consumer knows what
/// the (now-relative) `# FILES` paths are relative to. `warn_size`, when
/// set, adds a `large_files_over_N` count — matching agent stdout.
pub fn write_summary_section<W: Write>(
    w: &mut W,
    result: &ScanResult,
    root: &Path,
    warn_size: Option<usize>,
) -> Result<()> {
    writeln!(w, "# SUMMARY")?;
    writeln!(w, "metric\tvalue")?;
    writeln!(w, "total_lines\t{}", result.total_lines())?;
    writeln!(w, "total_code\t{}", result.total_code())?;
    writeln!(w, "total_comment\t{}", result.total_comment())?;
    writeln!(w, "total_blank\t{}", result.total_blank())?;
    writeln!(w, "text_files\t{}", result.text_file_count())?;
    writeln!(w, "binary_files\t{}", result.binary_file_count())?;
    writeln!(w, "lockfiles\t{}", result.lockfile_count())?;
    writeln!(w, "total_functions\t{}", result.total_functions())?;
    writeln!(w, "total_classes\t{}", result.total_classes())?;
    writeln!(w, "scan_dir\t{}", root.display())?;

    if let Some(ws) = warn_size {
        let large = result.files.iter().filter(|f| f.lines > ws).count();
        writeln!(w, "large_files_over_{}\t{}", ws, large)?;
    }
    Ok(())
}

/// Write the `# BREAKDOWN` section.
///
/// `include_functions` adds a `functions` column between `blank` and
/// `pct_lines` — mirrors [`crate::models::ExtensionStats::functions`].
pub fn write_breakdown_section<W: Write>(
    w: &mut W,
    result: &ScanResult,
    include_functions: bool,
) -> Result<()> {
    writeln!(w, "# BREAKDOWN")?;
    if include_functions {
        writeln!(w, "extension\tfiles\tlines\tcode\tcomment\tblank\tfunctions\tpct_lines")?;
    } else {
        writeln!(w, "extension\tfiles\tlines\tcode\tcomment\tblank\tpct_lines")?;
    }

    let total_lines = result.total_lines();
    let mut entries: Vec<_> = result.breakdown.iter().collect();
    entries.sort_by_key(|(_, stats)| std::cmp::Reverse(stats.lines));

    for (ext, stats) in &entries {
        let pct = if total_lines > 0 {
            format!("{:.2}%", stats.lines as f64 / total_lines as f64 * 100.0)
        } else {
            "0.00%".to_string()
        };
        if include_functions {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                ext, stats.files, stats.lines, stats.code, stats.comment, stats.blank,
                stats.functions, pct
            )?;
        } else {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                ext, stats.files, stats.lines, stats.code, stats.comment, stats.blank, pct
            )?;
        }
    }
    Ok(())
}

/// Write the `# FILES` section with paths relative to `root`.
///
/// All files are emitted (binary and lockfiles included) so the output
/// mirrors the stdout agent view exactly; callers filter on
/// `is_binary`/`is_lockfile` as needed. Falls back to the absolute path
/// if a file isn't actually under `root` (defensive — shouldn't happen
/// given the scan always walks from `root`, but a silent panic on a
/// malformed path is worse than a slightly wrong column).
pub fn write_files_section<W: Write>(
    w: &mut W,
    result: &ScanResult,
    root: &Path,
    include_functions: bool,
) -> Result<()> {
    writeln!(w, "# FILES")?;
    if include_functions {
        writeln!(
            w,
            "path\tlines\tcode\tcomment\tblank\textension\tis_binary\tis_lockfile\tfunctions\tclasses\tavg_fn_length\tlast_modified"
        )?;
    } else {
        writeln!(
            w,
            "path\tlines\tcode\tcomment\tblank\textension\tis_binary\tis_lockfile\tlast_modified"
        )?;
    }

    for fi in &result.files {
        let rel = fi
            .path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| fi.path.display().to_string());
        let modified = fi
            .last_modified
            .map(|d| d.to_rfc3339())
            .unwrap_or_default();

        if include_functions {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{}",
                rel,
                fi.lines,
                fi.code,
                fi.comment,
                fi.blank,
                fi.extension(),
                fi.is_binary,
                fi.is_lockfile,
                fi.function_count(),
                fi.class_count(),
                fi.avg_function_length(),
                modified,
            )?;
        } else {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                rel,
                fi.lines,
                fi.code,
                fi.comment,
                fi.blank,
                fi.extension(),
                fi.is_binary,
                fi.is_lockfile,
                modified,
            )?;
        }
    }
    Ok(())
}

/// Write the function-analysis block: `# FUNCTION_STATS`,
/// `# LARGEST_FUNCTIONS` (top 10 by line count), `# HIGH_COMPLEXITY`
/// (cyclomatic complexity > 10, top 15), and `# TOP_FILES` (top 10 by
/// function count). Ported 1:1 from the former
/// `display::display_agent_function_analysis` so stdout and file output
/// are structurally identical given the same input.
///
/// `# FUNCTION_STATS` is always written. The remaining three sections are
/// skipped entirely if no file has any extracted functions — an empty
/// `# LARGEST_FUNCTIONS` header with zero rows is noise, not data.
pub fn write_function_analysis_sections<W: Write>(
    w: &mut W,
    result: &ScanResult,
    root: &Path,
) -> Result<()> {
    let files_with_fns: Vec<_> = result
        .files
        .iter()
        .filter(|f| f.function_count() > 0)
        .collect();

    // ── FUNCTION_STATS ───────────────────────────────────────────────────
    writeln!(w, "# FUNCTION_STATS")?;
    writeln!(w, "metric\tvalue")?;
    writeln!(w, "total_functions\t{}", result.total_functions())?;
    writeln!(w, "total_classes\t{}", result.total_classes())?;

    let non_class_fns: Vec<_> = files_with_fns
        .iter()
        .flat_map(|f| f.functions.iter().filter(|fn_| !fn_.is_class))
        .collect();
    let avg_len = if non_class_fns.is_empty() {
        0.0_f64
    } else {
        non_class_fns.iter().map(|f| f.line_count()).sum::<usize>() as f64
            / non_class_fns.len() as f64
    };
    writeln!(w, "avg_function_length\t{:.2}", avg_len)?;

    if files_with_fns.is_empty() {
        return Ok(());
    }

    // ── LARGEST_FUNCTIONS (top 10) ───────────────────────────────────────
    let mut all_fns: Vec<(&Path, &FunctionInfo)> = files_with_fns
        .iter()
        .flat_map(|fi| {
            fi.functions
                .iter()
                .filter(|f| !f.is_class)
                .map(move |f| (fi.path.as_path(), f))
        })
        .collect();
    all_fns.sort_by_key(|(_, func)| std::cmp::Reverse(func.line_count()));

    writeln!(w)?;
    writeln!(w, "# LARGEST_FUNCTIONS")?;
    writeln!(w, "function\tfile\tlines\tcomplexity\tparams")?;
    for (path, func) in all_fns.iter().take(10) {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        writeln!(
            w,
            "{}\t{}\t{}\t{}\t{}",
            func.name,
            rel,
            func.line_count(),
            func.complexity,
            func.parameters.join(", "),
        )?;
    }

    // ── HIGH_COMPLEXITY (cc > 10, top 15) ────────────────────────────────
    let mut complex: Vec<_> = files_with_fns
        .iter()
        .flat_map(|fi| {
            fi.functions
                .iter()
                .filter(|f| !f.is_class && f.complexity > 10)
                .map(move |f| (fi.path.as_path(), f))
        })
        .collect();

    if !complex.is_empty() {
        complex.sort_by_key(|(_, func)| std::cmp::Reverse(func.complexity));
        writeln!(w)?;
        writeln!(w, "# HIGH_COMPLEXITY")?;
        writeln!(w, "function\tfile\tcomplexity")?;
        for (path, func) in complex.iter().take(15) {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            writeln!(w, "{}\t{}\t{}", func.name, rel, func.complexity)?;
        }
    }

    // ── TOP_FILES (top 10 by function count) ─────────────────────────────
    let mut sorted_files = files_with_fns.clone();
    sorted_files.sort_by_key(|f| std::cmp::Reverse(f.function_count()));

    writeln!(w)?;
    writeln!(w, "# TOP_FILES")?;
    writeln!(w, "file\tfunctions\tclasses\tavg_fn_length")?;
    for fi in sorted_files.iter().take(10) {
        let rel = fi
            .path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| fi.path.display().to_string());
        writeln!(
            w,
            "{}\t{}\t{}\t{:.2}",
            rel,
            fi.function_count(),
            fi.class_count(),
            fi.avg_function_length(),
        )?;
    }

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ExtensionStats, FileInfo, FunctionInfo};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_result_with_root() -> (ScanResult, PathBuf) {
        let root = PathBuf::from("/repo");
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "rs".to_string(),
            ExtensionStats { lines: 100, code: 80, comment: 10, blank: 10, files: 1, functions: 1 },
        );
        let file = FileInfo::new(
            PathBuf::from("/repo/src/main.rs"),
            100,
            80,
            10,
            10,
            false,
            None,
        )
        .with_functions(vec![FunctionInfo {
            name: "main".to_string(),
            line_start: 1,
            line_end: 20,
            parameters: vec![],
            is_async: false,
            is_method: false,
            is_class: false,
            docstring: None,
            decorators: vec![],
            complexity: 15,
        }]);
        (
            ScanResult { files: vec![file], breakdown },
            root,
        )
    }

    fn make_result() -> ScanResult {
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "rs".to_string(),
            ExtensionStats { lines: 100, code: 80, comment: 10, blank: 10, files: 2, functions: 5 },
        );
        ScanResult {
            files: vec![FileInfo::new(
                PathBuf::from("src/main.rs"),
                100,
                80,
                10,
                10,
                false,
                None,
            )],
            breakdown,
        }
    }

    #[test]
    fn summary_section_has_header_values_and_scan_dir() {
        let result = make_result();
        let mut buf = Vec::new();
        write_summary_section(&mut buf, &result, Path::new("/repo"), None).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.starts_with("# SUMMARY\n"));
        assert!(out.contains("metric\tvalue\n"));
        assert!(out.contains("total_lines\t100\n"));
        assert!(out.contains("total_code\t80\n"));
        assert!(out.contains("scan_dir\t/repo\n"));
    }

    #[test]
    fn summary_section_includes_large_files_when_warn_size_set() {
        let result = make_result();
        let mut buf = Vec::new();
        write_summary_section(&mut buf, &result, Path::new("/repo"), Some(50)).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("large_files_over_50\t1"));
    }

    #[test]
    fn summary_section_omits_large_files_when_warn_size_unset() {
        let result = make_result();
        let mut buf = Vec::new();
        write_summary_section(&mut buf, &result, Path::new("/repo"), None).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(!out.contains("large_files_over"));
    }

    #[test]
    fn breakdown_section_sorted_by_lines_desc() {
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "py".to_string(),
            ExtensionStats { lines: 50, code: 40, comment: 5, blank: 5, files: 1, functions: 0 },
        );
        breakdown.insert(
            "rs".to_string(),
            ExtensionStats { lines: 200, code: 160, comment: 20, blank: 20, files: 3, functions: 10 },
        );
        let result = ScanResult { files: vec![], breakdown };
        let mut buf = Vec::new();
        write_breakdown_section(&mut buf, &result, false).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let rs_pos = out.find("rs\t").unwrap();
        let py_pos = out.find("py\t").unwrap();
        assert!(rs_pos < py_pos, "rs (200 lines) should precede py (50 lines)");
    }

    #[test]
    fn breakdown_pct_sums_to_100() {
        let result = make_result();
        let mut buf = Vec::new();
        write_breakdown_section(&mut buf, &result, false).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("100.00%"), "Single-ext pct should be 100.00%:\n{}", out);
    }

    #[test]
    fn breakdown_section_includes_functions_column_when_requested() {
        let result = make_result();
        let mut buf = Vec::new();
        write_breakdown_section(&mut buf, &result, true).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let header = out.lines().nth(1).unwrap();
        assert_eq!(header, "extension\tfiles\tlines\tcode\tcomment\tblank\tfunctions\tpct_lines");
        // rs row: files=2, lines=100, code=80, comment=10, blank=10, functions=5
        assert!(out.lines().any(|l| l == "rs\t2\t100\t80\t10\t10\t5\t100.00%"));
    }

    #[test]
    fn files_section_no_functions_has_9_columns() {
        let result = make_result();
        let mut buf = Vec::new();
        write_files_section(&mut buf, &result, Path::new(""), false).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let header = out.lines().nth(1).unwrap();
        assert_eq!(header.split('\t').count(), 9);
    }

    #[test]
    fn files_section_with_functions_has_12_columns() {
        let result = make_result();
        let mut buf = Vec::new();
        write_files_section(&mut buf, &result, Path::new(""), true).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let header = out.lines().nth(1).unwrap();
        assert_eq!(header.split('\t').count(), 12);
    }

    #[test]
    fn files_section_strips_root_prefix_to_relative_path() {
        let (result, root) = make_result_with_root();
        let mut buf = Vec::new();
        write_files_section(&mut buf, &result, &root, false).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("src/main.rs\t"), "expected relative path:\n{}", out);
        assert!(!out.contains("/repo/src/main.rs"), "path should not remain absolute:\n{}", out);
    }

    #[test]
    fn files_section_falls_back_to_absolute_when_not_under_root() {
        let result = make_result(); // file path "src/main.rs", root "/other" won't be a prefix
        let mut buf = Vec::new();
        write_files_section(&mut buf, &result, Path::new("/other"), false).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("src/main.rs\t"));
    }

    #[test]
    fn function_analysis_sections_present_when_functions_exist() {
        let (result, root) = make_result_with_root();
        let mut buf = Vec::new();
        write_function_analysis_sections(&mut buf, &result, &root).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("# FUNCTION_STATS"));
        assert!(out.contains("# LARGEST_FUNCTIONS"));
        assert!(out.contains("# HIGH_COMPLEXITY")); // complexity 15 > 10
        assert!(out.contains("# TOP_FILES"));
        assert!(out.contains("main\tsrc/main.rs\t"));
    }

    #[test]
    fn function_analysis_sections_stats_only_when_no_functions() {
        let result = make_result(); // no functions attached
        let mut buf = Vec::new();
        write_function_analysis_sections(&mut buf, &result, Path::new("")).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("# FUNCTION_STATS"));
        assert!(!out.contains("# LARGEST_FUNCTIONS"));
        assert!(!out.contains("# HIGH_COMPLEXITY"));
        assert!(!out.contains("# TOP_FILES"));
    }

    #[test]
    fn export_tsv_creates_file_with_all_sections() {
        let (result, root) = make_result_with_root();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.tsv");
        export_tsv(&result, &path, &root, true, true, Some(50)).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# SUMMARY"));
        assert!(contents.contains("# BREAKDOWN"));
        assert!(contents.contains("# FILES"));
        assert!(contents.contains("# FUNCTION_STATS"));
        assert!(contents.contains("# LARGEST_FUNCTIONS"));
        assert!(contents.contains("scan_dir\t/repo"));
        assert!(contents.contains("large_files_over_50"));
    }

    #[test]
    fn export_tsv_omits_function_analysis_when_disabled() {
        let (result, root) = make_result_with_root();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.tsv");
        export_tsv(&result, &path, &root, true, false, None).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("# FUNCTION_STATS"));
    }
}
