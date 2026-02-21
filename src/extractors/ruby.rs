// Author: kelexine (https://github.com/kelexine)
// extractors/ruby.rs — Ruby function/class extraction

use super::{Extractor, LineMap, estimate_complexity, parse_params};
use crate::models::FunctionInfo;
use once_cell::sync::Lazy;
use regex::Regex;

static RE_RUBY_FN: Lazy<Regex> = Lazy::new(|| {
    // Matches: def name(args) or def self.name
    Regex::new(r"(?m)^[ \t]*def\s+(?:self\.)?(?P<name>[a-zA-Z_][a-zA-Z0-9_!?=]*)(?:\s*\((?P<params>[^)]*)\))?").unwrap()
});

static RE_RUBY_CLASS: Lazy<Regex> = Lazy::new(|| {
    // Matches: class Name < Base or module Name
    Regex::new(r"(?m)^[ \t]*(?:class|module)\s+(?P<name>[A-Z][a-zA-Z0-9_]*)(?:\s*<\s*[A-Z][a-zA-Z0-9_:]*)?").unwrap()
});

pub struct RubyExtractor;

impl Extractor for RubyExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let lines: Vec<&str> = content.lines().collect();
        let line_map = LineMap::new(content);
        let mut functions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for cap in RE_RUBY_FN.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) {
                continue;
            }
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let line_start = line_map.offset_to_line(m.start());
            let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));

            let is_method = content[m.start()..m.end()].contains("self.");

            let line_end = find_ruby_end(&lines, line_start);
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

        for cap in RE_RUBY_CLASS.captures_iter(content) {
            let m = cap.get(0).unwrap();
            let line_start = line_map.offset_to_line(m.start());
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            let line_end = find_ruby_end(&lines, line_start);

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

/// Advanced depth tracker for Ruby's `end` keyword blocks
fn find_ruby_end(lines: &[&str], start_line: usize) -> usize {
    let mut depth = 0;
    for (i, line) in lines[start_line.saturating_sub(1)..].iter().enumerate() {
        let t = line.trim();

        // Skip comments entirely
        if t.starts_with('#') {
            continue;
        }

        // Block openers
        let openers = [
            "def ", "class ", "module ", "if ", "unless ", "while ", "for ", "case ", "begin",
        ];
        if openers.iter().any(|&p| t.starts_with(p)) || t.ends_with(" do") || t == "do" {
            depth += 1;
        }

        // Block closers
        if t == "end" || t.starts_with("end ") || t.ends_with(" end") {
            depth -= 1;
            if depth <= 0 {
                return start_line + i;
            }
        }
    }
    lines.len()
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
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 5);
        assert_eq!(fns[0].name, "Utils");
        assert!(fns[0].is_class);
        assert_eq!(fns[1].name, "log");
        assert!(fns[1].is_method);
        assert_eq!(fns[2].name, "User");
        assert!(fns[2].is_class);
        assert_eq!(fns[3].name, "initialize");
    }
}
