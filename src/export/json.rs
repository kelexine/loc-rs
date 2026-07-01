// Author: kelexine (https://github.com/kelexine)
// export/json.rs — JSON and JSONL export logic

use crate::models::{FileInfo, ScanResult};
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Build the canonical scan-report JSON value shared by every consumer of
/// JSON-shaped output (stdout `--json`, `.json` file export, and the data
/// blob injected into the HTML dashboard).
///
/// `root` controls path representation:
/// - `None` — paths are emitted as-is (absolute, matching historical
///   behaviour for stdout / `.json` file export — kept for backward
///   compatibility with existing consumers/scripts).
/// - `Some(r)` — paths are stripped to be relative to `r` (used by the
///   HTML exporter, where absolute host paths are meaningless once the
///   report is opened on another machine or viewed in a browser).
///
/// This is the single source of truth for the report shape — both
/// [`print_json_stats`] and [`export_json`] delegate here so the two
/// surfaces can never drift out of schema sync.
pub fn build_scan_json(
    result: &ScanResult,
    extract_functions: bool,
    root: Option<&Path>,
) -> serde_json::Value {
    let text_files: Vec<_> = result
        .files
        .iter()
        .filter(|f| !f.is_binary && !f.is_lockfile)
        .collect();

    json!({
        "metadata": {
            "total_lines":    result.total_lines(),
            "total_code":     result.total_code(),
            "total_comment":  result.total_comment(),
            "total_blank":    result.total_blank(),
            "total_files":    result.text_file_count(),
            "binary_files":   result.binary_file_count(),
            "lockfiles":      result.lockfile_count(),
            "total_functions": result.total_functions(),
            "total_classes":  result.total_classes(),
            "timestamp":      Utc::now().to_rfc3339(),
            "function_extraction_enabled": extract_functions,
            "generator": concat!("loc v", env!("CARGO_PKG_VERSION"), " by kelexine (https://github.com/kelexine)"),
        },
        "breakdown": result.breakdown,
        "files": text_files.iter().map(|f| file_to_value(f, extract_functions, root)).collect::<Vec<_>>(),
    })
}

/// Print a compact JSON summary of the scan to stdout.
///
/// The output shape mirrors the file-export format so scripts can consume
/// either source interchangeably.  The `--json` flag routes here instead of
/// the coloured terminal display.
pub fn print_json_stats(result: &ScanResult, extract_functions: bool) -> Result<()> {
    let data = build_scan_json(result, extract_functions, None);

    let stdout = std::io::stdout();
    serde_json::to_writer(stdout.lock(), &data)
        .with_context(|| "Failed to serialize JSON stats to stdout")?;
    // Trailing newline for clean shell output
    println!();
    Ok(())
}

pub fn export_json(result: &ScanResult, path: &Path, extract_functions: bool) -> Result<()> {
    // Absolute paths — preserved for backward compatibility with existing
    // consumers of `-e out.json`. Use the HTML export if relative,
    // portable paths are what you need.
    let data = build_scan_json(result, extract_functions, None);

    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(f), &data)
        .with_context(|| "Failed to serialize JSON")?;

    println!("[SUCCESS] Exported JSON → {}", path.display());
    Ok(())
}

pub fn export_jsonl(result: &ScanResult, path: &Path) -> Result<()> {
    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut writer = BufWriter::new(f);

    for fi in result.files.iter().filter(|f| !f.is_binary && !f.is_lockfile) {
        let line = serde_json::to_string(&file_to_value(fi, true, None))
            .with_context(|| "Failed to serialize JSONL record")?;
        writeln!(writer, "{}", line)?;
    }

    println!("[SUCCESS] Exported JSONL → {}", path.display());
    Ok(())
}

/// Serialize a single [`FileInfo`] to a JSON value.
///
/// `root`, when provided, strips the given prefix from `fi.path` so the
/// emitted `path` field is relative and portable. Falls back to the
/// original path unchanged if the strip fails (e.g. `fi.path` isn't
/// actually under `root`), matching the same fallback policy used by the
/// TSV exporter.
pub fn file_to_value(
    fi: &FileInfo,
    include_functions: bool,
    root: Option<&Path>,
) -> serde_json::Value {
    let path_str = match root {
        Some(r) => fi
            .path
            .strip_prefix(r)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| fi.path.to_string_lossy().into_owned()),
        None => fi.path.to_string_lossy().into_owned(),
    };

    let mut obj = json!({
        "path": path_str,
        "lines": fi.lines,
        "code": fi.code,
        "comment": fi.comment,
        "blank": fi.blank,
        "is_binary": fi.is_binary,
        "is_lockfile": fi.is_lockfile,
        "extension": fi.extension(),
        "last_modified": fi.last_modified.map(|d| d.to_rfc3339()),
    });

    if include_functions {
        obj["function_count"] = json!(fi.function_count());
        obj["class_count"] = json!(fi.class_count());
        obj["avg_function_length"] = json!((fi.avg_function_length() * 100.0).round() / 100.0);
        obj["functions"] = json!(
            fi.functions
                .iter()
                .map(|f| {
                    json!({
                        "name": f.name,
                        "line_start": f.line_start,
                        "line_end": f.line_end,
                        "line_count": f.line_count(),
                        "parameters": f.parameters,
                        "is_async": f.is_async,
                        "is_method": f.is_method,
                        "is_class": f.is_class,
                        "docstring": f.truncated_docstring(),
                        "decorators": f.decorators,
                        "complexity": f.complexity,
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    obj
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ExtensionStats, FileInfo};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_file(path: &str) -> FileInfo {
        FileInfo::new(PathBuf::from(path), 100, 80, 10, 10, false, None)
    }

    fn make_result() -> ScanResult {
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "rs".to_string(),
            ExtensionStats { lines: 100, code: 80, comment: 10, blank: 10, files: 1, functions: 0 },
        );
        ScanResult {
            files: vec![make_file("/repo/src/main.rs")],
            breakdown,
        }
    }

    #[test]
    fn file_to_value_absolute_when_no_root() {
        let fi = make_file("/repo/src/main.rs");
        let v = file_to_value(&fi, false, None);
        assert_eq!(v["path"], "/repo/src/main.rs");
    }

    #[test]
    fn file_to_value_relative_when_root_given() {
        let fi = make_file("/repo/src/main.rs");
        let v = file_to_value(&fi, false, Some(Path::new("/repo")));
        assert_eq!(v["path"], "src/main.rs");
    }

    #[test]
    fn file_to_value_falls_back_when_not_under_root() {
        let fi = make_file("/repo/src/main.rs");
        let v = file_to_value(&fi, false, Some(Path::new("/other")));
        assert_eq!(v["path"], "/repo/src/main.rs");
    }

    #[test]
    fn build_scan_json_matches_between_absolute_and_relative_roots_shape() {
        let result = make_result();
        let abs = build_scan_json(&result, false, None);
        let rel = build_scan_json(&result, false, Some(Path::new("/repo")));
        // Same schema (keys), different path representation.
        assert_eq!(abs["metadata"]["total_lines"], rel["metadata"]["total_lines"]);
        assert_eq!(abs["files"][0]["path"], "/repo/src/main.rs");
        assert_eq!(rel["files"][0]["path"], "src/main.rs");
    }

    #[test]
    fn print_json_stats_and_export_json_produce_same_shape() {
        // Both call sites (print_json_stats / export_json) delegate to the
        // same builder, so their output can only ever differ in the
        // timestamp field (each call stamps Utc::now() independently).
        // Compare everything else for exact equality.
        let result = make_result();
        let mut a = build_scan_json(&result, true, None);
        let mut b = build_scan_json(&result, true, None);
        a["metadata"]["timestamp"] = json!(null);
        b["metadata"]["timestamp"] = json!(null);
        assert_eq!(a, b, "builder must be deterministic for identical inputs (aside from timestamp)");
    }

    #[test]
    fn export_json_creates_file() {
        let result = make_result();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.json");
        export_json(&result, &path, false).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\"total_lines\""));
    }

    #[test]
    fn export_jsonl_creates_one_line_per_file() {
        let result = make_result();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.jsonl");
        export_jsonl(&result, &path).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.lines().count(), 1);
    }
}
