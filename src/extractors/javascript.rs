// Author: kelexine (https://github.com/kelexine)
// extractors/javascript.rs — JavaScript/TypeScript function/class extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Language, Node, Parser};

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
        let mut parser = Parser::new();
        if parser.set_language(&self.language).is_err() {
            return vec![];
        }

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return vec![],
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut functions = Vec::new();

        traverse(tree.root_node(), content, &lines, &mut functions, false);

        functions.retain(|f| f.name != "?");
        functions.sort_by_key(|f| f.line_start);
        functions
    }
}

fn traverse(
    node: Node,
    content: &str,
    lines: &[&str],
    functions: &mut Vec<FunctionInfo>,
    _in_class: bool,
) {
    let kind = node.kind();

    if kind == "function_declaration" || kind == "generator_function_declaration" || kind == "method_definition" || kind == "arrow_function" || kind == "function" {
        if let Some(info) = parse_function(node, content, lines, kind == "method_definition") {
            functions.push(info);
        }
    } else if kind == "class_declaration" || kind == "class" {
        if let Some(info) = parse_class(node, content, lines) {
            functions.push(info);
        }
    } else if kind == "lexical_declaration" || kind == "variable_declaration" {
        // Find arrow functions or anonymous functions assigned to variables
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator"
                && let Some(info) = parse_variable_declarator(child, content, lines)
            {
                functions.push(info);
            }
        }
    }

    let is_class_body = kind == "class_body";

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Arrow functions and nested functions can be inside others, 
        // we keep traversing unless we already processed an arrow function node itself above.
        // Actually, parse_function doesn't traverse into the body to find nested functions, 
        // so we SHOULD traverse into the body of functions to find nested ones!
        traverse(child, content, lines, functions, _in_class || is_class_body);
    }
}

fn parse_function(
    node: Node,
    content: &str,
    lines: &[&str],
    is_method: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut is_async = false;
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

    // For arrow functions inside variable declarators, the name might not be here. 
    // It's handled by `parse_variable_declarator`.
    // But if we encounter an arrow_function here, we can give it a default name "?" if missing.

    if name.is_empty() {
        name = "?".to_string();
    }

    // Check async modifier. TS/JS async is usually a child node of function_declaration, or before it.
    // Let's just check the text for async since tree-sitter JS parses it as a modifier sometimes.
    let text = node.utf8_text(content.as_bytes()).unwrap_or("");
    if text.starts_with("async ") {
        is_async = true;
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let block = &lines[start_line.saturating_sub(1)..end_line.min(lines.len())];
    let complexity = estimate_complexity(block);

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
    lines: &[&str],
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
        let mut info = parse_function(fnode, content, lines, false)?;
        // Override the name since parse_function wouldn't find it inside the arrow_function
        info.name = name;
        // Async check for arrow function
        let text = fnode.utf8_text(content.as_bytes()).unwrap_or("");
        if text.starts_with("async ") {
            info.is_async = true;
        }
        return Some(info);
    }
    None
}

fn parse_class(node: Node, content: &str, _lines: &[&str]) -> Option<FunctionInfo> {
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
        let content = "
export async function fetchData(url) {}
const process = (data) => {}
class Calculator {
    add(a, b) {}
}
";
        let extractor = JavascriptExtractor::new(tree_sitter_javascript::LANGUAGE.into());
        let mut fns = extractor.extract(content);
        // Sort by name for deterministic test order
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
