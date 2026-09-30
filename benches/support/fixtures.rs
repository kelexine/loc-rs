// Author: kelexine <https://github.com/kelexine>
// support/fixtures.rs — Committed fixture manifest and deterministic scaling helpers.
//
// The fixtures under `benches/fixtures/` are the single source of truth for every benchmark input.  Larger inputs are derived by repeating whole fixtures, which
// keeps results reproducible and independent of the host filesystem.  Repeating
// is only sound because every fixture starts and ends in a neutral lexer state
// (no unterminated block comment or string); `tests/bench_support.rs` verifies
// that invariant against the real `loc` binary.

/// A committed benchmark input file.
#[derive(Debug, Clone, Copy)]
pub struct Fixture {
    /// Short stable label used in benchmark IDs (e.g. `rust`).
    pub name: &'static str,
    /// Lowercase extension with the leading dot, matching `COMMENT_REGISTRY` keys.
    pub ext: &'static str,
    /// File content, embedded at compile time.
    pub content: &'static str,
}

/// Number of plain-source fixtures at the front of [`ALL`].
pub const SOURCE_COUNT: usize = 11;

/// Every committed fixture: plain sources first, then container formats.
pub const ALL: &[Fixture] = &[
    Fixture {
        name: "rust",
        ext: ".rs",
        content: include_str!("../fixtures/sample.rs"),
    },
    Fixture {
        name: "python",
        ext: ".py",
        content: include_str!("../fixtures/sample.py"),
    },
    Fixture {
        name: "javascript",
        ext: ".js",
        content: include_str!("../fixtures/sample.js"),
    },
    Fixture {
        name: "typescript",
        ext: ".ts",
        content: include_str!("../fixtures/sample.ts"),
    },
    Fixture {
        name: "go",
        ext: ".go",
        content: include_str!("../fixtures/sample.go"),
    },
    Fixture {
        name: "c",
        ext: ".c",
        content: include_str!("../fixtures/sample.c"),
    },
    Fixture {
        name: "cpp",
        ext: ".cpp",
        content: include_str!("../fixtures/sample.cpp"),
    },
    Fixture {
        name: "java",
        ext: ".java",
        content: include_str!("../fixtures/Sample.java"),
    },
    Fixture {
        name: "php",
        ext: ".php",
        content: include_str!("../fixtures/sample.php"),
    },
    Fixture {
        name: "ruby",
        ext: ".rb",
        content: include_str!("../fixtures/sample.rb"),
    },
    Fixture {
        name: "swift",
        ext: ".swift",
        content: include_str!("../fixtures/sample.swift"),
    },
    Fixture {
        name: "html",
        ext: ".html",
        content: include_str!("../fixtures/sample.html"),
    },
    Fixture {
        name: "notebook",
        ext: ".ipynb",
        content: include_str!("../fixtures/sample.ipynb"),
    },
];

/// The plain-source fixtures (everything except HTML and the notebook).
pub fn sources() -> &'static [Fixture] {
    &ALL[..SOURCE_COUNT]
}

/// Look up a fixture by its `name`.
///
/// # Panics
/// Panics when `name` is not in [`ALL`].  Benchmark setup is the only caller and
/// an unknown name is a programming error that must fail loudly.
pub fn get(name: &str) -> &'static Fixture {
    ALL.iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("unknown benchmark fixture `{name}`"))
}

/// Repeat `content` (newline-terminated per copy) until it holds at least
/// `target_lines` lines.
///
/// The result is always a whole number of copies, so its line count is an exact
/// multiple of the fixture's.  Returns an empty string when either the fixture
/// has no lines or `target_lines` is zero, which avoids a divide-by-zero and an
/// unbounded loop on degenerate input.
pub fn repeat_to_lines(content: &str, target_lines: usize) -> String {
    let per_copy = content.lines().count();
    if per_copy == 0 || target_lines == 0 {
        return String::new();
    }

    let copies = target_lines.div_ceil(per_copy);
    let mut out = String::with_capacity((content.len() + 1) * copies);
    for _ in 0..copies {
        out.push_str(content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Build a notebook whose `cells` array holds `copies` repetitions of the
/// fixture's cells (at least one).  Metadata is preserved so the kernel language
/// resolves exactly as it does for the committed fixture.
///
/// # Panics
/// Panics if the committed notebook fixture is not valid JSON; the fixture
/// tests guard against that.
pub fn scale_notebook(copies: usize) -> String {
    let mut doc: serde_json::Value = serde_json::from_str(get("notebook").content)
        .expect("committed notebook fixture must be valid JSON");

    if let Some(cells) = doc.get_mut("cells").and_then(|c| c.as_array_mut()) {
        let original = cells.clone();
        for _ in 1..copies.max(1) {
            cells.extend(original.iter().cloned());
        }
    }

    serde_json::to_string_pretty(&doc).expect("serializing an in-memory JSON value cannot fail")
}
