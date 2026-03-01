// Author: kelexine (https://github.com/kelexine)
// extractors/ruby.rs — Ruby function/class extraction via Tree-sitter

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Node, Parser};

pub struct RubyExtractor;

impl Extractor for RubyExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_ruby::LANGUAGE.into()).is_err() {
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
    in_class: bool,
) {
    let kind = node.kind();

    if kind == "method" || kind == "singleton_method" {
        if let Some(info) = parse_method(node, content, lines, in_class || kind == "singleton_method") {
            functions.push(info);
        }
    } else if (kind == "class" || kind == "module")
        && let Some(info) = parse_class(node, content, lines)
    {
        functions.push(info);
    }

    let is_class_body = kind == "class" || kind == "module";

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse(child, content, lines, functions, in_class || is_class_body);
    }
}

fn parse_method(
    node: Node,
    content: &str,
    lines: &[&str],
    is_method: bool,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut params_str = String::new();

    if let Some(name_node) = node.child_by_field_name("name") {
        name = name_node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
    }
    
    if let Some(params_node) = node.child_by_field_name("parameters") {
        params_str = params_node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
    }

    if name.is_empty() || name == "?" || name == "?obj" {
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

    if let Some(name_node) = node.child_by_field_name("name") {
        name = name_node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
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
    fn test_extract_ruby_functions() {
        let content = "
module Utils
  def self.log(msg)
    puts msg
  end
end

class User
  def initialize(name)
    @name = name
  end
  
  def name
    @name
  end
end
";
        let extractor = RubyExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));
        
        assert_eq!(fns.len(), 5);
        
        let u = fns.iter().find(|f| f.name == "Utils").unwrap();
        assert!(u.is_class);
        
        let log = fns.iter().find(|f| f.name == "log").unwrap();
        assert!(log.is_method);
        assert_eq!(log.parameters, vec!["msg"]);
        
        let user = fns.iter().find(|f| f.name == "User").unwrap();
        assert!(user.is_class);
        
        let init = fns.iter().find(|f| f.name == "initialize").unwrap();
        assert!(init.is_method);
    }
}
