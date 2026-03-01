// Author: kelexine (https://github.com/kelexine)
// extractors/python.rs — Python function/class extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct PythonExtractor;

impl Extractor for PythonExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_python::LANGUAGE.into()).is_err() {
            return vec![];
        }

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return vec![],
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut functions = Vec::new();

        traverse(tree.root_node(), content, &lines, &mut functions, false, Vec::new());

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
    mut pending_decorators: Vec<String>,
) {
    let kind = node.kind();
    
    if kind == "decorator" {
        // Just extract the decorator text, e.g., "@staticmethod"
        let dec_text = node.utf8_text(content.as_bytes()).unwrap_or("");
        pending_decorators.push(dec_text.trim_start_matches('@').to_string());
        return; // we don't need to traverse inside decorator
    } else if kind == "decorated_definition" {
        // Collect decorators and pass them to children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            traverse(child, content, lines, functions, in_class, pending_decorators.clone());
        }
        return;
    }

    if kind == "function_definition" {
        functions.push(parse_function(node, content, lines, in_class, pending_decorators.clone()));
        pending_decorators.clear();
    } else if kind == "class_definition" {
        functions.push(parse_class(node, content, lines, pending_decorators.clone()));
        pending_decorators.clear();
    }

    let is_class_body = kind == "class_definition";
    
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Only pass decorators down if we're in a decorated_definition (handled above)
        // If we hit a normal statement, clear pending decorators (though they shouldn't leak)
        traverse(child, content, lines, functions, in_class || is_class_body, Vec::new());
    }
}

fn parse_function(
    node: Node,
    content: &str,
    lines: &[&str],
    is_method: bool,
    decorators: Vec<String>,
) -> FunctionInfo {
    let mut name = String::new();
    let mut is_async = false;
    let mut params_str = String::new();
    let mut docstring = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "async" {
            is_async = true;
        } else if kind == "parameters" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "block" {
            // Find docstring
            if child.child_count() > 0 {
                let first_stmt = child.child(0).unwrap();
                if first_stmt.kind() == "expression_statement" {
                    if first_stmt.child_count() > 0 {
                        let expr = first_stmt.child(0).unwrap();
                        if expr.kind() == "string" {
                            let doc = expr.utf8_text(content.as_bytes()).unwrap_or("");
                            docstring = Some(clean_docstring(doc));
                        }
                    }
                }
            }
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let block = &lines[start_line.saturating_sub(1)..end_line.min(lines.len())];
    let complexity = estimate_complexity(block);

    // Parse parameters
    let mut parameters = Vec::new();
    let trimmed_params = params_str.trim_start_matches('(').trim_end_matches(')');
    if !trimmed_params.is_empty() {
        for p in trimmed_params.split(',') {
            let p_trim = p.trim();
            // simple split might fail on default args with commas (like tuples), 
            // but for simple cases it's identical to the regex logic.
            if !p_trim.is_empty() {
                parameters.push(p_trim.to_string());
            }
        }
    }

    // Heuristic: If first param is self or cls, it's definitely a method
    let actual_is_method = is_method || parameters.first().map(|p| p.starts_with("self") || p.starts_with("cls")).unwrap_or(false);

    FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async,
        is_method: actual_is_method,
        is_class: false,
        docstring,
        decorators,
        complexity,
    }
}

fn parse_class(
    node: Node,
    content: &str,
    _lines: &[&str],
    decorators: Vec<String>,
) -> FunctionInfo {
    let mut name = String::new();
    let mut params_str = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        } else if kind == "argument_list" {
            params_str = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

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

    FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters, // Used for bases in class
        is_async: false,
        is_method: false,
        is_class: true,
        docstring: None,
        decorators,
        complexity: 1,
    }
}

fn clean_docstring(doc: &str) -> String {
    let s = doc.trim();
    if (s.starts_with("\"\"\"") && s.ends_with("\"\"\"") && s.len() >= 6) ||
       (s.starts_with("'''") && s.ends_with("'''") && s.len() >= 6) {
        s[3..s.len() - 3].trim().to_string()
    } else if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2) ||
              (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2) {
        s[1..s.len() - 1].trim().to_string()
    } else {
        s.to_string()
    }
}
