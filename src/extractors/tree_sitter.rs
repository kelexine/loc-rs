// Author: kelexine (https://github.com/kelexine)
// extractors/tree_sitter.rs — Generic tree-sitter extraction logic + AST complexity engine

use super::Extractor;
use crate::models::FunctionInfo;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};
use std::collections::HashMap;

// ── AST-based cyclomatic complexity ──────────────────────────────────────────
//
// More accurate than the keyword heuristic because:
//   • Strings and comments are invisible to the AST — zero false positives.
//   • `elif` is a distinct node type, never confused with `if`.
//   • `while(`/`for(` and column-0 constructs are caught without spacing tricks.
//   • Match arms counted individually via `match_arm` nodes.
//
// Used by all tree-sitter-backed language extractors.  Languages without a
// grammar fall back to `estimate_complexity` in extractors/mod.rs.

/// Node types that constitute a decision point (each adds one independent path).
/// Covers every tree-sitter grammar bundled with loc-rs.
const DECISION_NODE_TYPES: &[&str] = &[
    // ── if / elif / elseif ────────────────────────────────────────────────
    "if_expression",              // Rust
    "if_statement",               // Python, JS, TS, Java, C, C++, Go, PHP, Swift
    "elif_clause",                // Python   ← never double-counted with if_statement
    "elseif_clause",              // PHP
    // ── loops ────────────────────────────────────────────────────────────
    "while_expression",           // Rust
    "while_statement",            // Python, JS, TS, Java, C, C++, PHP, Swift
    "do_statement",               // JS, TS, Java, C, C++, PHP
    "for_expression",             // Rust
    "for_statement",              // Python, JS, TS, Java, C, C++, PHP
    "for_in_statement",           // JS, TS
    "for_of_statement",           // JS, TS
    "foreach_statement",          // PHP
    "enhanced_for_statement",     // Java
    "loop_expression",            // Rust (infinite loop)
    // ── match / switch cases ─────────────────────────────────────────────
    "match_arm",                  // Rust — each arm = 1 decision point
    "case_statement",             // C, C++
    "switch_block_statement_group", // Java
    "expression_case_clause",     // Go
    "case_clause",                // Go, JS
    "when",                       // Ruby
    // ── exception handling ────────────────────────────────────────────────
    "catch_clause",               // JS, TS, Java, C++
    "except_clause",              // Python
    "rescue_clause",              // Ruby
    "rescue",                     // Ruby (alternate form)
    // ── ternary / conditional ─────────────────────────────────────────────
    "ternary_expression",         // JS, TS, PHP, Java, C, C++
    "conditional_expression",     // C, C++ (some grammars)
    // ── guard ────────────────────────────────────────────────────────────
    "guard_statement",            // Swift
    // ── logical operators as first-class node types (some grammars) ──────
    "boolean_operator",           // Python: `and` / `or`
    "logical_and",                // some grammars
    "logical_or",                 // some grammars
];

/// Binary-expression node types that contain `&&`/`||` as operator child tokens
/// rather than exposing them as a named node type.
const BINARY_EXPR_TYPES: &[&str] = &[
    "binary_expression",   // Rust, JS, TS, Java, C, C++, Go, PHP, Ruby
    "logical_expression",  // some TS grammars
];

/// Operator texts treated as logical decision points inside binary expressions.
const LOGICAL_OPS: &[&str] = &["&&", "||"];

/// Walk the AST rooted at `root` and compute cyclomatic complexity.
///
/// # Algorithm
/// M = 1 + Σ decision_points
///
/// Uses an iterative DFS (no recursion) to avoid stack overflow on deeply
/// nested code.  Only processes each node on first arrival.
pub fn ast_complexity(root: Node<'_>, src: &[u8]) -> u32 {
    let mut cc = 1u32;
    let mut cursor = root.walk();
    let mut visited = false;

    loop {
        if !visited {
            let node = cursor.node();
            let kind = node.kind();

            if DECISION_NODE_TYPES.contains(&kind) {
                cc += 1;
            } else if BINARY_EXPR_TYPES.contains(&kind) {
                // Check direct children for logical-operator tokens (&&, ||).
                // Using index access avoids borrowing `cursor` while iterating.
                let child_count = node.child_count();
                for i in 0..child_count {
                    if let Some(child) = node.child(i) {
                        let text = child.utf8_text(src).unwrap_or("");
                        if LOGICAL_OPS.contains(&text) {
                            cc += 1;
                        }
                    }
                }
            }
        }

        if !visited && cursor.goto_first_child() {
            visited = false;
        } else if cursor.goto_next_sibling() {
            visited = false;
        } else if cursor.goto_parent() {
            visited = true;
            // Stop once we've backtracked to the subtree root.
            if cursor.node().id() == root.id() {
                break;
            }
        } else {
            break;
        }
    }

    cc
}

/// Generic tree-sitter-backed extractor driven by a query string.
///
/// Used as the AST engine for all bundled language extractors once completed.
/// Query captures must include at minimum `@function` (or `@method`/`@class`)
/// and `@name`.
#[allow(dead_code)]
pub struct TreeSitterExtractor {
    language: Language,
    query: Query,
}

#[allow(dead_code)]
impl TreeSitterExtractor {
    /// Creates a new TreeSitterExtractor for a given language and query string.
    /// The query string should define captures like @function, @name, @class, @method.
    pub fn new(language: Language, query_source: &str) -> Result<Self, tree_sitter::QueryError> {
        let query = Query::new(&language, query_source)?;
        Ok(Self { language, query })
    }
}

impl Extractor for TreeSitterExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&self.language).is_err() {
            return vec![];
        }

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return vec![],
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.query, tree.root_node(), content.as_bytes());

        let mut functions = HashMap::new();

        let capture_names = self.query.capture_names();
        let mut capture_map = HashMap::new();
        for (i, name) in capture_names.iter().enumerate() {
            capture_map.insert(*name, i as u32);
        }

        while let Some(m) = matches.next() {
            let mut root_node = None;
            let mut name = String::new();
            let mut is_class = false;
            let mut is_method = false;

            for cap in m.captures {
                let capture_name = capture_names[cap.index as usize];
                let text = cap.node.utf8_text(content.as_bytes()).unwrap_or("").to_string();

                match capture_name {
                    "function" | "class" | "method" => {
                        root_node = Some(cap.node);
                        if capture_name == "class" {
                            is_class = true;
                        }
                        if capture_name == "method" {
                            is_method = true;
                        }
                    }
                    "name" => {
                        name = text;
                    }
                    _ => {}
                }
            }

            if let Some(node) = root_node
                && !name.is_empty()
            {
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let line_start = start_point.row + 1;
                let line_end = end_point.row + 1;
                
                // AST-based complexity — accurate because strings/comments
                // are invisible to the parser and node types are unambiguous.
                let complexity = if is_class { 1 } else { ast_complexity(node, content.as_bytes()) };

                let info = FunctionInfo {
                    name: name.clone(),
                    line_start,
                    line_end,
                    parameters: vec![], // TODO: Expand this logic as needed for params
                    is_async: false,    // TODO: Expand this logic for async
                    is_method,
                    is_class,
                    docstring: None,
                    decorators: vec![],
                    complexity,
                };

                // Use node id to avoid duplicates if multiple queries match the same node
                functions.insert(node.id(), info);
            }
        }

        let mut result: Vec<_> = functions.into_values().collect();
        result.sort_by_key(|f| f.line_start);
        result
    }
}
