// Author: kelexine (https://github.com/kelexine)
// export/json.rs — JSON and JSONL export logic

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use crate::models::{FileInfo, ScanResult};

pub fn export_json(result: &ScanResult, path: &Path, extract_functions: bool) -> Result<()> {
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

pub fn export_jsonl(result: &ScanResult, path: &Path) -> Result<()> {
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

pub fn file_to_value(fi: &FileInfo, include_functions: bool) -> serde_json::Value {
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
