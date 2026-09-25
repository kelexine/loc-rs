// Author: kelexine (https://github.com/kelexine)
// counter/process.rs — Per-file processing, binary heuristic, and content loading

use std::borrow::Cow;
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};

use super::ScanConfig;
use super::discovery::get_fs_last_modified;
use super::embedded::{
    is_html_or_template, is_jupyter_notebook, parse_html_embedded, parse_jupyter_notebook,
};
use super::lines::analyze_content_with_spec;
use crate::extractors;
use crate::language::BINARY_EXTENSIONS;
use crate::models::FileInfo;

/// Process a single file: determine binary/text status, load content with UTF-8 fallback,
/// count code/comment/blank lines, and optionally extract functions.
pub fn process_file(path: &Path, config: &ScanConfig) -> Result<Option<FileInfo>> {
    if !path.is_file() {
        return Ok(None);
    }

    // ── Lockfile fast-path ────────────────────────────────────────────────────
    // Recognised lockfiles are shown in the tree but never line-counted.
    // We resolve last-modified when --git-dates is active, then return without
    // reading any file content. This also bypasses the extension filter so
    // that lockfiles always show up in the tree even when -t is specified.
    if crate::language::is_lockfile(path) {
        let last_modified = if config.use_git_dates {
            if let Some(ref cache) = config.git_dates_cache {
                cache.get(path).copied()
            } else {
                get_fs_last_modified(path)
            }
        } else {
            None
        };
        return Ok(Some(
            FileInfo::new(path.to_path_buf(), 0, 0, 0, 0, false, last_modified).mark_as_lockfile(),
        ));
    }

    // Extract lowercase extension once
    let ext_str: Cow<'_, str> = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => {
            let mut s = String::with_capacity(e.len() + 1);
            s.push('.');
            for b in e.bytes() {
                s.push(b.to_ascii_lowercase() as char);
            }
            Cow::Owned(s)
        }
        None => Cow::Borrowed(""),
    };

    let known_file = crate::language::detect_known_filename(path);

    // Extension filter
    if let Some(allowed) = &config.allowed_extensions {
        let is_allowed = allowed.contains(ext_str.as_ref())
            || path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| allowed.contains(n))
                .unwrap_or(false)
            || known_file
                .as_ref()
                .map(|k| {
                    allowed.contains(k.breakdown_key)
                        || allowed.contains(&format!(".{}", k.breakdown_key))
                })
                .unwrap_or(false);

        // If not allowed and not an extensionless candidate for shebang detection, filter out now.
        if !is_allowed && !ext_str.is_empty() {
            return Ok(None);
        }
    }

    // Check fast-path binary extension
    let is_ext_binary = BINARY_EXTENSIONS.contains(ext_str.as_ref());

    // Skip binary files when type-filtering is active
    if is_ext_binary && config.allowed_extensions.is_some() {
        return Ok(None);
    }

    // Read file bytes once — avoids double reading for binary check + analysis
    // Also eliminates a redundant statx call: read immediately and handle NotFound/is_dir on error
    let raw_bytes = if !is_ext_binary {
        match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound || path.is_dir() {
                    return Ok(None);
                }
                return Err(anyhow::anyhow!("read error: {}", e));
            }
        }
    } else {
        if !path.is_file() {
            return Ok(None);
        }
        None
    };

    let (is_binary, content) = if is_ext_binary {
        (true, None)
    } else if let Some(ref bytes) = raw_bytes {
        match decode_text_buffer(bytes) {
            Some(c) => (false, Some(c)),
            None => (true, None),
        }
    } else {
        (false, None)
    };

    // Skip binary files when type-filtering is active
    if is_binary && config.allowed_extensions.is_some() {
        return Ok(None);
    }

    // Resolve comment spec and language:
    // 1. Known filename (Makefile, Dockerfile, Kconfig, CMakeLists.txt, etc.)
    // 2. Extension registry (.rs, .py, .c, etc.)
    // 3. Shebang (#!) for extensionless files
    let mut resolved_lang = known_file.as_ref().map(|k| k.breakdown_key);
    let mut spec = known_file.as_ref().map(|k| k.comment_spec).or_else(|| {
        crate::language::COMMENT_REGISTRY
            .get(ext_str.as_ref())
            .copied()
    });

    if spec.is_none()
        && ext_str.is_empty()
        && let Some(ref s) = content
        && let Some(first_line) = s.lines().next()
        && let Some(shebang) = crate::language::detect_shebang(first_line)
    {
        resolved_lang = Some(shebang.breakdown_key);
        spec = Some(shebang.comment_spec);
    }

    // Deferred type filter check for extensionless files
    if let Some(allowed) = &config.allowed_extensions
        && ext_str.is_empty()
        && known_file.is_none()
    {
        let is_allowed = resolved_lang
            .map(|lang| allowed.contains(lang) || allowed.contains(&format!(".{}", lang)))
            .unwrap_or(false);
        if !is_allowed {
            return Ok(None);
        }
    }

    let mut embedded_chunks = Vec::new();
    let (total, code, comment, blank) = match &content {
        Some(s) if is_jupyter_notebook(path) => {
            if let Some((t, c, cm, b, chunks)) = parse_jupyter_notebook(s) {
                embedded_chunks = chunks;
                (t, c, cm, b)
            } else {
                analyze_content_with_spec(s, spec.as_ref())
            }
        }
        Some(s) if is_html_or_template(path) => {
            let container = if ext_str.is_empty() {
                resolved_lang.unwrap_or(".html")
            } else {
                ext_str.as_ref()
            };
            let (t, c, cm, b, chunks) = parse_html_embedded(s, container);
            embedded_chunks = chunks;
            (t, c, cm, b)
        }
        Some(s) => analyze_content_with_spec(s, spec.as_ref()),
        None => (0, 0, 0, 0),
    };

    // Only populate last_modified when --git-dates is active.
    let last_modified: Option<DateTime<Utc>> = if config.use_git_dates {
        if let Some(ref cache) = config.git_dates_cache {
            cache.get(path).copied()
        } else {
            get_fs_last_modified(path)
        }
    } else {
        None
    };

    let mut fi = FileInfo::new(
        path.to_path_buf(),
        total,
        code,
        comment,
        blank,
        is_binary,
        last_modified,
    );

    let canonical_lang = if let Some(lang) = resolved_lang {
        crate::language::canonical_language_name(lang)
    } else if !ext_str.is_empty() {
        crate::language::canonical_language_name(ext_str.as_ref())
    } else {
        "Unknown"
    };
    fi = fi.with_language(canonical_lang);

    if !embedded_chunks.is_empty() {
        fi = fi.with_embedded(embedded_chunks);
    }

    if config.extract_functions
        && !is_binary
        && let Some(ref s) = content
        && let Some(extractor) = extractors::get_extractor(path)
    {
        fi = fi.with_functions(extractor.extract(s));
    }

    Ok(Some(fi))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
    Binary,
}

/// Detects text encoding from BOM signatures and byte distribution heuristics.
pub fn detect_encoding(buf: &[u8]) -> TextEncoding {
    // 1. Check BOM signatures
    if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return TextEncoding::Utf8Bom;
    }
    if buf.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return TextEncoding::Utf32Le;
    }
    if buf.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return TextEncoding::Utf32Be;
    }
    if buf.starts_with(&[0xFF, 0xFE]) {
        return TextEncoding::Utf16Le;
    }
    if buf.starts_with(&[0xFE, 0xFF]) {
        return TextEncoding::Utf16Be;
    }

    // 2. Check for null bytes in initial sample
    let check_len = buf.len().min(8192);
    let slice = &buf[..check_len];
    if !slice.contains(&0u8) {
        return TextEncoding::Utf8;
    }

    // 3. Heuristic for BOM-less UTF-16
    // In UTF-16 text with ASCII code points, every alternating byte is 0x00.
    if slice.len() >= 4 {
        let pairs = slice.len() / 2;
        let mut le_ascii_zeros = 0;
        let mut be_ascii_zeros = 0;

        for chunk in slice.as_chunks::<2>().0 {
            // LE: chunk[0] is ASCII graphic or whitespace, chunk[1] is 0x00
            if chunk[1] == 0
                && (chunk[0] < 128
                    && (chunk[0].is_ascii_graphic() || chunk[0].is_ascii_whitespace()))
            {
                le_ascii_zeros += 1;
            }
            // BE: chunk[0] is 0x00, chunk[1] is ASCII graphic or whitespace
            if chunk[0] == 0
                && (chunk[1] < 128
                    && (chunk[1].is_ascii_graphic() || chunk[1].is_ascii_whitespace()))
            {
                be_ascii_zeros += 1;
            }
        }

        if pairs > 0 {
            if le_ascii_zeros * 10 >= pairs * 7 {
                return TextEncoding::Utf16Le;
            }
            if be_ascii_zeros * 10 >= pairs * 7 {
                return TextEncoding::Utf16Be;
            }
        }
    }

    TextEncoding::Binary
}

/// Decodes a raw buffer according to its detected encoding into a string.
/// Returns None if the buffer is determined to be binary.
pub fn decode_text_buffer(buf: &[u8]) -> Option<Cow<'_, str>> {
    match detect_encoding(buf) {
        TextEncoding::Binary => None,
        TextEncoding::Utf8 => match std::str::from_utf8(buf) {
            Ok(s) => Some(Cow::Borrowed(s)),
            Err(_) => Some(String::from_utf8_lossy(buf)),
        },
        TextEncoding::Utf8Bom => {
            let slice = &buf[3..];
            match std::str::from_utf8(slice) {
                Ok(s) => Some(Cow::Borrowed(s)),
                Err(_) => Some(String::from_utf8_lossy(slice)),
            }
        }
        TextEncoding::Utf16Le => {
            let slice = if buf.starts_with(&[0xFF, 0xFE]) {
                &buf[2..]
            } else {
                buf
            };
            let u16_iter = slice
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]));
            let s: String = char::decode_utf16(u16_iter)
                .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            Some(Cow::Owned(s))
        }
        TextEncoding::Utf16Be => {
            let slice = if buf.starts_with(&[0xFE, 0xFF]) {
                &buf[2..]
            } else {
                buf
            };
            let u16_iter = slice
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes([c[0], c[1]]));
            let s: String = char::decode_utf16(u16_iter)
                .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            Some(Cow::Owned(s))
        }
        TextEncoding::Utf32Le => {
            let slice = if buf.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
                &buf[4..]
            } else {
                buf
            };
            let s: String = slice
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .map(|u| char::from_u32(u).unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            Some(Cow::Owned(s))
        }
        TextEncoding::Utf32Be => {
            let slice = if buf.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
                &buf[4..]
            } else {
                buf
            };
            let s: String = slice
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]))
                .map(|u| char::from_u32(u).unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            Some(Cow::Owned(s))
        }
    }
}

/// Heuristic: checks first 8 KiB of buffer for null bytes, accounting for UTF-16/32 BOMs.
#[allow(dead_code)]
pub fn is_binary_buffer(buf: &[u8]) -> bool {
    detect_encoding(buf) == TextEncoding::Binary
}

#[cfg(test)]
pub fn is_binary_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    if BINARY_EXTENSIONS.contains(ext.as_str()) {
        return true;
    }

    match std::fs::read(path) {
        Ok(buf) => is_binary_buffer(&buf),
        Err(_) => true,
    }
}
