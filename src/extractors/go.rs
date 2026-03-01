// Author: kelexine (https://github.com/kelexine)
// extractors/go.rs — Go function extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct GoExtractor;

impl Extractor for GoExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_go::LANGUAGE.into()).is_err() {
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

    if kind == "function_declaration" {
        if let Some(info) = parse_function(node, content, lines, false) {
            functions.push(info);
        }
    } else if kind == "method_declaration"
        && let Some(info) = parse_function(node, content, lines, true)
    {
        functions.push(info);
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" || kind == "field_identifier" {
            if name.is_empty() {
                name = child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
            }
        } else if kind == "parameter_list" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_go_functions() {
        let content = "
package main
func Hello() {}
func (r *Repo) Get(id int) string {
    return \"\"
}
";
        let extractor = GoExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(fns.len(), 2);
        
        let g = fns.iter().find(|f| f.name == "Get").unwrap();
        assert!(g.is_method);
        // Usually `parameter_list` extracts the parameters text as `(id int)`.
        // My simple split on `,` gets `id int`.
        assert_eq!(g.parameters, vec!["id int"]);
        
        let h = fns.iter().find(|f| f.name == "Hello").unwrap();
        assert!(!h.is_method);
    }
}
