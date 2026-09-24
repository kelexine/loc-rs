// Author: kelexine (https://github.com/kelexine)
// counter/process.rs — Per-file processing, binary heuristic, and content loading

use std::borrow::Cow;
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};

use super::ScanConfig;
use super::discovery::get_fs_last_modified;
use super::lines::analyze_content;
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

    // Extension filter
    if let Some(allowed) = &config.allowed_extensions {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();
        if !allowed.contains(&ext) {
            return Ok(None);
        }
    }

    // Check fast-path binary extension
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();
    let is_ext_binary = BINARY_EXTENSIONS.contains(ext.as_str());

    // Skip binary files when type-filtering is active
    if is_ext_binary && config.allowed_extensions.is_some() {
        return Ok(None);
    }

    // Read file bytes once — avoids double reading for binary check + analysis
    let raw_bytes = if !is_ext_binary {
        match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) => return Err(anyhow::anyhow!("read error: {}", e)),
        }
    } else {
        None
    };

    let is_binary = if is_ext_binary {
        true
    } else if let Some(ref bytes) = raw_bytes {
        is_binary_buffer(bytes)
    } else {
        false
    };

    // Skip binary files when type-filtering is active
    if is_binary && config.allowed_extensions.is_some() {
        return Ok(None);
    }

    // Convert to string slice (zero-copy when valid UTF-8, lossy fallback if Latin-1/ISO-8859/other)
    let content: Option<Cow<'_, str>> = if !is_binary {
        raw_bytes.as_deref().map(String::from_utf8_lossy)
    } else {
        None
    };

    let (total, code, comment, blank) = match &content {
        Some(s) => analyze_content(s, path),
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

    if config.extract_functions
        && !is_binary
        && let Some(ref s) = content
        && let Some(extractor) = extractors::get_extractor(path)
    {
        fi = fi.with_functions(extractor.extract(s));
    }

    Ok(Some(fi))
}

/// Heuristic: checks first 8 KiB of buffer for null bytes, accounting for UTF-16/32 BOMs.
pub fn is_binary_buffer(buf: &[u8]) -> bool {
    let check_len = buf.len().min(8192);
    let slice = &buf[..check_len];

    // BOM Check: UTF-16/32 files contain null bytes but are not binary
    if slice.len() >= 2
        && ((slice[0] == 0xFE && slice[1] == 0xFF) || (slice[0] == 0xFF && slice[1] == 0xFE))
    {
        return false; // UTF-16
    }
    if slice.len() >= 4
        && ((slice[0] == 0x00 && slice[1] == 0x00 && slice[2] == 0xFE && slice[3] == 0xFF)
            || (slice[0] == 0xFF && slice[1] == 0xFE && slice[2] == 0x00 && slice[3] == 0x00))
    {
        return false; // UTF-32
    }

    slice.contains(&0u8)
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
