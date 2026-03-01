// Author: kelexine (https://github.com/kelexine)
// extractors/mod.rs — Trait definition and shared utilities for extractors

pub mod cpp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod nim;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod tree_sitter;

use crate::models::FunctionInfo;
use std::path::Path;

pub trait Extractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo>;
}

pub fn get_extractor(path: &Path) -> Option<Box<dyn Extractor>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    match ext.as_str() {
        ".rs" => Some(Box::new(rust::RustExtractor)),
        ".py" | ".pyw" | ".pyi" => Some(Box::new(python::PythonExtractor)),
        ".js" | ".mjs" | ".cjs" | ".jsx" => {
            Some(Box::new(javascript::JavascriptExtractor::new(tree_sitter_javascript::LANGUAGE.into())))
        }
        ".ts" | ".mts" => {
            Some(Box::new(javascript::JavascriptExtractor::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())))
        }
        ".tsx" => {
            Some(Box::new(javascript::JavascriptExtractor::new(tree_sitter_typescript::LANGUAGE_TSX.into())))
        }
        ".go" => Some(Box::new(go::GoExtractor)),
        ".c" | ".h" | ".cpp" | ".cc" | ".cxx" | ".hpp" | ".hxx" => {
            Some(Box::new(cpp::CppExtractor))
        }
        ".java" | ".kt" | ".kts" | ".cs" | ".scala" => Some(Box::new(java::JavaExtractor)),
        ".php" | ".php3" | ".php4" | ".php5" | ".phtml" => Some(Box::new(php::PhpExtractor)),
        ".swift" => Some(Box::new(swift::SwiftExtractor)),
        ".rb" | ".rake" | ".gemspec" => Some(Box::new(ruby::RubyExtractor)),
        ".nim" | ".nims" => Some(Box::new(nim::NimExtractor)),
        _ => None,
    }
}

pub fn estimate_complexity(block: &[&str]) -> u32 {
    const KEYWORDS: &[&str] = &[
        "if ", "else if", "elif ", " while ", " for ", " match ", "case ", " catch ", " except ",
        "&&", "||", "? ",
    ];
    let mut cc = 1u32;
    for line in block {
        for kw in KEYWORDS {
            cc += line.matches(kw).count() as u32;
        }
    }
    cc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_estimation() {
        let block = vec![
            "if x > 0 {",
            "  for i in 0..10 {",
            "    if true && false { }",
            "  }",
            "} else if y {",
            "}",
        ];
        // 1 (base) + 1 (if) + 1 (for) + 1 (if) + 1 (&&) + 2 (else if & if ) = 7
        assert_eq!(estimate_complexity(&block), 7);
    }
}
