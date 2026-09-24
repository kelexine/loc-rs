// Author: kelexine (https://github.com/kelexine)
// counter/lines.rs — High-performance byte-level line counting and stateful comment tracking

use std::path::Path;

use crate::language::CommentSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    None,
    Single,
    Double,
    Backtick,
    TripleDouble,
    TripleSingle,
}

/// Analyze text lines using a high-performance zero-allocation byte-level tokenizer.
pub fn analyze_content_with_spec(
    content: &str,
    spec: Option<&CommentSpec>,
) -> (usize, usize, usize, usize) {
    let mut total = 0usize;
    let mut code = 0usize;
    let mut comment = 0usize;
    let mut blank = 0usize;

    let mut comment_depth = 0usize;
    let mut quote_state = QuoteState::None;

    if spec.is_none() {
        for line in content.lines() {
            total += 1;
            let bytes = line.as_bytes();
            if bytes.iter().all(|b| b.is_ascii_whitespace()) {
                blank += 1;
            } else {
                code += 1;
            }
        }
        return (total, code, 0, blank);
    }

    // Check if the multi-line comment delimiters match Python triple quotes
    let is_python_docstring_spec = spec
        .and_then(|s| s.multi)
        .map(|(s, e)| s == "\"\"\"" && e == "\"\"\"")
        .unwrap_or(false);

    let single_bytes = spec.and_then(|s| s.single).map(|s| s.as_bytes());
    let multi_start_bytes = spec.and_then(|s| s.multi).map(|(s, _)| s.as_bytes());
    let multi_end_bytes = spec.and_then(|s| s.multi).map(|(_, e)| e.as_bytes());
    let supports_nesting = spec.map(|s| s.supports_nesting).unwrap_or(false);

    // Fast trigger table for lines outside comments and strings
    let mut trigger = [false; 256];
    trigger[b'"' as usize] = true;
    trigger[b'\'' as usize] = true;
    trigger[b'`' as usize] = true;
    if let Some(s) = single_bytes {
        trigger[s[0] as usize] = true;
    }
    if let Some(m) = multi_start_bytes {
        trigger[m[0] as usize] = true;
    }

    for line in content.lines() {
        total += 1;
        let bytes = line.as_bytes();
        let len = bytes.len();

        // Fast-path: find first non-whitespace byte
        let mut start_idx = 0;
        while start_idx < len && bytes[start_idx].is_ascii_whitespace() {
            start_idx += 1;
        }

        // Entirely whitespace line
        if start_idx == len {
            if comment_depth > 0
                || quote_state == QuoteState::TripleDouble
                || quote_state == QuoteState::TripleSingle
            {
                comment += 1;
            } else {
                blank += 1;
            }
            continue;
        }

        // Fast-path for lines outside comments and multi-line strings
        if comment_depth == 0 && quote_state == QuoteState::None {
            // Whole-line single comment (e.g. "//" or "#")
            if let Some(s_bytes) = single_bytes
                && bytes[start_idx..].starts_with(s_bytes)
            {
                comment += 1;
                continue;
            }

            // Pure code line: contains no comment or string delimiter triggers
            if !bytes[start_idx..].iter().any(|&b| trigger[b as usize]) {
                code += 1;
                continue;
            }
        }

        let mut has_code = false;
        let mut has_comment = false;

        // If we entered this line inside a comment, mark it
        if comment_depth > 0
            || quote_state == QuoteState::TripleDouble
            || quote_state == QuoteState::TripleSingle
        {
            has_comment = true;
        }

        let mut i = start_idx;
        while i < len {
            let b = bytes[i];

            // ── In Block Comment ─────────────────────────────────────────────────
            if comment_depth > 0 {
                has_comment = true;
                if let (Some(s_bytes), Some(e_bytes)) = (multi_start_bytes, multi_end_bytes) {
                    if supports_nesting && bytes[i..].starts_with(s_bytes) {
                        comment_depth += 1;
                        i += s_bytes.len();
                        continue;
                    }

                    if bytes[i..].starts_with(e_bytes) {
                        comment_depth = comment_depth.saturating_sub(1);
                        i += e_bytes.len();
                        continue;
                    }
                }
                i += 1;
                continue;
            }

            // ── In Triple-Quote String / Docstring (Python style) ────────────────
            if quote_state == QuoteState::TripleDouble {
                has_comment = true;
                if bytes[i..].starts_with(b"\"\"\"") {
                    quote_state = QuoteState::None;
                    i += 3;
                } else {
                    i += 1;
                }
                continue;
            }

            if quote_state == QuoteState::TripleSingle {
                has_comment = true;
                if bytes[i..].starts_with(b"'''") {
                    quote_state = QuoteState::None;
                    i += 3;
                } else {
                    i += 1;
                }
                continue;
            }

            // ── In Standard String Literal ───────────────────────────────────────
            match quote_state {
                QuoteState::Single => {
                    has_code = true;
                    if b == b'\\' {
                        i += 2; // Skip backslash + escaped byte
                    } else if b == b'\'' {
                        quote_state = QuoteState::None;
                        i += 1;
                    } else {
                        i += 1;
                    }
                    continue;
                }
                QuoteState::Double => {
                    has_code = true;
                    if b == b'\\' {
                        i += 2; // Skip backslash + escaped byte
                    } else if b == b'"' {
                        quote_state = QuoteState::None;
                        i += 1;
                    } else {
                        i += 1;
                    }
                    continue;
                }
                QuoteState::Backtick => {
                    has_code = true;
                    if b == b'\\' {
                        i += 2; // Skip backslash + escaped byte
                    } else if b == b'`' {
                        quote_state = QuoteState::None;
                        i += 1;
                    } else {
                        i += 1;
                    }
                    continue;
                }
                _ => {}
            }

            // ── Normal Code Context ──────────────────────────────────────────────
            // Check for Python triple quotes
            if is_python_docstring_spec {
                if bytes[i..].starts_with(b"\"\"\"") {
                    has_comment = true;
                    quote_state = QuoteState::TripleDouble;
                    i += 3;
                    continue;
                }
                if bytes[i..].starts_with(b"'''") {
                    has_comment = true;
                    quote_state = QuoteState::TripleSingle;
                    i += 3;
                    continue;
                }
            }

            // Check for multi-line comment start
            if let Some(s_bytes) = multi_start_bytes
                && bytes[i..].starts_with(s_bytes)
            {
                has_comment = true;
                comment_depth += 1;
                i += s_bytes.len();
                continue;
            }

            // Check for single-line comment start
            if let Some(s_bytes) = single_bytes
                && bytes[i..].starts_with(s_bytes)
            {
                has_comment = true;
                // Everything else on this line is commented out
                break;
            }

            // String opening
            if b == b'"' {
                has_code = true;
                quote_state = QuoteState::Double;
                i += 1;
                continue;
            }
            if b == b'\'' {
                has_code = true;
                // If it's a character literal like 'a' or '\n'
                if i + 2 < len && bytes[i + 2] == b'\'' && bytes[i + 1] != b'\\' {
                    i += 3;
                } else if i + 3 < len && bytes[i + 1] == b'\\' && bytes[i + 3] == b'\'' {
                    i += 4;
                } else {
                    quote_state = QuoteState::Single;
                    i += 1;
                }
                continue;
            }
            if b == b'`' {
                has_code = true;
                quote_state = QuoteState::Backtick;
                i += 1;
                continue;
            }

            // Non-whitespace character is code
            if !b.is_ascii_whitespace() {
                has_code = true;
            }
            i += 1;
        }

        // Reset single-character/single-quoted strings at end-of-line
        if quote_state == QuoteState::Single {
            quote_state = QuoteState::None;
        }

        // Tally line
        if has_code {
            code += 1;
        } else if has_comment {
            comment += 1;
        } else {
            blank += 1;
        }
    }

    (total, code, comment, blank)
}

/// Count lines in an already-loaded content string or decoded slice.
#[allow(dead_code)]
pub fn analyze_content(content: &str, path: &Path) -> (usize, usize, usize, usize) {
    let spec = path.extension().and_then(|e| e.to_str()).and_then(|e| {
        let len = e.len();
        if len <= 15 {
            let mut buf = [0u8; 16];
            buf[0] = b'.';
            for (i, b) in e.bytes().enumerate() {
                buf[i + 1] = b.to_ascii_lowercase();
            }
            std::str::from_utf8(&buf[..=len])
                .ok()
                .and_then(|s| crate::language::COMMENT_REGISTRY.get(s))
        } else {
            let lower = format!(".{}", e.to_ascii_lowercase());
            crate::language::COMMENT_REGISTRY.get(lower.as_str())
        }
    });

    analyze_content_with_spec(content, spec)
}

#[cfg(test)]
pub fn analyze_file(path: &Path) -> (usize, usize, usize, usize) {
    match std::fs::read(path) {
        Ok(bytes) => {
            let content = String::from_utf8_lossy(&bytes);
            analyze_content(&content, path)
        }
        Err(_) => (0, 0, 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_comment_inside_string_literal() {
        let rs_path = PathBuf::from("test.rs");
        let content = r#"
fn main() {
    let s = "// not a comment";
    let block = "/* also not a comment */";
    println!("{}", s);
}
"#;
        let (total, code, comment, blank) = analyze_content(content, &rs_path);
        assert_eq!(code, 5);
        assert_eq!(comment, 0);
        assert_eq!(blank, 1);
        assert_eq!(total, 6);
    }

    #[test]
    fn test_nested_comments_rust() {
        let rs_path = PathBuf::from("test.rs");
        let content = r#"
/* outer
   /* nested */
   still comment */
fn main() {}
"#;
        let (total, code, comment, blank) = analyze_content(content, &rs_path);
        assert_eq!(code, 1);
        assert_eq!(comment, 3);
        assert_eq!(blank, 1);
        assert_eq!(total, 5);
    }

    #[test]
    fn test_python_triple_quotes_string_vs_comment() {
        let py_path = PathBuf::from("test.py");
        let content = r#"
"""Docstring here"""
x = 10
'''Single triple docstring'''
y = 20
"#;
        let (total, code, comment, blank) = analyze_content(content, &py_path);
        assert_eq!(code, 2);
        assert_eq!(comment, 2);
        assert_eq!(blank, 1);
        assert_eq!(total, 5);
    }
}
