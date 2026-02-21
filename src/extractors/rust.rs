// Author: kelexine (https://github.com/kelexine)
// extractors/rust.rs — Rust function/struct extraction

use super::{Extractor, LineMap, estimate_complexity, find_closing_brace, parse_params};
use crate::models::FunctionInfo;
use once_cell::sync::Lazy;
use regex::Regex;

static RE_RUST_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?P<pub>pub(?:\([^)]+\))?\s+)?(?P<async>async\s+)?fn\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)")
        .expect("Rust fn regex")
});

static RE_RUST_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]+\))?\s+)?struct\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)")
        .expect("Rust struct regex")
});

pub struct RustExtractor;

impl Extractor for RustExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for cap in RE_RUST_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) {
                continue;
            }

            // Look back to see if it's annotated with #[test]
            let prefix = &content[..m.start()];
            let pre_trim = prefix.trim_end();
            if pre_trim.ends_with("]") && pre_trim.contains("#[test]")
                || pre_trim.contains("#[tokio::test]")
            {
                continue;
            }
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            let is_async = cap.name("async").is_some();
            let is_pub = cap.name("pub").is_some();
            let line_end = find_closing_brace(&lines, line_start);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);
            let is_method = content[..m.start()]
                .rfind("impl ")
                .map(|pos| !content[pos..m.start()].contains("\n\n"))
                .unwrap_or(false);

            functions.push(FunctionInfo {
                name,
                line_start,
                line_end,
                parameters: params,
                is_async,
                is_method,
                is_class: false,
                docstring: None,
                decorators: if is_pub { vec!["pub".into()] } else { vec![] },
                complexity,
            });
        }

        for cap in RE_RUST_STRUCT.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let line_end = find_closing_brace(&lines, line_start);
            functions.push(FunctionInfo {
                name,
                line_start,
                line_end,
                parameters: vec![],
                is_async: false,
                is_method: false,
                is_class: true,
                docstring: None,
                decorators: vec![],
                complexity: 1,
            });
        }

        functions.sort_by_key(|f| f.line_start);
        functions
    }
}
