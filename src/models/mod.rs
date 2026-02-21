// Author: kelexine (https://github.com/kelexine)
// models.rs — Core data structures for the LOC counter

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Information about a single function, method, or class extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub line_start: usize,
    pub line_end: usize,
    pub parameters: Vec<String>,
    pub is_async: bool,
    pub is_method: bool,
    pub is_class: bool,
    pub docstring: Option<String>,
    pub decorators: Vec<String>,
    /// Cyclomatic complexity (simplified branch-count heuristic)
    pub complexity: u32,
}

impl FunctionInfo {
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_end.saturating_sub(self.line_start) + 1
    }

    /// Truncate docstring to 100 chars for export compactness.
    pub fn truncated_docstring(&self) -> Option<String> {
        self.docstring.as_ref().map(|d| {
            if d.len() > 100 {
                format!("{}...", &d[..100])
            } else {
                d.clone()
            }
        })
    }
}

/// Aggregated information about a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub lines: usize,
    pub is_binary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<DateTime<Utc>>,
    pub functions: Vec<FunctionInfo>,
}

impl FileInfo {
    pub fn new(
        path: PathBuf,
        lines: usize,
        is_binary: bool,
        last_modified: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            path,
            lines,
            is_binary,
            last_modified,
            functions: Vec::new(),
        }
    }

    pub fn with_functions(mut self, functions: Vec<FunctionInfo>) -> Self {
        self.functions = functions;
        self
    }

    #[inline]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    #[inline]
    pub fn class_count(&self) -> usize {
        self.functions.iter().filter(|f| f.is_class).count()
    }

    pub fn avg_function_length(&self) -> f64 {
        let non_class: Vec<_> = self.functions.iter().filter(|f| !f.is_class).collect();
        if non_class.is_empty() {
            return 0.0;
        }
        let total: usize = non_class.iter().map(|f| f.line_count()).sum();
        total as f64 / non_class.len() as f64
    }

    /// File extension without the leading dot, or empty string.
    pub fn extension(&self) -> &str {
        self.path.extension().and_then(|e| e.to_str()).unwrap_or("")
    }
}

/// Per-extension aggregated statistics.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ExtensionStats {
    pub lines: usize,
    pub files: usize,
    pub functions: usize,
}

/// Breakdown map: extension → stats.
pub type Breakdown = HashMap<String, ExtensionStats>;

/// The full scan result returned from the counter.
#[derive(Debug)]
pub struct ScanResult {
    pub files: Vec<FileInfo>,
    pub breakdown: Breakdown,
}

impl ScanResult {
    pub fn total_lines(&self) -> usize {
        self.files
            .iter()
            .filter(|f| !f.is_binary)
            .map(|f| f.lines)
            .sum()
    }

    pub fn text_file_count(&self) -> usize {
        self.files.iter().filter(|f| !f.is_binary).count()
    }

    pub fn binary_file_count(&self) -> usize {
        self.files.iter().filter(|f| f.is_binary).count()
    }

    pub fn total_functions(&self) -> usize {
        self.files.iter().map(|f| f.function_count()).sum()
    }

    pub fn total_classes(&self) -> usize {
        self.files.iter().map(|f| f.class_count()).sum()
    }
}
