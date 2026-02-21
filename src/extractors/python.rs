// Author: kelexine (https://github.com/kelexine)
// extractors/python.rs — Python function/class extraction

use super::{Extractor, LineMap, estimate_complexity, parse_params};
use crate::models::FunctionInfo;
use once_cell::sync::Lazy;
use regex::Regex;

static RE_PY_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^(?P<indent>[ \t]*)(?P<async>async\s+)?def\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)\s*(?:->[^:]+)?:").unwrap()
});

static RE_PY_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^(?P<indent>[ \t]*)class\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)(?:\((?P<bases>[^)]*)\))?\s*:").unwrap()
});

static RE_PY_DECORATOR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^[ \t]*@([a-zA-Z_][a-zA-Z0-9_.]*)").unwrap());

pub struct PythonExtractor;

impl Extractor for PythonExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();

        for cap in RE_PY_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            let is_async = cap.name("async").is_some();
            let indent = cap.name("indent").map_or(0, |i| i.as_str().len());
            let is_method = params
                .first()
                .map(|p| p == "self" || p == "cls")
                .unwrap_or(false);
            let line_end = find_python_end(&lines, line_start, indent);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);
            let decorators = collect_python_decorators(content, m.start());
            let docstring = extract_py_docstring(block);

            functions.push(FunctionInfo {
                name,
                line_start,
                line_end,
                parameters: params,
                is_async,
                is_method,
                is_class: false,
                docstring,
                decorators,
                complexity,
            });
        }

        for cap in RE_PY_CLASS.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let bases = parse_params(cap.name("bases").map_or("", |b| b.as_str()));
            let indent = cap.name("indent").map_or(0, |i| i.as_str().len());
            let line_end = find_python_end(&lines, line_start, indent);
            functions.push(FunctionInfo {
                name,
                line_start,
                line_end,
                parameters: bases,
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

fn find_python_end(lines: &[&str], start_line: usize, base_indent: usize) -> usize {
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

fn collect_python_decorators(content: &str, fn_start: usize) -> Vec<String> {
    let before = &content[..fn_start];
    let last_blank = before.rfind("\n\n").unwrap_or(0);
    let segment = &before[last_blank..];
    RE_PY_DECORATOR
        .captures_iter(segment)
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .collect()
}

fn extract_py_docstring(block: &[&str]) -> Option<String> {
    for line in block.iter().skip(1).take(5) {
        let t = line.trim();
        if t.starts_with("\"\"\"") || t.starts_with("'''") {
            let quote = if t.starts_with("\"\"\"") {
                "\"\"\""
            } else {
                "'''"
            };
            let inner = &t[3..];
            if let Some(end) = inner.find(quote) {
                return Some(inner[..end].trim().to_string());
            }
            return Some(inner.trim().to_string());
        }
    }
    None
}
