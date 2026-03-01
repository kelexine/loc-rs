// Author: kelexine (https://github.com/kelexine)
// extractors/rust.rs — Rust function/struct extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct RustExtractor;

impl Extractor for RustExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_rust::LANGUAGE.into()).is_err() {
            return vec![];
        }

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return vec![],
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut functions = Vec::new();

        traverse(tree.root_node(), content, &lines, &mut functions, false);

        functions.sort_by_key(|f| f.line_start);
        functions
    }
}

fn traverse(
    node: Node,
    content: &str,
    lines: &[&str],
    functions: &mut Vec<FunctionInfo>,
    in_impl: bool,
) {
    let kind = node.kind();
    let is_impl = kind == "impl_item";

    if kind == "function_item" {
        if let Some(info) = parse_function(node, content, lines, in_impl) {
            functions.push(info);
        }
    } else if kind == "struct_item" {
        if let Some(info) = parse_struct(node, content) {
            functions.push(info);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse(child, content, lines, functions, in_impl || is_impl);
    }
}

fn parse_function(node: Node, content: &str, lines: &[&str], is_method: bool) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut is_async = false;
    let mut is_pub = false;
    let mut has_test_attr = false;
    let mut params_str = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "async" {
            is_async = true;
        } else if kind == "visibility_modifier" {
            is_pub = true;
        } else if kind == "parameters" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "attribute_item" {
            let attr_text = child.utf8_text(content.as_bytes()).unwrap_or("");
            if attr_text.contains("test") {
                has_test_attr = true;
            }
        }
    }

    if has_test_attr {
        return None;
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let block = &lines[start_line.saturating_sub(1)..end_line.min(lines.len())];
    let complexity = estimate_complexity(block);

    // Extract parameters
    let mut parameters = Vec::new();
    let trimmed_params = params_str.trim_start_matches('(').trim_end_matches(')');
    if !trimmed_params.is_empty() {
        for p in trimmed_params.split(',') {
            let p_trim = p.trim();
            if !p_trim.is_empty() {
                parameters.push(p_trim.to_string());
            }
        }
    }

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async,
        is_method,
        is_class: false,
        docstring: None,
        decorators: if is_pub { vec!["pub".into()] } else { vec![] },
        complexity,
    })
}

fn parse_struct(node: Node, content: &str) -> Option<FunctionInfo> {
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
            break;
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters: vec![],
        is_async: false,
        is_method: false,
        is_class: true,
        docstring: None,
        decorators: vec![],
        complexity: 1,
    })
}
