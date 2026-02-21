// Author: kelexine (https://github.com/kelexine)
// extractor.rs — Regex-based function/class extraction for multiple languages

use std::path::Path;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::models::FunctionInfo;

// ─────────────────────────────────────────────────────────────────────────────
// Pre-compiled regex patterns (all named groups use full words for clarity)
// ─────────────────────────────────────────────────────────────────────────────

static RE_RUST_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?P<pub>pub(?:\([^)]+\))?\s+)?(?P<async>async\s+)?fn\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)")
        .expect("Rust fn regex")
});

static RE_RUST_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]+\))?\s+)?struct\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)")
        .expect("Rust struct regex")
});

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

static RE_PY_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^(?P<indent>[ \t]*)(?P<async>async\s+)?def\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)\s*(?:->[^:]+)?:").unwrap()
});

static RE_PY_CLASS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^(?P<indent>[ \t]*)class\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)(?:\((?P<bases>[^)]*)\))?\s*:").unwrap()
});

static RE_PY_DECORATOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*@([a-zA-Z_][a-zA-Z0-9_.]*)").unwrap()
});

static RE_GO_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^func\s+(?:\([^)]+\)\s+)?(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)").unwrap()
});

static RE_GO_RECV: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^func\s+\([^)]+\)").unwrap()
});

static RE_C_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^(?![ \t]*#)(?![ \t]*//)[ \t]*(?:static\s+|inline\s+|virtual\s+|explicit\s+)?(?:\w[\w*& \t]+\s+)+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)\s*(?:const\s*)?(?:noexcept\s*)?(?:override\s*)?\{").unwrap()
});

static RE_JAVA_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|internal|open|override|abstract|static|final|sealed|async|virtual|extern|suspend)\s+)*(?:\w+(?:<[^>]*>)?[*& \t]+)*(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)\s*\((?P<params>[^)]*)\)\s*(?:throws\s+\w+\s*)?(?:\{|=>)").unwrap()
});

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────

fn offset_to_line(content: &str, offset: usize) -> usize {
    content[..offset].chars().filter(|&c| c == '\n').count() + 1
}

fn find_closing_brace(lines: &[&str], start_line: usize) -> usize {
    let mut depth: i32 = 0;
    let mut started = false;
    for (i, line) in lines[start_line.saturating_sub(1)..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => { depth += 1; started = true; }
                '}' => {
                    depth -= 1;
                    if started && depth <= 0 {
                        return start_line + i;
                    }
                }
                _ => {}
            }
        }
    }
    (start_line + 80).min(lines.len())
}

fn parse_params(raw: &str) -> Vec<String> {
    raw.split(',')
       .filter_map(|p| {
           let p = p.trim();
           if p.is_empty() || p == "void" { return None; }
           let name = p.split_whitespace()
               .last()
               .unwrap_or(p)
               .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
           if name.is_empty() { None } else { Some(name.to_string()) }
       })
       .collect()
}

fn estimate_complexity(block: &[&str]) -> u32 {
    const KEYWORDS: &[&str] = &[
        "if ", "else if", "elif ", " while ", " for ", " match ", "case ",
        " catch ", " except ", "&&", "||", "? ",
    ];
    let mut cc = 1u32;
    for line in block {
        for kw in KEYWORDS {
            cc += line.matches(kw).count() as u32;
        }
    }
    cc
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-language extractors
// ─────────────────────────────────────────────────────────────────────────────

pub fn extract_rust(content: &str) -> Vec<FunctionInfo> {
    let lines: Vec<&str> = content.lines().collect();
    let mut functions = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in RE_RUST_FN.captures_iter(content) {
        let m = cap.get(0).unwrap();
        if !seen.insert(m.start()) { continue; }
        let line_start = offset_to_line(content, m.start());
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
            name, line_start, line_end, parameters: params,
            is_async, is_method, is_class: false, docstring: None,
            decorators: if is_pub { vec!["pub".into()] } else { vec![] },
            complexity,
        });
    }

    for cap in RE_RUST_STRUCT.captures_iter(content) {
        let m = cap.get(0).unwrap();
        let line_start = offset_to_line(content, m.start());
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

pub fn extract_python(content: &str) -> Vec<FunctionInfo> {
    let lines: Vec<&str> = content.lines().collect();
    let mut functions = Vec::new();

    for cap in RE_PY_FN.captures_iter(content) {
        let m = cap.get(0).unwrap();
        let line_start = offset_to_line(content, m.start());
        let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
        let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
        let is_async = cap.name("async").is_some();
        let indent = cap.name("indent").map_or(0, |i| i.as_str().len());
        let is_method = params.first().map(|p| p == "self" || p == "cls").unwrap_or(false);
        let line_end = find_python_end(&lines, line_start, indent);
        let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
        let complexity = estimate_complexity(block);
        let decorators = collect_python_decorators(content, m.start());
        let docstring = extract_py_docstring(block);

        functions.push(FunctionInfo {
            name, line_start, line_end, parameters: params,
            is_async, is_method, is_class: false, docstring, decorators, complexity,
        });
    }

    for cap in RE_PY_CLASS.captures_iter(content) {
        let m = cap.get(0).unwrap();
        let line_start = offset_to_line(content, m.start());
        let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
        let bases = parse_params(cap.name("bases").map_or("", |b| b.as_str()));
        let indent = cap.name("indent").map_or(0, |i| i.as_str().len());
        let line_end = find_python_end(&lines, line_start, indent);
        functions.push(FunctionInfo {
            name, line_start, line_end, parameters: bases,
            is_async: false, is_method: false, is_class: true,
            docstring: None, decorators: vec![], complexity: 1,
        });
    }

    functions.sort_by_key(|f| f.line_start);
    functions
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
    RE_PY_DECORATOR.captures_iter(segment)
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .collect()
}

fn extract_py_docstring(block: &[&str]) -> Option<String> {
    for line in block.iter().skip(1).take(5) {
        let t = line.trim();
        if t.starts_with("\"\"\"") || t.starts_with("'''") {
            let quote = if t.starts_with("\"\"\"") { "\"\"\"" } else { "'''" };
            let inner = &t[3..];
            if let Some(end) = inner.find(quote) {
                return Some(inner[..end].trim().to_string());
            }
            return Some(inner.trim().to_string());
        }
    }
    None
}

pub fn extract_javascript(content: &str) -> Vec<FunctionInfo> {
    let lines: Vec<&str> = content.lines().collect();
    let mut functions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    const SKIP: &[&str] = &["if", "for", "while", "switch", "catch", "constructor", "return"];

    for re in RE_JS_FN.iter() {
        for cap in re.captures_iter(content) {
            let m = cap.get(0).unwrap();
            if !seen.insert(m.start()) { continue; }
            let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
            if SKIP.contains(&name.as_str()) { continue; }
            let line_start = offset_to_line(content, m.start());
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
        let line_start = offset_to_line(content, m.start());
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

pub fn extract_go(content: &str) -> Vec<FunctionInfo> {
    let lines: Vec<&str> = content.lines().collect();
    let mut functions = Vec::new();

    for cap in RE_GO_FN.captures_iter(content) {
        let m = cap.get(0).unwrap();
        let line_start = offset_to_line(content, m.start());
        let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
        let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
        let is_method = RE_GO_RECV.is_match(&content[m.start()..m.end()]);
        let line_end = find_closing_brace(&lines, line_start);
        let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
        let complexity = estimate_complexity(block);
        functions.push(FunctionInfo {
            name, line_start, line_end, parameters: params,
            is_async: false, is_method, is_class: false,
            docstring: None, decorators: vec![], complexity,
        });
    }

    functions
}

pub fn extract_c_cpp(content: &str) -> Vec<FunctionInfo> {
    let lines: Vec<&str> = content.lines().collect();
    let mut functions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    const SKIP: &[&str] = &["if", "for", "while", "switch", "do", "return"];

    for cap in RE_C_FN.captures_iter(content) {
        let m = cap.get(0).unwrap();
        if !seen.insert(m.start()) { continue; }
        let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
        if SKIP.contains(&name.as_str()) { continue; }
        let line_start = offset_to_line(content, m.start());
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

pub fn extract_java(content: &str) -> Vec<FunctionInfo> {
    let lines: Vec<&str> = content.lines().collect();
    let mut functions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    const SKIP: &[&str] = &["if", "for", "while", "switch", "catch", "try", "else", "do"];

    for cap in RE_JAVA_FN.captures_iter(content) {
        let m = cap.get(0).unwrap();
        if !seen.insert(m.start()) { continue; }
        let name = cap.name("name").map_or("?", |n| n.as_str()).to_string();
        if SKIP.contains(&name.as_str()) { continue; }
        let line_start = offset_to_line(content, m.start());
        let params = parse_params(cap.name("params").map_or("", |p| p.as_str()));
        let line_end = find_closing_brace(&lines, line_start);
        let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
        let complexity = estimate_complexity(block);
        functions.push(FunctionInfo {
            name, line_start, line_end, parameters: params,
            is_async: false, is_method: true, is_class: false,
            docstring: None, decorators: vec![], complexity,
        });
    }

    functions
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispatch
// ─────────────────────────────────────────────────────────────────────────────

pub fn extract_functions(path: &Path, content: &str) -> Vec<FunctionInfo> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    match ext.as_str() {
        ".rs" => extract_rust(content),
        ".py" | ".pyw" | ".pyi" => extract_python(content),
        ".js" | ".mjs" | ".cjs" | ".ts" | ".tsx" | ".jsx" => extract_javascript(content),
        ".go" => extract_go(content),
        ".c" | ".h" | ".cpp" | ".cc" | ".cxx" | ".hpp" | ".hxx" => extract_c_cpp(content),
        ".java" | ".kt" | ".kts" | ".cs" | ".scala" => extract_java(content),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_basic() {
        let code = r#"
            pub fn hello() {}
            fn internal(a: i32) -> i32 { a + 1 }
            async fn fetch() {}
            struct Data {}
        "#;
        let fns = extract_rust(code);
        assert_eq!(fns.len(), 4);
        assert_eq!(fns[0].name, "hello");
        assert!(fns[0].decorators.contains(&"pub".to_string()));
        assert_eq!(fns[1].name, "internal");
        assert_eq!(fns[2].name, "fetch");
        assert!(fns[2].is_async);
        assert!(fns[3].is_class); // struct is treated as class
    }

    #[test]
    fn test_extract_python_basic() {
        let code = r#"
@deco
def hello():
    """Docstring"""
    pass

async def fetch(url):
    pass

class MyClass:
    def method(self):
        pass
        "#;
        let fns = extract_python(code);
        // def hello, async def fetch, class MyClass, def method
        assert_eq!(fns.len(), 4);
        assert_eq!(fns[0].name, "hello");
        assert!(fns[0].decorators.contains(&"deco".to_string()));
        assert_eq!(fns[0].docstring, Some("Docstring".to_string()));
        assert_eq!(fns[1].name, "fetch");
        assert!(fns[1].is_async);
        assert_eq!(fns[2].name, "MyClass");
        assert!(fns[2].is_class);
        assert_eq!(fns[3].name, "method");
        assert!(fns[3].is_method);
    }

    #[test]
    fn test_complexity_estimation() {
        let block = vec![
            "if x > 0 {",
            "  for i in 0..10 {",
            "    if true && false { }",
            "  }",
            "} else if y {",
            "}"
        ];
        // 1 (base) + 1 (if) + 1 (for) + 1 (if) + 1 (&&) + 2 (else if & if ) = 7
        assert_eq!(estimate_complexity(&block), 7);
    }

    #[test]
    fn test_offset_to_line() {
        let content = "line1\nline2\nline3";
        assert_eq!(offset_to_line(content, 0), 1);
        assert_eq!(offset_to_line(content, 6), 2);
        assert_eq!(offset_to_line(content, 12), 3);
    }
}
