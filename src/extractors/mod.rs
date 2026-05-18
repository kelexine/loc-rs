// Author: kelexine (https://github.com/kelexine)
// extractors/mod.rs — Trait definition, shared utilities, and thread-local tree-sitter parser

pub mod cpp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod tree_sitter;

use crate::models::FunctionInfo;
use std::cell::RefCell;
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
        ".js" | ".mjs" | ".cjs" | ".jsx" => Some(Box::new(javascript::JavascriptExtractor::new(
            tree_sitter_javascript::LANGUAGE.into(),
        ))),
        ".ts" | ".mts" => Some(Box::new(javascript::JavascriptExtractor::new(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        ))),
        ".tsx" => Some(Box::new(javascript::JavascriptExtractor::new(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
        ))),
        ".go" => Some(Box::new(go::GoExtractor)),
        ".c" | ".h" | ".cpp" | ".cc" | ".cxx" | ".hpp" | ".hxx" => {
            Some(Box::new(cpp::CppExtractor))
        }
        ".java" | ".kt" | ".kts" | ".cs" | ".scala" => Some(Box::new(java::JavaExtractor)),
        ".php" | ".php3" | ".php4" | ".php5" | ".phtml" => Some(Box::new(php::PhpExtractor)),
        ".swift" => Some(Box::new(swift::SwiftExtractor)),
        ".rb" | ".rake" | ".gemspec" => Some(Box::new(ruby::RubyExtractor)),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Thread-local tree-sitter Parser
//
// Creating a fresh Parser for every file is wasteful; parsers are reusable
// within a thread once the language is set.  The thread-local keeps each
// Rayon worker thread's parser independent with no locking.
// ─────────────────────────────────────────────────────────────────────────────

thread_local! {
    static TS_PARSER: RefCell<::tree_sitter::Parser> =
        RefCell::new(::tree_sitter::Parser::new());
}

/// Set `language` on the thread-local parser, parse `content`, and invoke `f`
/// on the resulting tree.  Returns `None` if language loading or parsing fails.
pub fn with_parsed_tree<F, R>(
    language: ::tree_sitter::Language,
    content: &str,
    f: F,
) -> Option<R>
where
    F: FnOnce(::tree_sitter::Tree) -> R,
{
    TS_PARSER.with(|cell| {
        let mut parser = cell.borrow_mut();
        if parser.set_language(&language).is_err() {
            return None;
        }
        parser.parse(content, None).map(f)
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Cyclomatic-complexity heuristic (shared by all extractors)
// ─────────────────────────────────────────────────────────────────────────────

pub fn estimate_complexity(block: &[&str]) -> u32 {
    // "else if" is intentionally absent: "if " already matches the decision
    // inside "else if …", so listing both would double-count every else-if.
    const KEYWORDS: &[&str] = &[
        "if ", "elif ", " while ", " for ", " match ", "case ", " catch ", " except ",
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
    fn test_complexity_base() {
        let block = vec!["fn hello() {", "}"];
        assert_eq!(estimate_complexity(&block), 1);
    }

    #[test]
    fn test_complexity_branches() {
        let block = vec![
            "if x > 0 {",
            "  for i in 0..10 {",
            "    if true && false { }",
            "  }",
            "} else if y {",
            "}",
        ];
        // base(1) + if(1) + for(1) + if(1) + &&(1) + else_if's "if "(1) = 6
        assert_eq!(estimate_complexity(&block), 6);
    }

    #[test]
    fn test_complexity_no_double_count_else_if() {
        // "else if" must be exactly 1 decision point, not 2.
        let block = vec!["} else if condition {"];
        // base(1) + "if " match(1) = 2
        assert_eq!(estimate_complexity(&block), 2);
    }
}
