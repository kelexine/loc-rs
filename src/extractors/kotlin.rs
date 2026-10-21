// Author: kelexine (https://github.com/kelexine)
// Date: 2026-10-21
// Purpose: Kotlin function/class extraction via Tree-sitter (`tree-sitter-kotlin-ng`)

use super::Extractor;
use super::tree_sitter::ComplexityIndex;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct KotlinExtractor;

impl Extractor for KotlinExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_kotlin_ng::LANGUAGE.into(), content, |tree| {
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
    in_class: bool,
) {
    let mut stack: Vec<(Node, bool)> = vec![(node, in_class)];
    let mut children: Vec<Node> = Vec::new();

    while let Some((node, in_class)) = stack.pop() {
        let kind = node.kind();

        if (kind == "function_declaration" || kind == "secondary_constructor")
            && let Some(info) = parse_function(node, content, index, in_class)
        {
            functions.push(info);
        } else if (kind == "class_declaration"
            || kind == "object_declaration"
            || kind == "companion_object")
            && let Some(info) = parse_class(node, content)
        {
            functions.push(info);
        }

        let is_class_body = matches!(kind, "class_body" | "enum_class_body");

        children.clear();
        {
            let mut cursor = node.walk();
            children.extend(node.children(&mut cursor));
        }
        for &child in children.iter().rev() {
            stack.push((child, in_class || is_class_body));
        }
    }
}

fn parse_function(
    node: Node,
    content: &str,
    index: &ComplexityIndex,
    in_class: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut is_async = false;
    let mut has_receiver = false;

    if node.kind() == "secondary_constructor" {
        name = "constructor".to_string();
    } else if let Some(name_node) = node.child_by_field_name("name") {
        name = name_node
            .utf8_text(content.as_bytes())
            .unwrap_or("")
            .to_string();
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        } else if kind == "." {
            has_receiver = true;
        } else if kind == "function_value_parameters" {
            collect_parameters(child, content, &mut parameters);
        } else if kind == "modifiers" {
            let mod_text = child.utf8_text(content.as_bytes()).unwrap_or("");
            if mod_text.split_whitespace().any(|t| t == "suspend") {
                is_async = true;
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
        parameters,
        is_async,
        is_method: in_class || has_receiver || node.kind() == "secondary_constructor",
        is_class: false,
        docstring: None,
        decorators: vec![],
        complexity: index.get(node),
    })
}

fn collect_parameters(params_node: Node, content: &str, out: &mut Vec<String>) {
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if matches!(child.kind(), "parameter" | "parameter_with_optional_type") {
            let text = child.utf8_text(content.as_bytes()).unwrap_or("").trim();
            if !text.is_empty() {
                out.push(text.to_string());
            }
        }
    }
}

fn parse_class(node: Node, content: &str) -> Option<FunctionInfo> {
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
        if node.kind() == "companion_object" {
            name = "Companion".to_string();
        } else {
            return None;
        }
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
    fn test_extract_kotlin_functions_and_classes() {
        let content = r#"
class UserService(private val repo: Repo) {
    constructor(id: Int) : this(Repo(id))

    suspend fun fetchUser(id: Int, includeMeta: Boolean): User? {
        if (id <= 0 && !includeMeta) {
            return null
        }
        return when (id) {
            1 -> repo.find(1)
            2 -> repo.find(2)
            else -> repo.find(id)
        }
    }
}

object MathUtil {
    fun clamp(v: Int, min: Int, max: Int): Int = v
}

fun String.shout(): String = this.uppercase()
"#;
        let extractor = KotlinExtractor;
        let fns = extractor.extract(content);

        let cls = fns.iter().find(|f| f.name == "UserService").unwrap();
        assert!(cls.is_class);

        let obj = fns.iter().find(|f| f.name == "MathUtil").unwrap();
        assert!(obj.is_class);

        let ctor = fns.iter().find(|f| f.name == "constructor").unwrap();
        assert!(ctor.is_method);
        assert_eq!(ctor.parameters, vec!["id: Int"]);

        let fetch = fns.iter().find(|f| f.name == "fetchUser").unwrap();
        assert!(fetch.is_method);
        assert!(fetch.is_async);
        assert_eq!(fetch.parameters, vec!["id: Int", "includeMeta: Boolean"]);
        // 1 + if + && + 3 when_entry branches = 6
        assert_eq!(fetch.complexity, 6);

        let clamp = fns.iter().find(|f| f.name == "clamp").unwrap();
        assert!(clamp.is_method);
        assert_eq!(clamp.parameters, vec!["v: Int", "min: Int", "max: Int"]);

        let shout = fns.iter().find(|f| f.name == "shout").unwrap();
        assert!(shout.is_method);
    }
}
