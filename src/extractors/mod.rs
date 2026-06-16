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

/// Keyword-based cyclomatic complexity heuristic (fallback).
///
/// Retained for languages that may be added in the future without a
/// tree-sitter grammar.  All current bundled extractors use the more
/// accurate [`tree_sitter::ast_complexity`] engine instead.
///
/// # Algorithm
/// M = 1 + P, where P is the number of lines containing a predicate keyword.
#[allow(dead_code)]
pub fn estimate_complexity(block: &[&str]) -> u32 {
    const KEYWORDS: &[&str] = &[
        // ── branching ──────────────────────────────────────────────────────
        // "else if" absent: "if " already fires on the `if` inside it.
        // "elif "  absent: same — "if " matches the `if` inside `elif x:`.
        "if ",
        // ── loops — space-style and paren-style ───────────────────────────
        // Leading-space guard removed so column-0 loops are caught.
        // Two patterns each avoid requiring a trailing space before `(`.
        "while ", "while(",
        "for ",   "for(",
        // ── match / switch ─────────────────────────────────────────────────
        " match ", "case ",
        // ── exception handling ─────────────────────────────────────────────
        " catch ", " except ",
        // ── logical operators ──────────────────────────────────────────────
        "&&", "||",
        // ── ternary / Rust ? propagation ───────────────────────────────────
        // Also fires on `?` in string/comment content — known heuristic limit.
        "? ",
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

    // ── Bug-fix regression tests ─────────────────────────────────────────────

    #[test]
    fn test_complexity_no_double_count_elif() {
        // Python `elif` must be exactly 1 decision point, not 2.
        // Before fix: "elif " → +1, "if " inside "elif" → +1 = wrong total of 3.
        // After fix:  "elif " absent; "if " matches once = correct total of 2.
        let block = vec!["elif x > 0:"];
        assert_eq!(estimate_complexity(&block), 2);
    }

    #[test]
    fn test_complexity_while_at_column_zero() {
        // Previously required leading space → missed unindented while.
        let block = vec!["while x != 0 {"];
        assert_eq!(estimate_complexity(&block), 2);
    }

    #[test]
    fn test_complexity_while_paren_style() {
        // C/Java/JS style: while( with no space between keyword and paren.
        let block = vec!["while(x > 0) {"];
        assert_eq!(estimate_complexity(&block), 2);
    }

    #[test]
    fn test_complexity_while_indented_paren_style() {
        // Indented C-style while( still counted exactly once.
        let block = vec!["    while(i < len) {"];
        assert_eq!(estimate_complexity(&block), 2);
    }

    #[test]
    fn test_complexity_for_at_column_zero() {
        // Previously required leading space → missed unindented for.
        let block = vec!["for i in range(10):"];
        assert_eq!(estimate_complexity(&block), 2);
    }

    #[test]
    fn test_complexity_for_c_style() {
        // C-style: for( with no space.
        let block = vec!["for(int i = 0; i < n; i++) {"];
        assert_eq!(estimate_complexity(&block), 2);
    }

    #[test]
    fn test_complexity_while_and_for_no_double_count() {
        // "while (" matches "while " but NOT "while(" — no double count.
        // "for (" matches "for " but NOT "for(" — no double count.
        let block = vec!["while (x) {", "for (int i = 0; i < n; ++i) {"];
        // base(1) + while(1) + for(1) = 3
        assert_eq!(estimate_complexity(&block), 3);
    }
}
