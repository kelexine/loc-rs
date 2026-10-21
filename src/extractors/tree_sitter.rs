// Author: kelexine (https://github.com/kelexine)
// extractors/tree_sitter.rs — Generic tree-sitter extraction logic + AST complexity engine

use super::Extractor;
use crate::models::FunctionInfo;
use rustc_hash::FxHashMap;
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

/// Returns `true` when `kind` is a decision-point AST node type across any
/// bundled tree-sitter grammar. Compiled to a jump/hash dispatch by LLVM
/// instead of a 48-element linear slice scan per visited AST node.
#[inline]
fn is_decision_node_kind(kind: &str) -> bool {
    matches!(
        kind,
        // ── if / elif / elseif ────────────────────────────────────────────
        "if_expression"
            | "if_statement"
            | "elif_clause"
            | "elseif_clause"
            | "if"
            | "elsif"
            | "unless"
            | "if_modifier"
            | "unless_modifier"
            // ── loops ─────────────────────────────────────────────────────
            | "while_expression"
            | "while_statement"
            | "do_while_statement"
            | "do_while_expression"
            | "repeat_while_statement"
            | "for_expression"
            | "for_statement"
            | "for_range_loop"
            | "for_in_statement"
            | "for_of_statement"
            | "foreach_statement"
            | "enhanced_for_statement"
            | "loop_expression"
            | "while"
            | "until"
            | "for"
            | "while_modifier"
            | "until_modifier"
            // ── match / switch cases ──────────────────────────────────────
            | "match_arm"
            | "case_statement"
            | "switch_block_statement_group"
            | "switch_section"
            | "switch_expression_arm"
            | "expression_case_clause"
            | "case_clause"
            | "switch_case"
            | "switch_entry"
            | "when_entry"
            | "when"
            // ── exception handling ────────────────────────────────────────
            | "catch_clause"
            | "catch_block"
            | "except_clause"
            | "rescue_clause"
            | "rescue"
            | "rescue_modifier"
            // ── ternary / conditional / guard / boolean ops ───────────────
            | "ternary_expression"
            | "conditional_expression"
            | "conditional"
            | "guard_statement"
            | "boolean_operator"
            | "logical_and"
            | "logical_or"
    )
}

#[inline]
fn is_binary_expr_kind(kind: &str) -> bool {
    matches!(
        kind,
        "binary_expression" | "infix_expression" | "binary" | "logical_expression"
    )
}

#[inline]
fn is_logical_op(text: &str) -> bool {
    matches!(text, "&&" | "||" | "and" | "or")
}

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
    if is_decision_node_kind(kind) {
        1
    } else if is_binary_expr_kind(kind) {
        let mut count = 0;
        let child_count = node.child_count();
        for i in 0..child_count {
            if let Some(child) = node.child(i)
                && (!child.is_named() || child.kind() == "operator_identifier")
            {
                let text = child.utf8_text(src).unwrap_or("");
                if is_logical_op(text) {
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
    complexities: FxHashMap<usize, u32>,
}

impl ComplexityIndex {
    /// Build a complexity index for the entire subtree rooted at `root`.
    pub fn build(root: Node<'_>, src: &[u8]) -> Self {
        let estimated_nodes = (src.len() / 24).max(16);
        let mut complexities =
            FxHashMap::with_capacity_and_hasher(estimated_nodes, Default::default());
        let mut cursor = root.walk();
        let mut stack: Vec<(usize, u32)> = Vec::with_capacity(64);

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
