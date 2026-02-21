// Author: kelexine (https://github.com/kelexine)
// extractors/nim.rs — Nim function extraction

use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::FunctionInfo;
use super::{Extractor, LineMap, estimate_complexity, parse_params};

static RE_NIM_FN: Lazy<Regex> = Lazy::new(|| {
    // Matches: proc name*(args) or func name[T](args)
    Regex::new(r"(?m)^(?P<indent>[ \t]*)(?:proc|func|method|iterator|macro|template)\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*(?:\*)?\s*(?:\[[^\]]*\])?\s*\((?P<params>[^)]*)\)").unwrap()
});

pub struct NimExtractor;

impl Extractor for NimExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();

        for cap in RE_NIM_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            
            let indent = cap.name("indent").map_or(0, |i| i.as_str().len());
            let is_method = content[m.start()..m.end()].starts_with("method");
            
            let line_end = find_indentation_end(&lines, line_start, indent);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);

            // Nim's `*` indicates public export, treated as a decorator here
            let is_exported = content[m.start()..m.end()].contains('*');
            let decorators = if is_exported { vec!["public(*)".into()] } else { vec![] };

            functions.push(FunctionInfo {
                name, line_start, line_end, parameters: params,
                is_async: false, is_method, is_class: false,
                docstring: None, decorators, complexity,
            });
        }

        functions
    }
}

/// Tracks indentation changes to resolve block endings (Python/Nim style)
fn find_indentation_end(lines: &[&str], start_line: usize, base_indent: usize) -> usize {
    for (i, line) in lines[start_line..].iter().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent <= base_indent {
            return start_line + i;
        }
    }
    lines.len()
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
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 3);
        assert_eq!(fns[0].name, "hello");
        assert!(fns[0].decorators.contains(&"public(*)".to_string()));
        assert_eq!(fns[1].name, "add");
        assert_eq!(fns[2].name, "draw");
        assert!(fns[2].is_method);
    }
}