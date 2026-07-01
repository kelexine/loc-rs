// Author: kelexine (https://github.com/kelexine)
// export/mod.rs — Export dispatcher

pub mod csv;
pub mod html;
pub mod json;
pub mod tsv;

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
    /// TSV file — mirrors agent-mode stdout, section-delimited by `# HEADER` lines.
    Tsv,
    /// Standalone HTML dashboard report.
    Html,
}

impl ExportFormat {
    /// Resolve export format from a path extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "json"       => Some(Self::Json),
            "jsonl"      => Some(Self::Jsonl),
            "csv"        => Some(Self::Csv),
            "tsv"        => Some(Self::Tsv),
            "html" | "htm" => Some(Self::Html),
            _ => None,
        }
    }
}

/// Export scan results to the file indicated by `output_path`.
///
/// Format is inferred from file extension:
/// `.json`, `.jsonl`, `.csv`, `.tsv`, `.html` / `.htm`.
///
/// `root` is used by the TSV and HTML exporters to render file paths
/// relative to the scan root instead of absolute host paths. `func_analysis`
/// and `warn_size` control whether — and how — the function-analysis block
/// and large-file flagging are included in those two formats; JSON/JSONL/CSV
/// are unaffected (out of scope for this refinement pass).
pub fn export(
    result: &ScanResult,
    output_path: &str,
    root: &Path,
    extract_functions: bool,
    func_analysis: bool,
    warn_size: Option<usize>,
) -> Result<()> {
    let path = Path::new(output_path);

    match ExportFormat::from_path(path) {
        Some(ExportFormat::Json)  => json::export_json(result, path, extract_functions),
        Some(ExportFormat::Jsonl) => json::export_jsonl(result, path),
        Some(ExportFormat::Csv)   => csv::export_csv(result, path, extract_functions),
        Some(ExportFormat::Tsv)   => tsv::export_tsv(result, path, root, extract_functions, func_analysis, warn_size),
        Some(ExportFormat::Html)  => html::export_html(result, path, root, extract_functions, func_analysis, warn_size),
        None => anyhow::bail!(
            "Unsupported export format '{}'. Use .json, .jsonl, .csv, .tsv, or .html",
            path.extension().and_then(|e| e.to_str()).unwrap_or("?")
        ),
    }
}
