// Author: kelexine (https://github.com/kelexine)
// extractors/javascript.rs — JavaScript/TypeScript function/class extraction via Tree-sitter

use super::Extractor;
use super::tree_sitter::ast_complexity;
use crate::models::FunctionInfo;
use tree_sitter::{Language, Node};

pub struct JavascriptExtractor {
    language: Language,
}

impl JavascriptExtractor {
    pub fn new(language: Language) -> Self {
        Self { language }
    }
}

impl Extractor for JavascriptExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(self.language.clone(), content, |tree| {
            let mut functions = Vec::new();
            traverse(tree.root_node(), content, &mut functions, false);
            functions.retain(|f| f.name != "?");
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

    if matches!(
        kind,
        "function_declaration"
            | "generator_function_declaration"
            | "method_definition"
            | "arrow_function"
            | "function"
    ) {
        if let Some(info) = parse_function(node, content, kind == "method_definition") {
            functions.push(info);
        }
    } else if kind == "class_declaration" || kind == "class" {
        if let Some(info) = parse_class(node, content) {
            functions.push(info);
        }
    } else if kind == "lexical_declaration" || kind == "variable_declaration" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator"
                && let Some(info) = parse_variable_declarator(child, content)
            {
                functions.push(info);
            }
        }
    }

    let is_class_body = kind == "class_body";

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
        if (kind == "identifier" || kind == "property_identifier") && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "formal_parameters" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let text = node.utf8_text(content.as_bytes()).unwrap_or("");
    let is_async = text.starts_with("async ");

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
        decorators: vec![],
        complexity,
    })
}

fn parse_variable_declarator(
    node: Node,
    content: &str,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut func_node = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "arrow_function" || kind == "function" {
            func_node = Some(child);
        }
    }

    if let Some(fnode) = func_node
        && !name.is_empty()
    {
        let mut info = parse_function(fnode, content, false)?;
        info.name = name;
        let text = fnode.utf8_text(content.as_bytes()).unwrap_or("");
        if text.starts_with("async ") {
            info.is_async = true;
        }
        return Some(info);
    }
    None
}

fn parse_class(node: Node, content: &str) -> Option<FunctionInfo> {
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if (kind == "identifier" || kind == "type_identifier") && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
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
    fn test_extract_js_functions() {
        let content = r#"
export async function fetchData(url) {}
const process = (data) => {}
class Calculator {
    add(a, b) {}
}
"#;
        let extractor = JavascriptExtractor::new(tree_sitter_javascript::LANGUAGE.into());
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(fns.len(), 4);

        let c = fns.iter().find(|f| f.name == "Calculator").unwrap();
        assert!(c.is_class);

        let a = fns.iter().find(|f| f.name == "add").unwrap();
        assert!(a.is_method);

        let fd = fns.iter().find(|f| f.name == "fetchData").unwrap();
        assert!(fd.is_async);

        let p = fns.iter().find(|f| f.name == "process").unwrap();
        assert!(!p.is_async);
    }
}
