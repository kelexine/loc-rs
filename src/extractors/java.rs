// Author: kelexine (https://github.com/kelexine)
// extractors/java.rs — Java/Kotlin/C# function extraction

use super::{Extractor, LineMap, estimate_complexity, find_closing_brace, parse_params};
use crate::models::FunctionInfo;
use once_cell::sync::Lazy;
use regex::Regex;

static RE_JAVA_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|internal|open|override|abstract|static|final|sealed|async|virtual|extern|suspend)\s+)*(?:\w+(?:<[^>]*>)?[*& \t]+)*(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)\s*(?:throws\s+\w+\s*)?(?:\{|=>)").unwrap()
});

pub struct JavaExtractor;

impl Extractor for JavaExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();
        const SKIP: &[&str] = &["if", "for", "while", "switch", "catch", "try", "else", "do"];

        for cap in RE_JAVA_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) {
                continue;
            }
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            let line_start = line_map.offset_to_line(m.start());
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            let line_end = find_closing_brace(&lines, line_start);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);
            functions.push(FunctionInfo {
                name,
                line_start,
                line_end,
                parameters: params,
                is_async: false,
                is_method: true,
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
    fn test_extract_java_functions() {
        let content = "
public class Main {
    public static void main(String[] args) {}
    private int calc(int a, int b) => a + b;
}
";
        let extractor = JavaExtractor;
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "main");
        assert_eq!(fns[1].name, "calc");
    }
}
