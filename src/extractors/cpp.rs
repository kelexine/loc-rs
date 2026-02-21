// Author: kelexine (https://github.com/kelexine)
// extractors/cpp.rs — C/C++ function extraction

use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::FunctionInfo;
use super::{Extractor, LineMap, find_closing_brace, parse_params, estimate_complexity};

static RE_C_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^(?![ \t]*#)(?![ \t]*//)[ \t]*(?:static\s+|inline\s+|virtual\s+|explicit\s+)?(?:\w[\w*& \t]+\s+)+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)\s*(?:const\s*)?(?:noexcept\s*)?(?:override\s*)?\{").unwrap()
});

pub struct CppExtractor;

impl Extractor for CppExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        const SKIP: &[&str] = &["if", "for", "while", "switch", "do", "return"];

        for cap in RE_C_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) { continue; }
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            if SKIP.contains(&name.as_str()) { continue; }
            let line_start = line_map.offset_to_line(m.start());
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            let line_end = find_closing_brace(&lines, line_start);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);
            functions.push(FunctionInfo {
                name, line_start, line_end, parameters: params,
                is_async: false, is_method: false, is_class: false,
                docstring: None, decorators: vec![], complexity,
            });
        }

        functions
    }
}
