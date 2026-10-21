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
    "if_expression",   // Rust
    "if_statement",    // Python, JS, TS, Java, C, C++, Go, PHP, Swift
    "elif_clause",     // Python   ← never double-counted with if_statement
    "elseif_clause",   // PHP
    "if",              // Ruby
    "elsif",           // Ruby
    "unless",          // Ruby
    "if_modifier",     // Ruby (`x if y`)
    "unless_modifier", // Ruby (`x unless y`)
    // ── loops ────────────────────────────────────────────────────────────
    "while_expression",       // Rust, Scala
    "while_statement",        // Python, JS, TS, Java, C, C++, PHP, Swift, Kotlin, C#
    "do_statement",           // JS, TS, Java, C, C++, PHP, C# (special-cased: needs `while`)
    "do_while_statement",     // Kotlin
    "do_while_expression",    // Scala
    "repeat_while_statement", // Swift
    "for_expression",         // Rust, Scala
    "for_statement",          // Python, JS, TS, Java, C, C++, PHP, Kotlin, C#
    "for_range_loop",         // C++ (`for (auto x : xs)`)
    "for_in_statement",       // JS, TS
    "for_of_statement",       // JS, TS
    "foreach_statement",      // PHP, C#
    "enhanced_for_statement", // Java
    "loop_expression",        // Rust (infinite loop)
    "while",                  // Ruby
    "until",                  // Ruby
    "for",                    // Ruby
    "while_modifier",         // Ruby
    "until_modifier",         // Ruby
    // ── match / switch cases ─────────────────────────────────────────────
    "match_arm",                    // Rust — each arm = 1 decision point
    "case_statement",               // C, C++
    "switch_block_statement_group", // Java
    "switch_section",               // C#
    "switch_expression_arm",        // C#
    "expression_case_clause",       // Go
    "case_clause",                  // Go, Python `match`, Scala
    "switch_case",                  // JS, TS
    "switch_entry",                 // Swift
    "when_entry",                   // Kotlin
    "when",                         // Ruby
    // ── exception handling ────────────────────────────────────────────────
    "catch_clause",    // JS, TS, Java, C++, C#, Scala
    "catch_block",     // Swift, Kotlin
    "except_clause",   // Python
    "rescue_clause",   // Ruby
    "rescue",          // Ruby (alternate form)
    "rescue_modifier", // Ruby (`x rescue y`)
    // ── ternary / conditional ─────────────────────────────────────────────
    "ternary_expression",     // JS, TS, PHP, Java, C, C++
    "conditional_expression", // C, C++ (some grammars)
    "conditional",            // Ruby ternary
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
    "binary_expression",  // Rust, JS, TS, Java, C, C++, Go, PHP, Swift, Kotlin, C#
    "infix_expression",   // Scala
    "binary",             // Ruby
    "logical_expression", // some TS grammars
];

/// Operator texts treated as logical decision points inside binary expressions.
const LOGICAL_OPS: &[&str] = &["&&", "||", "and", "or"];

/// Count decision points contributed directly by `node` (excluding its descendants).
///
/// Only *named* nodes are counted as decision constructs: anonymous keyword
/// tokens share their text as `kind()` (e.g. the `if` token inside Ruby's
/// `if` node), so counting them would double-count every construct.
#[inline]
fn node_decision_count(node: Node<'_>, src: &[u8]) -> u32 {
    if !node.is_named() {
        return 0;
    }
    let kind = node.kind();
    // `do_statement` is a do-while loop in C-likes but a plain scope block in Swift
    // (`do { … } catch …`); only the loop form has a `while` token child.
    if kind == "do_statement" {
        let mut cursor = node.walk();
        let has_while = node.children(&mut cursor).any(|c| c.kind() == "while");
        return u32::from(has_while);
    }
    if DECISION_NODE_TYPES.contains(&kind) {
        1
    } else if BINARY_EXPR_TYPES.contains(&kind) {
        let mut count = 0;
        let child_count = node.child_count();
        for i in 0..child_count {
            if let Some(child) = node.child(i)
                && (!child.is_named() || child.kind() == "operator_identifier")
            {
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

    /// Parse `code` with `language` and return the whole-tree cyclomatic complexity.
    fn root_complexity(language: Language, code: &str) -> u32 {
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(code, None).unwrap();
        ast_complexity(tree.root_node(), code.as_bytes())
    }

    #[test]
    fn ruby_control_flow_is_counted_once_each() {
        let code = "def f(a)\n  if a > 1 && a < 5\n    1\n  elsif a == 0\n    2\n  end\n  \
                    while a > 0 do a -= 1 end\n  puts 'x' unless a\nend\n";
        // 1 + if + && + elsif + while + unless modifier
        assert_eq!(root_complexity(tree_sitter_ruby::LANGUAGE.into(), code), 6);
    }

    #[test]
    fn ruby_when_and_rescue_not_double_counted() {
        let code = "case x\nwhen 1 then 1\nwhen 2 then 2\nend\nbegin\n  y\nrescue\n  z\nend\n";
        // 1 + when + when + rescue
        assert_eq!(root_complexity(tree_sitter_ruby::LANGUAGE.into(), code), 4);
    }

    #[test]
    fn javascript_switch_cases_are_counted() {
        let code = "function f(x){ switch (x) { case 1: break; case 2: break; default: break; } }";
        assert_eq!(
            root_complexity(tree_sitter_javascript::LANGUAGE.into(), code),
            3
        );
    }

    #[test]
    fn cpp_range_for_is_counted() {
        let code = "void f(){ for (auto x : xs) { g(x); } }";
        assert_eq!(root_complexity(tree_sitter_cpp::LANGUAGE.into(), code), 2);
    }

    #[test]
    fn c_do_while_counts_but_swift_do_block_does_not() {
        let code = "void f(){ do { x(); } while (y); }";
        // 1 + do-while
        assert_eq!(root_complexity(tree_sitter_c::LANGUAGE.into(), code), 2);
    }

    #[test]
    fn swift_switch_entries_and_catch_are_counted() {
        let code = "func f() {\n  switch x {\n  case 1: break\n  default: break\n  }\n  \
                    do { try g() } catch { }\n}\n";
        // 1 + two switch entries + catch block
        assert_eq!(root_complexity(tree_sitter_swift::LANGUAGE.into(), code), 4);
    }

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
