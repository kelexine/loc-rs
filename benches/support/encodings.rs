// Author: kelexine <https://github.com/kelexine>
// support/encodings.rs — Deterministic encoded variants of a text buffer.
//
// Produces the byte layouts `counter::process::detect_encoding` must classify:
// every BOM flavour, the BOM-less UTF-16 heuristic path, lossy UTF-8, and binary.

/// One byte-level representation of a text buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16LeBom,
    Utf16BeBom,
    Utf32LeBom,
    Utf32BeBom,
    /// UTF-16LE with no BOM; exercises the alternating-zero heuristic.
    Utf16LeNoBom,
    /// UTF-8 with one invalid sequence; exercises the lossy fallback.
    Utf8Invalid,
    /// Pseudo-random bytes containing NULs; must be classified as binary.
    Binary,
}

/// Every variant, in benchmark order.
pub const ALL: [Encoding; 9] = [
    Encoding::Utf8,
    Encoding::Utf8Bom,
    Encoding::Utf16LeBom,
    Encoding::Utf16BeBom,
    Encoding::Utf32LeBom,
    Encoding::Utf32BeBom,
    Encoding::Utf16LeNoBom,
    Encoding::Utf8Invalid,
    Encoding::Binary,
];

impl Encoding {
    /// Stable label used in benchmark IDs.
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf8",
            Encoding::Utf8Bom => "utf8_bom",
            Encoding::Utf16LeBom => "utf16le_bom",
            Encoding::Utf16BeBom => "utf16be_bom",
            Encoding::Utf32LeBom => "utf32le_bom",
            Encoding::Utf32BeBom => "utf32be_bom",
            Encoding::Utf16LeNoBom => "utf16le_nobom",
            Encoding::Utf8Invalid => "utf8_invalid",
            Encoding::Binary => "binary",
        }
    }
}

/// Encode `text` as `encoding`.  [`Encoding::Binary`] ignores the content and
/// returns pseudo-random bytes of comparable length (minimum 64).
pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => with_prefix(&[0xEF, 0xBB, 0xBF], text.as_bytes()),
        Encoding::Utf16LeBom => with_prefix(&[0xFF, 0xFE], &utf16(text, u16::to_le_bytes)),
        Encoding::Utf16BeBom => with_prefix(&[0xFE, 0xFF], &utf16(text, u16::to_be_bytes)),
        Encoding::Utf32LeBom => {
            with_prefix(&[0xFF, 0xFE, 0x00, 0x00], &utf32(text, u32::to_le_bytes))
        }
        Encoding::Utf32BeBom => {
            with_prefix(&[0x00, 0x00, 0xFE, 0xFF], &utf32(text, u32::to_be_bytes))
        }
        Encoding::Utf16LeNoBom => utf16(text, u16::to_le_bytes),
        Encoding::Utf8Invalid => {
            let mut bytes = text.as_bytes().to_vec();
            let at = bytes.len() / 2;
            // 0xC3 promises a continuation byte; 0x28 ('(') is not one.
            bytes.splice(at..at, [0xC3, 0x28]);
            bytes
        }
        Encoding::Binary => pseudo_random_bytes(text.len().max(64)),
    }
}

/// Deterministic xorshift64 byte stream that can never be mistaken for a BOM and
/// always contains a NUL inside the 8 KiB detection window (for `len >= 5`).
pub fn pseudo_random_bytes(len: usize) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);

    if let Some(first) = out.first_mut() {
        *first = 0x7F;
    }
    if let Some(nul) = out.get_mut(4) {
        *nul = 0x00;
    }
    out
}

fn with_prefix(prefix: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(prefix.len() + body.len());
    out.extend_from_slice(prefix);
    out.extend_from_slice(body);
    out
}

fn utf16(text: &str, to_bytes: fn(u16) -> [u8; 2]) -> Vec<u8> {
    text.encode_utf16().flat_map(to_bytes).collect()
}

fn utf32(text: &str, to_bytes: fn(u32) -> [u8; 4]) -> Vec<u8> {
    text.chars().flat_map(|c| to_bytes(c as u32)).collect()
}
