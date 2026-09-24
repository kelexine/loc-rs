// Author: kelexine (https://github.com/kelexine)
// counter/lines.rs — Line counting and language-aware comment tracking

use std::path::Path;

/// Count lines in an already-loaded content string or decoded slice.
///
/// Multi-line comment tracking is language-aware via the comment registry.
/// Handles the Python triple-quote single-liner bug: for equal start/end
/// delimiters (e.g. `"""`), we verify a *second* occurrence exists on the same
/// line before deciding the block closes immediately.
pub fn analyze_content(content: &str, path: &Path) -> (usize, usize, usize, usize) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    let spec = crate::language::COMMENT_REGISTRY.get(ext.as_str());

    let mut total = 0usize;
    let mut code = 0usize;
    let mut comment = 0usize;
    let mut blank = 0usize;
    let mut in_multi_comment = false;

    for line in content.lines() {
        total += 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if in_multi_comment {
                comment += 1;
            } else {
                blank += 1;
            }
            continue;
        }

        if let Some(s) = spec {
            if in_multi_comment {
                comment += 1;
                if let Some((_, end)) = s.multi
                    && trimmed.contains(end)
                {
                    in_multi_comment = false;
                }
                continue;
            }

            if let Some((start, end)) = s.multi
                && trimmed.starts_with(start)
            {
                comment += 1;

                // Determine whether the multi-line block closes on this same line.
                let ends_on_same_line = if start == end {
                    // Same delimiter on both sides (e.g. Python """...""").
                    // A second occurrence must exist *after* the opening delimiter.
                    trimmed[start.len()..].contains(end)
                } else {
                    // Different delimiters: block closes if end marker appears anywhere
                    // on the line (and it's not just the opening marker itself).
                    trimmed.contains(end)
                };

                if !ends_on_same_line {
                    in_multi_comment = true;
                }
                continue;
            }

            if let Some(single) = s.single
                && trimmed.starts_with(single)
            {
                comment += 1;
                continue;
            }
        }

        code += 1;
    }

    (total, code, comment, blank)
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
