// Author: kelexine (https://github.com/kelexine)
// extractors/go.rs — Go function extraction

use super::{Extractor, LineMap, estimate_complexity, find_closing_brace, parse_params};
use crate::models::FunctionInfo;
use once_cell::sync::Lazy;
use regex::Regex;

static RE_GO_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)^func\s+(?:\([^)]+\)\s+)?(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)",
    )
    .unwrap()
});

static RE_GO_RECV: Lazy<Regex> = Lazy::new(|| Regex::new(r"^func\s+\([^)]+\)").unwrap());

pub struct GoExtractor;

impl Extractor for GoExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();

        for cap in RE_GO_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            let is_method = RE_GO_RECV.is_match(&content[m.start()..m.end()]);
            let line_end = find_closing_brace(&lines, line_start);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);
            functions.push(FunctionInfo {
                name,
                line_start,
                line_end,
                parameters: params,
                is_async: false,
                is_method,
                is_class: false,
                docstring: None,
                decorators: vec![],
                complexity,
            });
        }

        functions
    }
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
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "Hello");
        assert!(!fns[0].is_method);
        assert_eq!(fns[1].name, "Get");
        assert!(fns[1].is_method);
        assert_eq!(fns[1].parameters, vec!["id", "int"]);
    }
}
