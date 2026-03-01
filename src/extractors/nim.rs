// Author: kelexine (https://github.com/kelexine)
// extractors/nim.rs — Nim function/class extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct NimExtractor;

impl Extractor for NimExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_nim::language()).is_err() {
            return vec![];
        }

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return vec![],
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut functions = Vec::new();

        traverse(tree.root_node(), content, &lines, &mut functions);

        functions.sort_by_key(|f| f.line_start);
        functions
    }
}

fn traverse(
    node: Node,
    content: &str,
    lines: &[&str],
    functions: &mut Vec<FunctionInfo>,
) {
    let kind = node.kind();

    if kind == "proc_declaration" || kind == "func_declaration" || kind == "method_declaration" || kind == "iterator_declaration" || kind == "macro_declaration" || kind == "template_declaration" {
        if let Some(info) = parse_function(node, content, lines, kind == "method_declaration") {
            functions.push(info);
        }
    } else if kind == "type_declaration" {
        // Find object or ref object
        let mut is_class = false;
        let mut name = String::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_symbol_declaration" && name.is_empty() {
                name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
            } else if child.kind() == "object_declaration" || child.kind() == "ref_object_declaration" {
                is_class = true;
            }
        }

        if is_class && !name.is_empty() {
            let start_line = node.start_position().row + 1;
            let end_line = node.end_position().row + 1;
            
            functions.push(FunctionInfo {
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
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse(child, content, lines, functions);
    }
}

fn parse_function(
    node: Node,
    content: &str,
    lines: &[&str],
    is_method: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut params_str = String::new();
    let mut is_exported = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "exported_symbol" && name.is_empty() {
            is_exported = true;
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    name = inner_child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                    break;
                }
            }
        } else if kind == "parameter_declaration_list" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        }
    }

    if name.is_empty() {
        return None;
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let block = &lines[start_line.saturating_sub(1)..end_line.min(lines.len())];
    let complexity = estimate_complexity(block);

    let mut parameters = Vec::new();
    let trimmed_params = params_str.trim_start_matches('(').trim_end_matches(')');
    if !trimmed_params.is_empty() {
        for p in trimmed_params.split(';') {
            let p_trim = p.trim();
            if !p_trim.is_empty() {
                parameters.push(p_trim.to_string());
            }
        }
    }

    let decorators = if is_exported {
        vec!["public(*)".into()]
    } else {
        vec![]
    };

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async: false,
        is_method,
        is_class: false,
        docstring: None,
        decorators,
        complexity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_nim_functions() {
        let content = "
proc hello*(name: string) =
  echo \"Hello \", name

func add(a, b: int): int =
  a + b

method draw(s: Shape) =
  discard
";
        let extractor = NimExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));
        
        assert_eq!(fns.len(), 3);
        
        let a = fns.iter().find(|f| f.name == "add").unwrap();
        assert!(!a.is_method);
        
        let d = fns.iter().find(|f| f.name == "draw").unwrap();
        assert!(d.is_method);
        
        let h = fns.iter().find(|f| f.name == "hello").unwrap();
        assert!(h.decorators.contains(&"public(*)".to_string()));
    }
}
