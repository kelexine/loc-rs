// Author: kelexine (https://github.com/kelexine)
// extractors/swift.rs — Swift function/class extraction

use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::FunctionInfo;
use super::{Extractor, LineMap, find_closing_brace, parse_params, estimate_complexity};

static RE_SWIFT_FN: Lazy<Regex> = Lazy::new(|| {
    // Matches: [modifiers] func name<Generics>(args) [async] [throws]
    Regex::new(r"(?m)^[ \t]*(?:(?:public|private|internal|fileprivate|open|mutating|nonmutating|class|static|override|final)\s+)*func\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)\s*(?:async\s+)?(?:throws\s+)?").unwrap()
});

static RE_SWIFT_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:(?:public|private|internal|fileprivate|open|final)\s+)*(?:class|struct|enum|protocol)\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)").unwrap()
});

pub struct SwiftExtractor;

impl Extractor for SwiftExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for cap in RE_SWIFT_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) { continue; }
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let line_start = line_map.offset_to_line(m.start());
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
            
            // Heuristic: If it has mutating/override/class/static, it's a method
            let is_method = content[m.start()..m.end()].contains("mutating ") || 
                            content[m.start()..m.end()].contains("override ") ||
                            content[m.start()..m.end()].contains("static ") ||
                            content[m.start()..m.end()].contains("class func");

            let is_async = content[m.start()..m.end()].contains("async");
            let line_end = find_closing_brace(&lines, line_start);
            let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
            let complexity = estimate_complexity(block);

            functions.push(FunctionInfo {
                name, line_start, line_end, parameters: params,
                is_async, is_method, is_class: false,
                docstring: None, decorators: vec![], complexity,
            });
        }

        for cap in RE_SWIFT_CLASS.captures_iter(content) {
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
    fn test_extract_swift_functions() {
        let content = "
import Foundation
func hello() {}
public func fetchData() async -> Data? { return nil }
class Service {
    override func start() {}
}
struct Point {
    var x, y: Double
    mutating func moveBy(x deltaX: Double, y deltaY: Double) {}
}
";
        let extractor = SwiftExtractor;
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 6);
        assert_eq!(fns[0].name, "hello");
        assert_eq!(fns[1].name, "fetchData");
        assert!(fns[1].is_async);
        assert_eq!(fns[2].name, "Service");
        assert!(fns[2].is_class);
        assert_eq!(fns[3].name, "start");
        assert!(fns[3].is_method);
        assert_eq!(fns[4].name, "Point");
        assert!(fns[4].is_class);
        assert_eq!(fns[5].name, "moveBy");
    }
}