// Author: kelexine (https://github.com/kelexine)
// extractors/mod.rs — Trait definition and shared utilities for extractors

pub mod cpp;
pub mod go;
pub mod java;
pub mod javascript;
pub mod nim;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;

use crate::models::FunctionInfo;
use std::path::Path;

pub struct LineMap {
    offsets: Vec<usize>,
}

impl LineMap {
    pub fn new(content: &str) -> Self {
        let mut offsets = vec![0];
        for (i, b) in content.bytes().enumerate() {
            if b == b'\n' {
                offsets.push(i + 1);
            }
        }
        Self { offsets }
    }

    pub fn offset_to_line(&self, offset: usize) -> usize {
        match self.offsets.binary_search(&offset) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        }
    }
}

pub trait Extractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo>;
}

pub fn get_extractor(path: &Path) -> Option<Box<dyn Extractor>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    match ext.as_str() {
        ".rs" => Some(Box::new(rust::RustExtractor)),
        ".py" | ".pyw" | ".pyi" => Some(Box::new(python::PythonExtractor)),
        ".js" | ".mjs" | ".cjs" | ".ts" | ".tsx" | ".jsx" => {
            Some(Box::new(javascript::JavascriptExtractor))
        }
        ".go" => Some(Box::new(go::GoExtractor)),
        ".c" | ".h" | ".cpp" | ".cc" | ".cxx" | ".hpp" | ".hxx" => {
            Some(Box::new(cpp::CppExtractor))
        }
        ".java" | ".kt" | ".kts" | ".cs" | ".scala" => Some(Box::new(java::JavaExtractor)),
        ".php" | ".php3" | ".php4" | ".php5" | ".phtml" => Some(Box::new(php::PhpExtractor)),
        ".swift" => Some(Box::new(swift::SwiftExtractor)),
        ".rb" | ".rake" | ".gemspec" => Some(Box::new(ruby::RubyExtractor)),
        ".nim" | ".nims" => Some(Box::new(nim::NimExtractor)),
        _ => None,
    }
}

pub fn find_closing_brace(lines: &[&str], start_line: usize) -> usize {
    let mut depth: i32 = 0;
    let mut started = false;
    let mut in_string = false;
    let mut string_char = ' ';

    for (i, line) in lines[start_line.saturating_sub(1)..].iter().enumerate() {
        let mut chars = line.chars().peekable();
        let mut escaped_eol = false;
        while let Some(ch) = chars.next() {
            if in_string {
                if ch == '\\' {
                    if chars.next().is_none() {
                        escaped_eol = true;
                    }
                } else if ch == string_char {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' | '`' => {
                    in_string = true;
                    string_char = ch;
                }
                '\'' => {
                    let mut dist = 0;
                    let mut found = false;
                    for c in chars.clone() {
                        dist += 1;
                        if c == '\'' {
                            found = true;
                            break;
                        }
                    }
                    if found && dist < 12 {
                        in_string = true;
                        string_char = ch;
                    }
                }
                '/' if chars.peek() == Some(&'/') => break, // Line comment
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => {
                    depth -= 1;
                    if started && depth <= 0 {
                        return start_line + i;
                    }
                }
                _ => {}
            }
        }

        // Reset string state if not explicitly continued or a backtick string
        if in_string && !escaped_eol && string_char != '`' {
            in_string = false;
        }
    }
    (start_line + 80).min(lines.len())
}

pub fn parse_params(raw: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in raw.chars() {
        match ch {
            '<' | '[' | '(' => {
                depth += 1;
                current.push(ch);
            }
            '>' | ']' | ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() && trimmed != "void" {
                    params.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() && trimmed != "void" {
        params.push(trimmed);
    }
    params
}

pub fn estimate_complexity(block: &[&str]) -> u32 {
    const KEYWORDS: &[&str] = &[
        "if ", "else if", "elif ", " while ", " for ", " match ", "case ", " catch ", " except ",
        "&&", "||", "? ",
    ];
    let mut cc = 1u32;
    for line in block {
        for kw in KEYWORDS {
            cc += line.matches(kw).count() as u32;
        }
    }
    cc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_estimation() {
        let block = vec![
            "if x > 0 {",
            "  for i in 0..10 {",
            "    if true && false { }",
            "  }",
            "} else if y {",
            "}",
        ];
        // 1 (base) + 1 (if) + 1 (for) + 1 (if) + 1 (&&) + 2 (else if & if ) = 7
        assert_eq!(estimate_complexity(&block), 7);
    }

    #[test]
    fn test_find_closing_brace_robust() {
        let lines = vec![
            "fn hello() {",
            "  let x = \"}\"; // false brace",
            "  if true {",
            "    println!(\"{}\", x);",
            "  }",
            "}",
        ];
        // Should end at line 6
        assert_eq!(find_closing_brace(&lines, 1), 6);
    }

    #[test]
    fn test_line_map() {
        let content = "line1\nline2\nline3";
        let map = LineMap::new(content);
        assert_eq!(map.offset_to_line(0), 1);
        assert_eq!(map.offset_to_line(6), 2);
        assert_eq!(map.offset_to_line(12), 3);
    }
}
