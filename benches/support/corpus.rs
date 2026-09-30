// Author: kelexine <https://github.com/kelexine>
// support/corpus.rs — Deterministic on-disk corpora for discovery, pipeline and e2e benchmarks.
//
// A corpus is `files` source files cycling through every committed fixture,
// spread across `dirs` package directories.  Optional "noise" adds the entries a
// real repository carries: pruned directories, a lockfile, a binary asset, and
// files that `.locignore` rules include or exclude depending on the profile.
//
// Directory names deliberately avoid the scanner's built-in exclusions
// (`bin`, `build`, `dist`, `target`, `vendor`, ...) so only the noise we add is pruned.

use std::fs;
use std::io;
use std::path::Path;

use tempfile::TempDir;

use super::encodings::pseudo_random_bytes;
use super::fixtures::{self, Fixture};

/// Which ignore rules the corpus root carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreProfile {
    /// No ignore files at all (the scanner's zero-rule fast path).
    None,
    /// `.locignore` with plain exclusions (`*.log`, `generated/`).
    Simple,
    /// Simple rules plus a `!keep.log` negation, which disables directory pruning.
    WithNegation,
}

impl IgnoreProfile {
    /// Stable label used in benchmark IDs.
    pub fn label(self) -> &'static str {
        match self {
            IgnoreProfile::None => "no_rules",
            IgnoreProfile::Simple => "simple_rules",
            IgnoreProfile::WithNegation => "negation_rules",
        }
    }

    fn locignore_body(self) -> Option<&'static str> {
        match self {
            IgnoreProfile::None => None,
            IgnoreProfile::Simple => Some("# bench profile: simple\n*.log\ngenerated/\n"),
            IgnoreProfile::WithNegation => {
                Some("# bench profile: negation\n*.log\ngenerated/\n!keep.log\n")
            }
        }
    }
}

/// Shape of a generated corpus.
#[derive(Debug, Clone, Copy)]
pub struct CorpusSpec {
    /// Number of source files to generate.
    pub files: usize,
    /// Number of package directories the files are spread across (min 1).
    pub dirs: usize,
    /// Approximate lines per source file (whole fixture repetitions).
    pub lines_per_file: usize,
    /// Add pruned directories, a lockfile, a binary asset and log/generated files.
    pub noise: bool,
    /// Ignore rules written to the corpus root.
    pub ignore: IgnoreProfile,
}

/// A generated corpus.  The backing directory is removed on drop.
pub struct Corpus {
    dir: TempDir,
    spec: CorpusSpec,
}

impl Corpus {
    /// Materialise `spec` in a fresh temporary directory.
    pub fn build(spec: CorpusSpec) -> io::Result<Self> {
        let dir = TempDir::new()?;
        let root = dir.path();
        let dirs = spec.dirs.max(1);

        for i in 0..spec.files {
            let fixture = &fixtures::ALL[i % fixtures::ALL.len()];
            let rel = format!("pkg_{:03}/inner/file_{:05}{}", i % dirs, i, fixture.ext);
            write_text(root, &rel, &scaled_content(fixture, spec.lines_per_file))?;
        }

        if spec.noise {
            write_noise(root)?;
        }
        if let Some(body) = spec.ignore.locignore_body() {
            write_text(root, ".locignore", body)?;
        }

        Ok(Self { dir, spec })
    }

    /// Absolute path of the corpus root.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// The spec this corpus was built from.
    pub fn spec(&self) -> CorpusSpec {
        self.spec
    }

    /// Number of files a default scan (hidden files off) must report.
    ///
    /// Counts every surfaced entry, including the lockfile and binary asset,
    /// because the scanner lists them even though it does not count their lines.
    pub fn visible_entries(&self) -> usize {
        let mut visible = self.spec.files;
        if self.spec.noise {
            visible += 2; // Cargo.lock + assets/logo.png
            visible += match self.spec.ignore {
                // generated/out.rs + logs/app.log + logs/keep.log
                IgnoreProfile::None => 3,
                IgnoreProfile::Simple => 0,
                // logs/keep.log is re-included by the negation.
                IgnoreProfile::WithNegation => 1,
            };
        }
        visible
    }
}

fn scaled_content(fixture: &Fixture, lines: usize) -> String {
    match fixture.ext {
        // Notebooks must stay a single valid JSON document, so scale by cells.
        ".ipynb" => fixtures::scale_notebook(lines.div_ceil(12).max(1)),
        _ => fixtures::repeat_to_lines(fixture.content, lines.max(1)),
    }
}

fn write_noise(root: &Path) -> io::Result<()> {
    // Pruned by the built-in exclusion list.
    write_text(root, "node_modules/dep_a/index.js", "module.exports = 1;\n")?;
    write_text(root, "node_modules/dep_b/index.js", "module.exports = 2;\n")?;
    write_text(root, "target/debug/junk.rs", "fn junk() {}\n")?;
    // Pruned as a hidden directory.
    write_text(root, ".cache/tmp.py", "x = 1\n")?;
    // Visible or ignored depending on the profile.
    write_text(root, "generated/out.rs", "pub const GENERATED: u32 = 1;\n")?;
    write_text(root, "logs/app.log", "started\nstopped\n")?;
    write_text(root, "logs/keep.log", "kept\n")?;
    // Always surfaced: a known lockfile and a binary asset.
    write_text(root, "Cargo.lock", "# lockfile placeholder\n")?;
    write_bytes(root, "assets/logo.png", &pseudo_random_bytes(64))
}

fn write_text(root: &Path, rel: &str, content: &str) -> io::Result<()> {
    write_bytes(root, rel, content.as_bytes())
}

fn write_bytes(root: &Path, rel: &str, content: &[u8]) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
