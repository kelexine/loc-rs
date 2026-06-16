// Author: kelexine (https://github.com/kelexine)
// extractors/rust.rs — Rust function/struct extraction via Tree-sitter

use super::Extractor;
use super::tree_sitter::ast_complexity;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct RustExtractor;

impl Extractor for RustExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_rust::LANGUAGE.into(), content, |tree| {
            let mut functions = Vec::new();
            traverse(tree.root_node(), content, &mut functions, false);
            functions.sort_by_key(|f| f.line_start);
            functions
        })
        .unwrap_or_default()
    }
}

fn traverse(
    node: Node,
    content: &str,
    functions: &mut Vec<FunctionInfo>,
    in_impl: bool,
) {
    let kind = node.kind();
    let is_impl = kind == "impl_item";

    // Collect outer attributes that precede a function/struct item.
    // In tree-sitter-rust outer attribute_item nodes are siblings, not children.
    // We walk children of the current node and carry pending attributes forward.
    if kind == "source_file" || kind == "impl_item" || kind == "block" {
        let mut pending_attrs: Vec<String> = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let ckind = child.kind();
            if ckind == "attribute_item" {
                let text = child.utf8_text(content.as_bytes()).unwrap_or("");
                pending_attrs.push(text.to_string());
            } else if ckind == "function_item" {
                let is_test = pending_attrs.iter().any(|a| a.contains("test"));
                pending_attrs.clear();
                if !is_test {
                    if let Some(info) = parse_function(child, content, in_impl || is_impl) {
                        functions.push(info);
                    }
                }
            } else if ckind == "struct_item" {
                pending_attrs.clear();
                if let Some(info) = parse_struct(child, content) {
                    functions.push(info);
                }
            } else if ckind == "impl_item" {
                pending_attrs.clear();
                // Recurse into impl blocks with in_impl=true
                traverse(child, content, functions, true);
            } else {
                pending_attrs.clear();
                traverse(child, content, functions, in_impl || is_impl);
            }
        }
        return;
    }

    if kind == "function_item" {
        if let Some(info) = parse_function(node, content, in_impl) {
            functions.push(info);
        }
    } else if kind == "struct_item" {
        if let Some(info) = parse_struct(node, content) {
            functions.push(info);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse(child, content, functions, in_impl || is_impl);
    }
}

fn parse_function(
    node: Node,
    content: &str,
    is_method: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut is_async = false;
    let mut is_pub = false;
    let mut params_str = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "function_modifiers" {
            // In tree-sitter-rust, `async` lives inside function_modifiers
            let mod_text = child.utf8_text(content.as_bytes()).unwrap_or("");
            if mod_text.contains("async") {
                is_async = true;
            }
        } else if kind == "visibility_modifier" {
            is_pub = true;
        } else if kind == "parameters" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let complexity = ast_complexity(node, content.as_bytes());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_functions() {
        let content = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

async fn fetch() {}

struct Point { x: f64, y: f64 }

impl Point {
    pub fn new(x: f64, y: f64) -> Self { Point { x, y } }
}
"#;
        let extractor = RustExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));

        // add, fetch, new, Point (struct)
        assert_eq!(fns.len(), 4);

        let add = fns.iter().find(|f| f.name == "add").unwrap();
        assert!(!add.is_async);
        assert!(add.decorators.contains(&"pub".to_string()));

        let fetch = fns.iter().find(|f| f.name == "fetch").unwrap();
        assert!(fetch.is_async);

        let point = fns.iter().find(|f| f.name == "Point").unwrap();
        assert!(point.is_class);

        let new = fns.iter().find(|f| f.name == "new").unwrap();
        assert!(new.is_method);
    }

    #[test]
    fn test_rust_test_functions_excluded() {
        let content = r#"
#[test]
fn my_test() {}

fn real_fn() {}
"#;
        let extractor = RustExtractor;
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "real_fn");
    }
}
