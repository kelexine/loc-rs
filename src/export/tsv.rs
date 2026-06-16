// Author: kelexine (https://github.com/kelexine)
// export/tsv.rs — TSV file export for agent-mode results

use crate::models::ScanResult;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export scan results as TSV to `path`.
///
/// Layout mirrors the agent stdout format so a script consuming `-e out.tsv`
/// and one consuming `--format agent` stdout sees identical structure:
///
/// ```text
/// # SUMMARY
/// metric\tvalue
/// total_lines\t12345
/// ...
///
/// # BREAKDOWN
/// extension\tfiles\tlines\tcode\tcomment\tblank\tpct_lines
/// rs\t15\t8000\t6000\t1500\t500\t64.90%
/// ...
///
/// # FILES
/// path\tlines\tcode\tcomment\tblank\textension\tfunctions\tclasses\tlast_modified
/// src/main.rs\t97\t80\t5\t12\trs\t0\t0\t2025-01-01T00:00:00Z
/// ...
/// ```
pub fn export_tsv(result: &ScanResult, path: &Path, extract_functions: bool) -> Result<()> {
    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut w = BufWriter::new(f);

    write_summary_section(&mut w, result)?;
    writeln!(w)?;
    write_breakdown_section(&mut w, result)?;
    writeln!(w)?;
    write_files_section(&mut w, result, extract_functions)?;

    println!("[SUCCESS] Exported TSV → {}", path.display());
    Ok(())
}

pub fn write_summary_section<W: Write>(w: &mut W, result: &ScanResult) -> Result<()> {
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
    Ok(())
}

pub fn write_breakdown_section<W: Write>(w: &mut W, result: &ScanResult) -> Result<()> {
    writeln!(w, "# BREAKDOWN")?;
    writeln!(w, "extension\tfiles\tlines\tcode\tcomment\tblank\tpct_lines")?;

    let total_lines = result.total_lines();
    let mut entries: Vec<_> = result.breakdown.iter().collect();
    entries.sort_by(|a, b| b.1.lines.cmp(&a.1.lines));

    for (ext, stats) in &entries {
        let pct = if total_lines > 0 {
            format!("{:.2}%", stats.lines as f64 / total_lines as f64 * 100.0)
        } else {
            "0.00%".to_string()
        };
        writeln!(
            w,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            ext, stats.files, stats.lines, stats.code, stats.comment, stats.blank, pct
        )?;
    }
    Ok(())
}

pub fn write_files_section<W: Write>(
    w: &mut W,
    result: &ScanResult,
    include_functions: bool,
) -> Result<()> {
    writeln!(w, "# FILES")?;
    if include_functions {
        writeln!(
            w,
            "path\tlines\tcode\tcomment\tblank\textension\tfunctions\tclasses\tavg_fn_length\tlast_modified"
        )?;
    } else {
        writeln!(
            w,
            "path\tlines\tcode\tcomment\tblank\textension\tlast_modified"
        )?;
    }

    for fi in result.files.iter().filter(|f| !f.is_binary && !f.is_lockfile) {
        let modified = fi
            .last_modified
            .map(|d| d.to_rfc3339())
            .unwrap_or_default();

        if include_functions {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{}",
                fi.path.display(),
                fi.lines,
                fi.code,
                fi.comment,
                fi.blank,
                fi.extension(),
                fi.function_count(),
                fi.class_count(),
                fi.avg_function_length(),
                modified,
            )?;
        } else {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                fi.path.display(),
                fi.lines,
                fi.code,
                fi.comment,
                fi.blank,
                fi.extension(),
                modified,
            )?;
        }
    }
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ExtensionStats, FileInfo, ScanResult};
    use std::collections::HashMap;
    use std::path::PathBuf;

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
    fn summary_section_has_header_and_values() {
        let result = make_result();
        let mut buf = Vec::new();
        write_summary_section(&mut buf, &result).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.starts_with("# SUMMARY\n"));
        assert!(out.contains("metric\tvalue\n"));
        assert!(out.contains("total_lines\t100\n"));
        assert!(out.contains("total_code\t80\n"));
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
        write_breakdown_section(&mut buf, &result).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let rs_pos = out.find("rs\t").unwrap();
        let py_pos = out.find("py\t").unwrap();
        assert!(rs_pos < py_pos, "rs (200 lines) should precede py (50 lines)");
    }

    #[test]
    fn breakdown_pct_sums_to_100() {
        let result = make_result();
        let mut buf = Vec::new();
        write_breakdown_section(&mut buf, &result).unwrap();
        let out = String::from_utf8(buf).unwrap();
        // Single extension — must be 100.00%
        assert!(out.contains("100.00%"), "Single-ext pct should be 100.00%:\n{}", out);
    }

    #[test]
    fn files_section_no_functions_has_7_columns() {
        let result = make_result();
        let mut buf = Vec::new();
        write_files_section(&mut buf, &result, false).unwrap();
        let out = String::from_utf8(buf).unwrap();
        // Header line: path\tlines\tcode\tcomment\tblank\textension\tlast_modified
        let header = out.lines().nth(1).unwrap();
        assert_eq!(header.split('\t').count(), 7);
    }

    #[test]
    fn files_section_with_functions_has_10_columns() {
        let result = make_result();
        let mut buf = Vec::new();
        write_files_section(&mut buf, &result, true).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let header = out.lines().nth(1).unwrap();
        assert_eq!(header.split('\t').count(), 10);
    }

    #[test]
    fn export_tsv_creates_file() {
        let result = make_result();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.tsv");
        export_tsv(&result, &path, false).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# SUMMARY"));
        assert!(contents.contains("# BREAKDOWN"));
        assert!(contents.contains("# FILES"));
    }
}
