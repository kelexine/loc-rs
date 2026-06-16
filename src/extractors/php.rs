// Author: kelexine (https://github.com/kelexine)
// extractors/php.rs — PHP function/class extraction via Tree-sitter

use super::Extractor;
use super::tree_sitter::ast_complexity;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct PhpExtractor;

impl Extractor for PhpExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_php::LANGUAGE_PHP.into(), content, |tree| {
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
    in_class: bool,
) {
    let kind = node.kind();

    if kind == "function_definition" || kind == "method_declaration" {
        if let Some(info) = parse_function(node, content, kind == "method_declaration") {
            functions.push(info);
        }
    } else if (kind == "class_declaration"
        || kind == "interface_declaration"
        || kind == "trait_declaration")
        && let Some(info) = parse_class(node, content)
    {
        functions.push(info);
    }

    let is_class_body = kind == "declaration_list";

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse(child, content, functions, in_class || is_class_body);
    }
}

fn parse_function(
    node: Node,
    content: &str,
    is_method: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut params_str = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "name" {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "formal_parameters" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        }
    }

    if name.is_empty() {
        if let Some(name_node) = node.child_by_field_name("name") {
            name = name_node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else {
            let mut c2 = node.walk();
            for child in node.children(&mut c2) {
                if child.kind() == "name" || child.kind() == "identifier" {
                    name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                    break;
                }
            }
        }
    }

    if name.is_empty() {
        return None;
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
        is_async: false,
        is_method,
        is_class: false,
        docstring: None,
        decorators: vec![],
        complexity,
    })
}

fn parse_class(node: Node, content: &str) -> Option<FunctionInfo> {
    let mut name = String::new();

    if let Some(name_node) = node.child_by_field_name("name") {
        name = name_node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "name" && name.is_empty() {
                name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                break;
            }
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
    fn test_extract_php() {
        let content = r#"
<?php
class User {
    public function getName($id) {
        return 'kelexine';
    }
}
function helper() {}
"#;
        let extractor = PhpExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(fns.len(), 3);
        assert!(fns.iter().any(|f| f.name == "User" && f.is_class));
        assert!(fns.iter().any(|f| f.name == "getName" && f.is_method));
        assert!(fns.iter().any(|f| f.name == "helper" && !f.is_method));
    }
}
