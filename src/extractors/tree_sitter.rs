// Author: kelexine (https://github.com/kelexine)
// extractors/tree_sitter.rs — Generic tree-sitter extraction logic + AST complexity engine

use super::Extractor;
use crate::models::FunctionInfo;
use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};

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
    "if_expression", // Rust
    "if_statement",  // Python, JS, TS, Java, C, C++, Go, PHP, Swift
    "elif_clause",   // Python   ← never double-counted with if_statement
    "elseif_clause", // PHP
    // ── loops ────────────────────────────────────────────────────────────
    "while_expression",       // Rust
    "while_statement",        // Python, JS, TS, Java, C, C++, PHP, Swift
    "do_statement",           // JS, TS, Java, C, C++, PHP
    "for_expression",         // Rust
    "for_statement",          // Python, JS, TS, Java, C, C++, PHP
    "for_in_statement",       // JS, TS
    "for_of_statement",       // JS, TS
    "foreach_statement",      // PHP
    "enhanced_for_statement", // Java
    "loop_expression",        // Rust (infinite loop)
    // ── match / switch cases ─────────────────────────────────────────────
    "match_arm",                    // Rust — each arm = 1 decision point
    "case_statement",               // C, C++
    "switch_block_statement_group", // Java
    "expression_case_clause",       // Go
    "case_clause",                  // Go, JS
    "when",                         // Ruby
    // ── exception handling ────────────────────────────────────────────────
    "catch_clause",  // JS, TS, Java, C++
    "except_clause", // Python
    "rescue_clause", // Ruby
    "rescue",        // Ruby (alternate form)
    // ── ternary / conditional ─────────────────────────────────────────────
    "ternary_expression",     // JS, TS, PHP, Java, C, C++
    "conditional_expression", // C, C++ (some grammars)
    // ── guard ────────────────────────────────────────────────────────────
    "guard_statement", // Swift
    // ── logical operators as first-class node types (some grammars) ──────
    "boolean_operator", // Python: `and` / `or`
    "logical_and",      // some grammars
    "logical_or",       // some grammars
];

/// Binary-expression node types that contain `&&`/`||` as operator child tokens
/// rather than exposing them as a named node type.
const BINARY_EXPR_TYPES: &[&str] = &[
    "binary_expression",  // Rust, JS, TS, Java, C, C++, Go, PHP, Ruby
    "logical_expression", // some TS grammars
];

/// Operator texts treated as logical decision points inside binary expressions.
const LOGICAL_OPS: &[&str] = &["&&", "||"];

/// Count decision points contributed directly by `node` (excluding its descendants).
#[inline]
fn node_decision_count(node: Node<'_>, src: &[u8]) -> u32 {
    let kind = node.kind();
    if DECISION_NODE_TYPES.contains(&kind) {
        1
    } else if BINARY_EXPR_TYPES.contains(&kind) {
        let mut count = 0;
        let child_count = node.child_count();
        for i in 0..child_count {
            if let Some(child) = node.child(i) {
                let text = child.utf8_text(src).unwrap_or("");
                if LOGICAL_OPS.contains(&text) {
                    count += 1;
                }
            }
        }
        count
    } else {
        0
    }
}

/// Precomputed cyclomatic complexity index for an entire AST.
///
/// Built in a single iterative post-order traversal ($O(N)$ total node visits).
/// Replaces quadratic $O(N \times \text{depth})$ re-walks when functions are nested.
#[derive(Debug, Clone, Default)]
pub struct ComplexityIndex {
    complexities: HashMap<usize, u32>,
}

impl ComplexityIndex {
    /// Build a complexity index for the entire subtree rooted at `root`.
    pub fn build(root: Node<'_>, src: &[u8]) -> Self {
        let mut complexities = HashMap::new();
        let mut cursor = root.walk();
        let mut stack: Vec<(usize, u32)> = Vec::new();

        stack.push((cursor.node().id(), node_decision_count(cursor.node(), src)));

        loop {
            // Descend to the leftmost leaf
            while cursor.goto_first_child() {
                let node = cursor.node();
                stack.push((node.id(), node_decision_count(node, src)));
            }

            // Process current leaf / node
            let (id, decisions) = stack.pop().expect("stack underflow");
            complexities.insert(id, 1 + decisions);
            if let Some(parent) = stack.last_mut() {
                parent.1 += decisions;
            }

            // Try moving to next sibling
            if cursor.goto_next_sibling() {
                let node = cursor.node();
                stack.push((node.id(), node_decision_count(node, src)));
                continue;
            }

            // Ascend until a sibling is found or root is reached
            let mut reached_root = false;
            while cursor.goto_parent() {
                let (id, decisions) = stack.pop().expect("stack underflow");
                complexities.insert(id, 1 + decisions);
                if let Some(parent) = stack.last_mut() {
                    parent.1 += decisions;
                }

                if cursor.node().id() == root.id() {
                    reached_root = true;
                    break;
                }

                if cursor.goto_next_sibling() {
                    let node = cursor.node();
                    stack.push((node.id(), node_decision_count(node, src)));
                    break;
                }
            }

            if reached_root || stack.is_empty() {
                break;
            }
        }

        Self { complexities }
    }

    /// Retrieve the precomputed cyclomatic complexity of `node` in $O(1)$ time.
    #[inline]
    #[must_use]
    pub fn get(&self, node: Node<'_>) -> u32 {
        self.complexities.get(&node.id()).copied().unwrap_or(1)
    }
}

/// Walk the AST rooted at `root` and compute cyclomatic complexity.
///
/// # Algorithm
/// M = 1 + Σ decision_points
#[allow(dead_code)]
#[must_use]
pub fn ast_complexity(root: Node<'_>, src: &[u8]) -> u32 {
    ComplexityIndex::build(root, src).get(root)
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

        let complexity_index = ComplexityIndex::build(tree.root_node(), content.as_bytes());
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
                let text = cap
                    .node
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();

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
                let complexity = if is_class {
                    1
                } else {
                    complexity_index.get(node)
                };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_index_matches_ast_complexity() {
        let code = r#"
fn complex_fn(a: i32, b: bool) -> i32 {
    if a > 0 && b {
        for i in 0..a {
            if i % 2 == 0 || !b {
                println!("{}", i);
            }
        }
    } else if a < -10 {
        match a {
            -11 => return 1,
            -12 => return 2,
            _ => return 3,
        }
    }
    0
}
"#;
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();
        let src = code.as_bytes();

        let index = ComplexityIndex::build(root, src);
        assert_eq!(index.get(root), ast_complexity(root, src));

        // Find the function node specifically
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "function_item" {
                assert_eq!(index.get(child), ast_complexity(child, src));
                assert_eq!(index.get(child), 10);
            }
        }
    }
}
