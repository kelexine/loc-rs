// Author: kelexine (https://github.com/kelexine)
// extractors/php.rs — PHP function/class extraction

use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::FunctionInfo;
use super::{Extractor, LineMap, find_closing_brace, parse_params, estimate_complexity};

static RE_PHP_FN: Lazy<Regex> = Lazy::new(|| {
    // Matches: [modifiers] function name(args)
    Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|static|final|abstract)\s+)*function\s+(?P<name>[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*)\s*\((?P<params>[^)]*)\)").unwrap()
});

static RE_PHP_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:(?:abstract|final)\s+)?(?:class|interface|trait)\s+(?P<name>[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*)").unwrap()
});

pub struct PhpExtractor;

impl Extractor for PhpExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Extract Functions & Methods
        for cap in RE_PHP_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) { continue; }
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let line_start = line_map.offset_to_line(m.start());
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            
            let is_method = content[m.start()..m.end()].contains("public") || 
                            content[m.start()..m.end()].contains("private") || 
                            content[m.start()..m.end()].contains("protected");

            let line_end = find_closing_brace(&lines, line_start);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);

            functions.push(FunctionInfo {
                name, line_start, line_end, parameters: params,
                is_async: false, is_method, is_class: false,
                docstring: None, decorators: vec![], complexity,
            });
        }

        // Extract Classes/Traits/Interfaces
        for cap in RE_PHP_CLASS.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let line_end = find_closing_brace(&lines, line_start);
            functions.push(FunctionInfo {
                name, line_start, line_end, parameters: vec![],
                is_async: false, is_method: false, is_class: true,
                docstring: None, decorators: vec![], complexity: 1,
            });
        }

        functions.sort_by_key(|f| f.line_start);
        functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_php() {
        let content = "
<?php
class User {
    public function getName($id) {
        return 'kelexine';
    }
}
function helper() {}
";
        let fns = PhpExtractor.extract(content);
        assert_eq!(fns.len(), 3);
        assert!(fns.iter().any(|f| f.name == "User" && f.is_class));
        assert!(fns.iter().any(|f| f.name == "getName" && f.is_method));
        assert!(fns.iter().any(|f| f.name == "helper" && !f.is_method));
    }
}