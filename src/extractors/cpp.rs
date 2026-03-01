// Author: kelexine (https://github.com/kelexine)
// extractors/cpp.rs — C/C++ function extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct CppExtractor;

impl Extractor for CppExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_cpp::LANGUAGE.into()).is_err() {
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

    if kind == "function_definition" {
        if let Some(info) = parse_function(node, content, lines, in_class) {
            functions.push(info);
        }
    } else if (kind == "class_specifier" || kind == "struct_specifier")
        && let Some(info) = parse_class(node, content, lines)
    {
        functions.push(info);
    }

    let is_class_body = kind == "field_declaration_list";

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
    let mut params_str = String::new();

    // In tree-sitter-cpp, the function_definition has a declarator child.
    // That declarator might be a function_declarator, which in turn has an identifier (name)
    // and a parameter_list.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declarator" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                let ikind = inner_child.kind();
                if ikind == "identifier" || ikind == "field_identifier" || ikind == "destructor_name" {
                    name = inner_child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                } else if ikind == "parameter_list" {
                    params_str = inner_child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                }
            }
        }
    }

    // fallback if not found in immediate children (e.g. nested declarators)
    if name.is_empty()
        && let Some(decl) = find_descendant(node, "function_declarator")
    {
        let mut inner_cursor = decl.walk();
        for inner_child in decl.children(&mut inner_cursor) {
            let ikind = inner_child.kind();
            if (ikind == "identifier" || ikind == "field_identifier" || ikind == "destructor_name") && name.is_empty() {
                name = inner_child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
            } else if ikind == "parameter_list" && params_str.is_empty() {
                params_str = inner_child.utf8_text(content.as_bytes()).unwrap_or("").to_string();
            }
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

fn find_descendant<'a>(node: Node<'a>, target_kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == target_kind {
            return Some(child);
        }
        if let Some(found) = find_descendant(child, target_kind) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cpp_functions() {
        let content = "
void Hello() {}
int main(int argc, char** argv) { return 0; }
class Box {
public:
    void SetWidth(double wid) {}
};
";
        let extractor = CppExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));
        
        assert_eq!(fns.len(), 4);
        
        let c = fns.iter().find(|f| f.name == "Box").unwrap();
        assert!(c.is_class);
        
        let h = fns.iter().find(|f| f.name == "Hello").unwrap();
        assert!(!h.is_method);
        
        let m = fns.iter().find(|f| f.name == "main").unwrap();
        assert!(!m.is_method);
        assert_eq!(m.parameters, vec!["int argc", "char** argv"]);
        
        let s = fns.iter().find(|f| f.name == "SetWidth").unwrap();
        assert!(s.is_method);
        assert_eq!(s.parameters, vec!["double wid"]);
    }
}
