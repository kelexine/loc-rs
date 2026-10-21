// Author: kelexine (https://github.com/kelexine)
// Date: 2026-10-21
// Purpose: C# function/class extraction via Tree-sitter (`tree-sitter-c-sharp`)

use super::Extractor;
use super::tree_sitter::ComplexityIndex;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct CSharpExtractor;

impl Extractor for CSharpExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_c_sharp::LANGUAGE.into(), content, |tree| {
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
    in_type: bool,
) {
    let mut stack: Vec<(Node, bool)> = vec![(node, in_type)];
    let mut children: Vec<Node> = Vec::new();

    while let Some((node, in_type)) = stack.pop() {
        let kind = node.kind();

        if matches!(
            kind,
            "method_declaration"
                | "constructor_declaration"
                | "destructor_declaration"
                | "local_function_statement"
        ) && let Some(info) = parse_method(node, content, index, in_type)
        {
            functions.push(info);
        } else if matches!(
            kind,
            "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "record_declaration"
                | "enum_declaration"
        ) && let Some(info) = parse_type(node, content)
        {
            functions.push(info);
        }

        let enters_type = matches!(
            kind,
            "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "record_declaration"
        );

        children.clear();
        {
            let mut cursor = node.walk();
            children.extend(node.children(&mut cursor));
        }
        for &child in children.iter().rev() {
            stack.push((child, in_type || enters_type));
        }
    }
}

fn parse_method(
    node: Node,
    content: &str,
    index: &ComplexityIndex,
    in_type: bool,
) -> Option<FunctionInfo> {
    let mut name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(content.as_bytes()).ok())
        .unwrap_or("")
        .to_string();

    if node.kind() == "destructor_declaration" && !name.is_empty() {
        name = format!("~{name}");
    }

    let mut parameters = Vec::new();
    let mut is_async = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        } else if kind == "parameter_list" {
            collect_parameters(child, content, &mut parameters);
        } else if kind == "modifier" {
            let text = child.utf8_text(content.as_bytes()).unwrap_or("");
            if text == "async" {
                is_async = true;
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    let is_local = node.kind() == "local_function_statement";
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async,
        is_method: in_type && !is_local,
        is_class: false,
        docstring: None,
        decorators: vec![],
        complexity: index.get(node),
    })
}

fn collect_parameters(params_node: Node, content: &str, out: &mut Vec<String>) {
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if child.kind() == "parameter" {
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
    fn test_extract_csharp_functions_and_types() {
        let content = r#"
namespace Demo {
    public class OrderService {
        public OrderService(int timeout) {}

        public async Task<int> ProcessAsync(int id, string tag) {
            if (id > 0 && tag != null) {
                foreach (var item in tag) {
                    return 1;
                }
            }
            return id switch {
                1 => 10,
                _ => 20,
            };
        }
    }

    public record OrderDto(int Id);
}
"#;
        let extractor = CSharpExtractor;
        let fns = extractor.extract(content);

        let cls = fns.iter().find(|f| f.name == "OrderService").unwrap();
        assert!(cls.is_class);

        let rec = fns.iter().find(|f| f.name == "OrderDto").unwrap();
        assert!(rec.is_class);

        let ctor = fns
            .iter()
            .find(|f| f.name == "OrderService" && !f.is_class)
            .unwrap();
        assert!(ctor.is_method);
        assert_eq!(ctor.parameters, vec!["int timeout"]);

        let proc_fn = fns.iter().find(|f| f.name == "ProcessAsync").unwrap();
        assert!(proc_fn.is_method);
        assert!(proc_fn.is_async);
        assert_eq!(proc_fn.parameters, vec!["int id", "string tag"]);
        // 1 + if + && + foreach + 2 switch_expression_arm = 6
        assert_eq!(proc_fn.complexity, 6);
    }
}
