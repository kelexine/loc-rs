// Author: kelexine (https://github.com/kelexine)
// extractors/go.rs — Go function extraction via Tree-sitter

use super::Extractor;
use super::tree_sitter::ComplexityIndex;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct GoExtractor;

impl Extractor for GoExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_go::LANGUAGE.into(), content, |tree| {
            let index = ComplexityIndex::build(tree.root_node(), content.as_bytes());
            let mut functions = Vec::new();
            traverse(tree.root_node(), content, &mut functions, &index);
            functions.sort_by_key(|f| f.line_start);
            functions
        })
        .unwrap_or_default()
    }
}

/// Explicit-stack DFS — see `rust.rs::traverse` for why this is not a
/// recursive walk: extraction runs on rayon worker threads with 2 MiB stacks.
fn traverse(node: Node, content: &str, functions: &mut Vec<FunctionInfo>, index: &ComplexityIndex) {
    let mut stack: Vec<Node> = vec![node];
    let mut children: Vec<Node> = Vec::new();

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if kind == "function_declaration"
            && let Some(info) = parse_function(node, content, false, index)
        {
            functions.push(info);
        } else if kind == "method_declaration"
            && let Some(info) = parse_function(node, content, true, index)
        {
            functions.push(info);
        }

        children.clear();
        {
            let mut cursor = node.walk();
            children.extend(node.children(&mut cursor));
        }
        // Reverse push preserves the pre-order a recursive walk would produce.
        for &child in children.iter().rev() {
            stack.push(child);
        }
    }
}

fn parse_function(
    node: Node,
    content: &str,
    is_method: bool,
    index: &ComplexityIndex,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut params_str = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if (kind == "identifier" || kind == "field_identifier") && name.is_empty() {
            name = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        } else if kind == "parameter_list" {
            params_str = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        }
    }

    if name.is_empty() {
        return None;
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let complexity = index.get(node);

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
        let content = r#"
package main
func Hello() {}
func (r *Repo) Get(id int) string {
    return ""
}
"#;
        let extractor = GoExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(fns.len(), 2);

        let g = fns.iter().find(|f| f.name == "Get").unwrap();
        assert!(g.is_method);
        assert_eq!(g.parameters, vec!["id int"]);

        let h = fns.iter().find(|f| f.name == "Hello").unwrap();
        assert!(!h.is_method);
    }
}
