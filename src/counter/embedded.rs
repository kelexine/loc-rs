// Author: kelexine (https://github.com/kelexine)
// counter/embedded.rs — Embedded language extraction (HTML script/style and Jupyter .ipynb)

use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

use super::lines::analyze_content_with_spec;
use crate::language::COMMENT_REGISTRY;
use crate::models::EmbeddedChunk;

static SCRIPT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<script(?P<attrs>[^>]*)>(?P<body>.*?)</script>").unwrap());

static STYLE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<style(?P<attrs>[^>]*)>(?P<body>.*?)</style>").unwrap());

/// Check if a path corresponds to a Jupyter notebook.
pub fn is_jupyter_notebook(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("ipynb"))
        .unwrap_or(false)
}

/// Check if a path corresponds to an HTML/component template file.
pub fn is_html_or_template(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_ascii_lowercase();
            matches!(
                lower.as_str(),
                "html" | "htm" | "vue" | "svelte" | "xml" | "xsl" | "xslt"
            )
        })
        .unwrap_or(false)
}

/// Parse Jupyter notebook JSON and extract embedded lines per language (code and markdown).
pub fn parse_jupyter_notebook(
    content: &str,
) -> Option<(usize, usize, usize, usize, Vec<EmbeddedChunk>)> {
    let json: Value = serde_json::from_str(content).ok()?;
    let cells = json.get("cells")?.as_array()?;

    // Determine kernel language from metadata (defaulting to python)
    let kernel_lang = json
        .pointer("/metadata/language_info/name")
        .and_then(|v| v.as_str())
        .unwrap_or("python");

    let code_ext = match kernel_lang.to_ascii_lowercase().as_str() {
        "python" | "py" => ".py",
        "javascript" | "js" => ".js",
        "typescript" | "ts" => ".ts",
        "r" => ".r",
        "julia" | "jl" => ".jl",
        "rust" | "rs" => ".rs",
        _ => ".py",
    };

    let code_spec = COMMENT_REGISTRY.get(code_ext);
    let md_spec = COMMENT_REGISTRY.get(".md");

    let mut code_lines = 0usize;
    let mut code_code = 0usize;
    let mut code_comment = 0usize;
    let mut code_blank = 0usize;

    let mut md_lines = 0usize;
    let mut md_code = 0usize;
    let mut md_comment = 0usize;
    let mut md_blank = 0usize;

    for cell in cells {
        let cell_type = cell.get("cell_type").and_then(|v| v.as_str()).unwrap_or("");
        let source_val = cell.get("source");

        let mut cell_source = String::new();
        match source_val {
            Some(Value::Array(lines)) => {
                for l in lines {
                    if let Some(s) = l.as_str() {
                        cell_source.push_str(s);
                    }
                }
            }
            Some(Value::String(s)) => cell_source.push_str(s),
            _ => continue,
        }

        if cell_source.is_empty() {
            continue;
        }

        if cell_type == "code" {
            let (t, c, cm, b) = analyze_content_with_spec(&cell_source, code_spec);
            code_lines += t;
            code_code += c;
            code_comment += cm;
            code_blank += b;
        } else if cell_type == "markdown" {
            let (t, c, cm, b) = analyze_content_with_spec(&cell_source, md_spec);
            md_lines += t;
            md_code += c;
            md_comment += cm;
            md_blank += b;
        }
    }

    let total_lines = code_lines + md_lines;
    let total_code = code_code + md_code;
    let total_comment = code_comment + md_comment;
    let total_blank = code_blank + md_blank;

    let mut chunks = Vec::new();
    let clean_code_ext = code_ext.trim_start_matches('.');
    if code_lines > 0 {
        chunks.push(EmbeddedChunk {
            extension: clean_code_ext.to_string(),
            lines: code_lines,
            code: code_code,
            comment: code_comment,
            blank: code_blank,
        });
    }
    if md_lines > 0 {
        chunks.push(EmbeddedChunk {
            extension: "md".to_string(),
            lines: md_lines,
            code: md_code,
            comment: md_comment,
            blank: md_blank,
        });
    }

    Some((total_lines, total_code, total_comment, total_blank, chunks))
}

/// Extract embedded script, style, and template blocks from HTML-like content.
pub fn parse_html_embedded(
    content: &str,
    container_ext: &str,
) -> (usize, usize, usize, usize, Vec<EmbeddedChunk>) {
    let mut embedded_chunks = Vec::new();

    let js_spec = COMMENT_REGISTRY.get(".js");
    let ts_spec = COMMENT_REGISTRY.get(".ts");
    let css_spec = COMMENT_REGISTRY.get(".css");

    // Process scripts
    for cap in SCRIPT_REGEX.captures_iter(content) {
        let attrs = cap.name("attrs").map(|m| m.as_str()).unwrap_or("");
        let body = cap.name("body").map(|m| m.as_str()).unwrap_or("");

        let is_ts = attrs.contains("lang=\"ts\"")
            || attrs.contains("lang='ts'")
            || attrs.contains("type=\"text/typescript\"")
            || attrs.contains("type='text/typescript'");

        let target_ext = if is_ts { "ts" } else { "js" };
        let spec = if is_ts { ts_spec } else { js_spec };

        let (t, c, cm, b) = analyze_content_with_spec(body, spec);
        if t > 0 {
            embedded_chunks.push(EmbeddedChunk {
                extension: target_ext.to_string(),
                lines: t,
                code: c,
                comment: cm,
                blank: b,
            });
        }
    }

    // Process styles
    for cap in STYLE_REGEX.captures_iter(content) {
        let attrs = cap.name("attrs").map(|m| m.as_str()).unwrap_or("");
        let body = cap.name("body").map(|m| m.as_str()).unwrap_or("");

        let is_scss = attrs.contains("lang=\"scss\"") || attrs.contains("lang='scss'");
        let target_ext = if is_scss { "scss" } else { "css" };

        let (t, c, cm, b) = analyze_content_with_spec(body, css_spec);
        if t > 0 {
            embedded_chunks.push(EmbeddedChunk {
                extension: target_ext.to_string(),
                lines: t,
                code: c,
                comment: cm,
                blank: b,
            });
        }
    }

    // Strip bodies from script and style tags to count remaining HTML container lines
    let stripped = SCRIPT_REGEX.replace_all(content, "<script$attrs></script>");
    let stripped = STYLE_REGEX.replace_all(&stripped, "<style$attrs></style>");

    let html_spec = COMMENT_REGISTRY.get(container_ext);
    let (h_total, h_code, h_comment, h_blank) = analyze_content_with_spec(&stripped, html_spec);

    let clean_container = container_ext.trim_start_matches('.');
    if h_total > 0 && !embedded_chunks.is_empty() {
        embedded_chunks.insert(
            0,
            EmbeddedChunk {
                extension: clean_container.to_string(),
                lines: h_total,
                code: h_code,
                comment: h_comment,
                blank: h_blank,
            },
        );
    }

    // Physical file metrics
    let (phys_total, phys_code, phys_comment, phys_blank) =
        analyze_content_with_spec(content, html_spec);

    (
        phys_total,
        phys_code,
        phys_comment,
        phys_blank,
        embedded_chunks,
    )
}
