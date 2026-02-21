// Author: kelexine (https://github.com/kelexine)
// export.rs — JSON, JSONL, and CSV export for scan results

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;

use crate::models::{FileInfo, ScanResult};

pub enum ExportFormat {
    Json,
    Jsonl,
    Csv,
}

impl ExportFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "json"  => Some(Self::Json),
            "jsonl" => Some(Self::Jsonl),
            "csv"   => Some(Self::Csv),
            _ => None,
        }
    }
}

/// Export the scan result in the specified format to `output_path`.
pub fn export(result: &ScanResult, output_path: &str, extract_functions: bool) -> Result<()> {
    let path = Path::new(output_path);

    match ExportFormat::from_path(path) {
        Some(ExportFormat::Json)  => export_json(result, path, extract_functions),
        Some(ExportFormat::Jsonl) => export_jsonl(result, path),
        Some(ExportFormat::Csv)   => export_csv(result, path, extract_functions),
        None => anyhow::bail!(
            "Unsupported export format '{}'. Use .json, .jsonl, or .csv",
            path.extension().and_then(|e| e.to_str()).unwrap_or("?")
        ),
    }
}

fn file_to_value(fi: &FileInfo, include_functions: bool) -> serde_json::Value {
    let mut obj = json!({
        "path": fi.path.to_string_lossy(),
        "lines": fi.lines,
        "is_binary": fi.is_binary,
        "extension": fi.extension(),
        "last_modified": fi.last_modified.map(|d| d.to_rfc3339()),
    });

    if include_functions {
        obj["function_count"] = json!(fi.function_count());
        obj["class_count"] = json!(fi.class_count());
        obj["avg_function_length"] = json!((fi.avg_function_length() * 100.0).round() / 100.0);
        obj["functions"] = json!(fi.functions.iter().map(|f| {
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
        }).collect::<Vec<_>>());
    }

    obj
}

fn export_json(result: &ScanResult, path: &Path, extract_functions: bool) -> Result<()> {
    let text_files: Vec<_> = result.files.iter().filter(|f| !f.is_binary).collect();
    let data = json!({
        "metadata": {
            "total_lines": result.total_lines(),
            "total_files": result.text_file_count(),
            "total_functions": result.total_functions(),
            "total_classes": result.total_classes(),
            "timestamp": Utc::now().to_rfc3339(),
            "function_extraction_enabled": extract_functions,
            "generator": "loc v5.0.0 by kelexine (https://github.com/kelexine)",
        },
        "breakdown": result.breakdown,
        "files": text_files.iter().map(|f| file_to_value(f, extract_functions)).collect::<Vec<_>>(),
    });

    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(f), &data)
        .with_context(|| "Failed to serialize JSON")?;

    eprintln!("[SUCCESS] Exported JSON → {}", path.display());
    Ok(())
}

fn export_jsonl(result: &ScanResult, path: &Path) -> Result<()> {
    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut writer = BufWriter::new(f);

    for fi in result.files.iter().filter(|f| !f.is_binary) {
        let line = serde_json::to_string(&file_to_value(fi, true))
            .with_context(|| "Failed to serialize JSONL record")?;
        writeln!(writer, "{}", line)?;
    }

    eprintln!("[SUCCESS] Exported JSONL → {}", path.display());
    Ok(())
}

fn export_csv(result: &ScanResult, path: &Path, include_functions: bool) -> Result<()> {
    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut wtr = csv::Writer::from_writer(BufWriter::new(f));

    // Header
    if include_functions {
        wtr.write_record(&["Path", "Lines", "Extension", "Functions", "Classes", "Avg Fn Length", "Last Modified"])?;
    } else {
        wtr.write_record(&["Path", "Lines", "Extension", "Last Modified"])?;
    }

    for fi in result.files.iter().filter(|f| !f.is_binary) {
        let last_mod = fi.last_modified
            .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default();

        if include_functions {
            wtr.write_record(&[
                fi.path.to_string_lossy().as_ref(),
                &fi.lines.to_string(),
                fi.extension(),
                &fi.function_count().to_string(),
                &fi.class_count().to_string(),
                &format!("{:.2}", fi.avg_function_length()),
                &last_mod,
            ])?;
        } else {
            wtr.write_record(&[
                fi.path.to_string_lossy().as_ref(),
                &fi.lines.to_string(),
                fi.extension(),
                &last_mod,
            ])?;
        }
    }

    wtr.flush()?;
    eprintln!("[SUCCESS] Exported CSV → {}", path.display());
    Ok(())
}
