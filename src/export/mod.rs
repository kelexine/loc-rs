// Author: kelexine (https://github.com/kelexine)
// export/mod.rs — Export dispatcher

pub mod csv;
pub mod html;
pub mod json;

use crate::models::ScanResult;
use anyhow::Result;
use std::path::Path;

/// Supported export formats resolved from output filename extension.
pub enum ExportFormat {
    /// Pretty JSON document with metadata, breakdown, and file records.
    Json,
    /// One JSON object per line (file-level records).
    Jsonl,
    /// CSV file with file-level metrics.
    Csv,
    /// Standalone HTML dashboard report.
    Html,
}

impl ExportFormat {
    /// Resolve export format from a path extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "json" => Some(Self::Json),
            "jsonl" => Some(Self::Jsonl),
            "csv" => Some(Self::Csv),
            "html" | "htm" => Some(Self::Html),
            _ => None,
        }
    }
}

/// Export scan results to the file indicated by `output_path`.
///
/// The format is inferred from file extension (`.json`, `.jsonl`, `.csv`, `.html`/`.htm`).
pub fn export(result: &ScanResult, output_path: &str, extract_functions: bool) -> Result<()> {
    let path = Path::new(output_path);

    match ExportFormat::from_path(path) {
        Some(ExportFormat::Json) => json::export_json(result, path, extract_functions),
        Some(ExportFormat::Jsonl) => json::export_jsonl(result, path),
        Some(ExportFormat::Csv) => csv::export_csv(result, path, extract_functions),
        Some(ExportFormat::Html) => html::export_html(result, path, extract_functions),
        None => anyhow::bail!(
            "Unsupported export format '{}'. Use .json, .jsonl, .csv, or .html",
            path.extension().and_then(|e| e.to_str()).unwrap_or("?")
        ),
    }
}
