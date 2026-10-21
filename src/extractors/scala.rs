// Author: kelexine (https://github.com/kelexine)
// Date: 2026-10-21
// Purpose: Scala function/class extraction via Tree-sitter (`tree-sitter-scala`)

use super::Extractor;
use super::tree_sitter::ComplexityIndex;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct ScalaExtractor;

impl Extractor for ScalaExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_scala::LANGUAGE.into(), content, |tree| {
            let index = ComplexityIndex::build(tree.root_node(), content.as_bytes());
            let mut functions = Vec::new();
            traverse(tree.root_node(), content, &index, &mut functions, false);
            functions.sort_by_key(|f| f.line_start);
            functions
        })
        .unwrap_or_default()
    }
}

/// Explicit-stack DFS — see `rust.rs::traverse` for why this is not a
/// recursive walk: extraction runs on rayon worker threads with 2 MiB stacks.
fn traverse(
    node: Node,
    content: &str,
    index: &ComplexityIndex,
    functions: &mut Vec<FunctionInfo>,
    in_template: bool,
) {
    let mut stack: Vec<(Node, bool)> = vec![(node, in_template)];
    let mut children: Vec<Node> = Vec::new();

    while let Some((node, in_template)) = stack.pop() {
        let kind = node.kind();

        if matches!(kind, "function_definition" | "function_declaration")
            && let Some(info) = parse_function(node, content, index, in_template)
        {
            functions.push(info);
        } else if matches!(
            kind,
            "class_definition" | "object_definition" | "trait_definition" | "enum_definition"
        ) && let Some(info) = parse_type(node, content)
        {
            functions.push(info);
        }

        let is_template_body = matches!(kind, "template_body" | "enum_body");

        children.clear();
        {
            let mut cursor = node.walk();
            children.extend(node.children(&mut cursor));
        }
        for &child in children.iter().rev() {
            stack.push((child, in_template || is_template_body));
        }
    }
}

fn parse_function(
    node: Node,
    content: &str,
    index: &ComplexityIndex,
    in_template: bool,
) -> Option<FunctionInfo> {
    let mut name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(content.as_bytes()).ok())
        .unwrap_or("")
        .to_string();

    let mut parameters = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        } else if kind == "parameters" {
            collect_parameters(child, content, &mut parameters);
        }
    }

    if name.is_empty() {
        return None;
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async: false,
        is_method: in_template,
        is_class: false,
        docstring: None,
        decorators: vec![],
        complexity: index.get(node),
    })
}

fn collect_parameters(params_node: Node, content: &str, out: &mut Vec<String>) {
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if matches!(child.kind(), "parameter" | "class_parameter") {
            let text = child.utf8_text(content.as_bytes()).unwrap_or("").trim();
            if !text.is_empty() {
                out.push(text.to_string());
            }
        }
    }
}

fn parse_type(node: Node, content: &str) -> Option<FunctionInfo> {
    let mut name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(content.as_bytes()).ok())
        .unwrap_or("")
        .to_string();

    if name.is_empty() {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                name = child
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if name.is_empty() {
        return None;
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
    fn test_extract_scala_functions_and_types() {
        let content = r#"
package demo

class Calculator {
  def compute(a: Int, b: Int): Int = {
    if (a > 0 && b > 0) {
      a + b
    } else {
      a match {
        case 0 => b
        case _ => 0
      }
    }
  }
}

object Helpers {
  def ping(): String = "pong"
}

trait Service {
  def run(id: Long): Unit
}
"#;
        let extractor = ScalaExtractor;
        let fns = extractor.extract(content);

        let calc = fns.iter().find(|f| f.name == "Calculator").unwrap();
        assert!(calc.is_class);

        let helpers = fns.iter().find(|f| f.name == "Helpers").unwrap();
        assert!(helpers.is_class);

        let svc = fns.iter().find(|f| f.name == "Service").unwrap();
        assert!(svc.is_class);

        let compute = fns.iter().find(|f| f.name == "compute").unwrap();
        assert!(compute.is_method);
        assert_eq!(compute.parameters, vec!["a: Int", "b: Int"]);
        // 1 + if + && + 2 case_clause = 5
        assert_eq!(compute.complexity, 5);

        let ping = fns.iter().find(|f| f.name == "ping").unwrap();
        assert!(ping.is_method);

        let run = fns.iter().find(|f| f.name == "run").unwrap();
        assert!(run.is_method);
        assert_eq!(run.parameters, vec!["id: Long"]);
    }
}
