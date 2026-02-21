// Author: kelexine (https://github.com/kelexine)
// extractors/javascript.rs — JavaScript/TypeScript function/class extraction

use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::FunctionInfo;
use super::{Extractor, LineMap, estimate_complexity, find_closing_brace, parse_params};

static RE_JS_FN: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?m)^[ \t]*(?:export\s+(?:default\s+)?)?(?:async\s+)?function\s+(?P<name>[a-zA-Z_$][a-zA-Z0-9_$]*)\s*\((?P<params>[^)]*)\)").unwrap(),
        Regex::new(r"(?m)^[ \t]*(?:export\s+)?(?:const|let|var)\s+(?P<name>[a-zA-Z_$][a-zA-Z0-9_$]*)\s*=\s*(?:async\s+)?(?:function\s*)?\((?P<params>[^)]*)\)\s*(?:=>)?").unwrap(),
        Regex::new(r"(?m)^[ \t]+(?:async\s+)?(?:static\s+)?(?:get\s+|set\s+)?(?P<name>[a-zA-Z_$][a-zA-Z0-9_$]*)\s*\((?P<params>[^)]*)\)\s*\{").unwrap(),
    ]
});

static RE_JS_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:export\s+(?:default\s+)?)?class\s+(?P<name>[a-zA-Z_$][a-zA-Z0-9_$]*)").unwrap()
});

pub struct JavascriptExtractor;

impl Extractor for JavascriptExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        const SKIP: &[&str] = &["if", "for", "while", "switch", "catch", "constructor", "return"];

        for re in RE_JS_FN.iter() {
            for cap in re.captures_iter(content) {
                let m = cap.get(0).unwrap();
                if !seen.insert(m.start()) { continue; }
                let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
                if SKIP.contains(&name.as_str()) { continue; }
                let line_start = line_map.offset_to_line(m.start());
                let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
                let is_async = content[m.start()..m.end()].contains("async ");
                let line_end = find_closing_brace(&lines, line_start);
                let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
                let complexity = estimate_complexity(block);
                functions.push(FunctionInfo {
                    name, line_start, line_end, parameters: params,
                    is_async, is_method: false, is_class: false,
                    docstring: None, decorators: vec![], complexity,
                });
            }
        }

        for cap in RE_JS_CLASS.captures_iter(content) {
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
