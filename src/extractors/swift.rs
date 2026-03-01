// Author: kelexine (https://github.com/kelexine)
// extractors/swift.rs — Swift function/class extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct SwiftExtractor;

impl Extractor for SwiftExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_swift::LANGUAGE.into()).is_err() {
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
    in_class: bool,
) {
    let kind = node.kind();

    if kind == "function_declaration" || kind == "init_declaration" {
        if let Some(info) = parse_function(node, content, lines, in_class) {
            functions.push(info);
        }
    } else if (kind == "class_declaration" || kind == "struct_declaration" || kind == "enum_declaration" || kind == "protocol_declaration" || kind == "extension_declaration")
        && let Some(info) = parse_class(node, content, lines)
    {
        functions.push(info);
    }

    let is_class_body = kind == "class_body" || kind == "struct_body" || kind == "enum_body" || kind == "protocol_body" || kind == "extension_body";

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse(child, content, lines, functions, in_class || is_class_body);
    }
}

fn parse_function(
    node: Node,
    content: &str,
    lines: &[&str],
    is_method: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut is_async = false;
    let mut is_explicit_method = is_method;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "simple_identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "parameter" {
            parameters.push(child.utf8_text(content.as_bytes()).unwrap_or("").to_string());
        } else if kind == "modifiers" {
            let mod_text = child.utf8_text(content.as_bytes()).unwrap_or("");
            if mod_text.contains("mutating") || mod_text.contains("override") || mod_text.contains("static") || mod_text.contains("class") {
                is_explicit_method = true;
            }
        } else if kind == "async" {
            is_async = true;
        }
    }

    if node.kind() == "init_declaration" {
        name = "init".to_string();
    }

    if name.is_empty() {
        return None;
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let block = &lines[start_line.saturating_sub(1)..end_line.min(lines.len())];
    let complexity = estimate_complexity(block);

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async,
        is_method: is_explicit_method,
        is_class: false,
        docstring: None,
        decorators: vec![],
        complexity,
    })
}

fn parse_class(node: Node, content: &str, _lines: &[&str]) -> Option<FunctionInfo> {
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" && name.is_empty() {
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
    fn test_extract_swift_functions() {
        let content = "
import Foundation
func hello() {}
public func fetchData() async -> Data? { return nil }
class Service {
    override func start() {}
}
struct Point {
    var x, y: Double
    mutating func moveBy(x deltaX: Double, y deltaY: Double) {}
}
";
        let extractor = SwiftExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));
        
        assert_eq!(fns.len(), 6);
        
        let c = fns.iter().find(|f| f.name == "Service").unwrap();
        assert!(c.is_class);
        
        let p = fns.iter().find(|f| f.name == "Point").unwrap();
        assert!(p.is_class);
        
        let h = fns.iter().find(|f| f.name == "hello").unwrap();
        assert!(!h.is_method);
        
        let m = fns.iter().find(|f| f.name == "moveBy").unwrap();
        assert!(m.is_method);
        assert_eq!(m.parameters, vec!["x deltaX: Double", "y deltaY: Double"]);
        
        let f = fns.iter().find(|f| f.name == "fetchData").unwrap();
        assert!(f.is_async);
    }
}
