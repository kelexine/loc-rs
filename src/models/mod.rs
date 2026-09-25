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
            if d.chars().count() > 100 {
                let truncated: String = d.chars().take(100).collect();
                format!("{}...", truncated)
            } else {
                d.clone()
            }
        })
    }
}

/// A chunk of embedded source code in a container file (such as HTML `<script>` or Jupyter cell).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedChunk {
    /// Target extension for language categorization, e.g. "js", "css", "py", "md".
    pub extension: String,
    pub lines: usize,
    pub code: usize,
    pub comment: usize,
    pub blank: usize,
}

/// Aggregated information about a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub lines: usize,
    pub code: usize,
    pub comment: usize,
    pub blank: usize,
    pub is_binary: bool,
    /// True when this file is a recognised dependency lockfile.
    ///
    /// Lockfiles are shown in the tree with a `[lockfile]` tag but are
    /// excluded from all line-count totals and the extension breakdown.
    #[serde(default)]
    pub is_lockfile: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<DateTime<Utc>>,
    pub functions: Vec<FunctionInfo>,
    /// Embedded language breakdown for multi-language containers (HTML, Jupyter Notebooks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded: Vec<EmbeddedChunk>,
    /// Explicit or detected language / breakdown key (e.g. "sh", "py", "Makefile", "Kconfig").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl FileInfo {
    pub fn new(
        path: PathBuf,
        lines: usize,
        code: usize,
        comment: usize,
        blank: usize,
        is_binary: bool,
        last_modified: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            path,
            lines,
            code,
            comment,
            blank,
            is_binary,
            is_lockfile: false,
            last_modified,
            functions: Vec::new(),
            embedded: Vec::new(),
            language: None,
        }
    }

    /// Mark this file as a dependency lockfile.
    ///
    /// Lockfiles are displayed in the tree with a `[lockfile]` tag but
    /// contribute nothing to line-count statistics.
    pub fn mark_as_lockfile(mut self) -> Self {
        self.is_lockfile = true;
        self
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn with_functions(mut self, functions: Vec<FunctionInfo>) -> Self {
        self.functions = functions;
        self
    }

    pub fn with_embedded(mut self, embedded: Vec<EmbeddedChunk>) -> Self {
        self.embedded = embedded;
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

    /// Returns the resolved language/extension key for grouping in breakdown.
    pub fn language_key(&self) -> &str {
        if let Some(ref lang) = self.language {
            lang.as_str()
        } else if self.extension().is_empty() {
            "Unknown"
        } else {
            crate::language::canonical_language_name(self.extension())
        }
    }
}

/// Per-extension aggregated statistics.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ExtensionStats {
    pub lines: usize,
    pub code: usize,
    pub comment: usize,
    pub blank: usize,
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
            .filter(|f| !f.is_binary && !f.is_lockfile)
            .map(|f| f.lines)
            .sum()
    }

    pub fn total_code(&self) -> usize {
        self.files
            .iter()
            .filter(|f| !f.is_binary && !f.is_lockfile)
            .map(|f| f.code)
            .sum()
    }

    pub fn total_comment(&self) -> usize {
        self.files
            .iter()
            .filter(|f| !f.is_binary && !f.is_lockfile)
            .map(|f| f.comment)
            .sum()
    }

    pub fn total_blank(&self) -> usize {
        self.files
            .iter()
            .filter(|f| !f.is_binary && !f.is_lockfile)
            .map(|f| f.blank)
            .sum()
    }

    pub fn text_file_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| !f.is_binary && !f.is_lockfile)
            .count()
    }

    pub fn binary_file_count(&self) -> usize {
        self.files.iter().filter(|f| f.is_binary).count()
    }

    /// Number of recognised dependency lockfiles in the scan.
    pub fn lockfile_count(&self) -> usize {
        self.files.iter().filter(|f| f.is_lockfile).count()
    }

    pub fn total_functions(&self) -> usize {
        self.files.iter().map(|f| f.function_count()).sum()
    }

    pub fn total_classes(&self) -> usize {
        self.files.iter().map(|f| f.class_count()).sum()
    }
}
