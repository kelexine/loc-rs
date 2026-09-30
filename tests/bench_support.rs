// Author: kelexine <https://github.com/kelexine>
// bench_support.rs — Validates the benchmark support layer (`benches/support/`).
//
// `harness = false` bench targets cannot host `#[test]`s, so the support modules are
// mounted here via `#[path]` and checked in isolation, then cross-checked against the
// shipped `loc` binary so benchmark inputs are proven to mean what the benches assume.

#[path = "../benches/support/mod.rs"]
mod support;

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

use support::corpus::{Corpus, CorpusSpec, IgnoreProfile};
use support::encodings::{self, Encoding};
use support::fixtures;

/// Run `loc <path> --format json` with an isolated config dir and parse the result.
fn scan_json(path: &Path) -> Value {
    let config_home = TempDir::new().expect("temp config dir");
    let output = Command::new(env!("CARGO_BIN_EXE_loc"))
        .arg(path)
        .args(["--format", "json"])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("HOME", config_home.path())
        .output()
        .expect("spawn loc");
    assert!(
        output.status.success(),
        "loc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("loc emitted valid JSON")
}

fn metadata_total(doc: &Value, key: &str) -> u64 {
    doc["metadata"][key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing metadata.{key}"))
}

fn scan_text_file(dir: &Path, name: &str, content: &str) -> Value {
    fs::write(dir.join(name), content).expect("write scan input");
    scan_json(&dir.join(name))
}

// Fixtures

#[test]
fn manifest_names_and_extensions_are_unique() {
    let names: HashSet<_> = fixtures::ALL.iter().map(|f| f.name).collect();
    let exts: HashSet<_> = fixtures::ALL.iter().map(|f| f.ext).collect();
    assert_eq!(names.len(), fixtures::ALL.len(), "duplicate fixture name");
    assert_eq!(
        exts.len(),
        fixtures::ALL.len(),
        "duplicate fixture extension"
    );
}

#[test]
fn sources_slice_excludes_container_formats() {
    let sources = fixtures::sources();
    assert_eq!(sources.len(), fixtures::SOURCE_COUNT);
    assert!(
        sources
            .iter()
            .all(|f| f.ext != ".html" && f.ext != ".ipynb")
    );
    assert_eq!(fixtures::get("html").ext, ".html");
    assert_eq!(fixtures::get("notebook").ext, ".ipynb");
}

#[test]
#[should_panic(expected = "unknown benchmark fixture")]
fn get_panics_loudly_on_unknown_name() {
    let _ = fixtures::get("cobol");
}

#[test]
fn every_fixture_is_authored_and_newline_terminated() {
    for f in fixtures::ALL {
        assert!(!f.content.trim().is_empty(), "{} is empty", f.name);
        assert!(
            f.content.ends_with('\n'),
            "{} lacks a trailing newline",
            f.name
        );
        assert!(
            f.content.contains("kelexine"),
            "{} lacks authorship",
            f.name
        );
    }
}

#[test]
fn notebook_fixture_is_valid_nbformat() {
    let doc: Value = serde_json::from_str(fixtures::get("notebook").content).expect("valid JSON");
    assert_eq!(doc["nbformat"], 4);
    assert!(doc["cells"].as_array().is_some_and(|c| c.len() >= 2));
}

// Scaling helpers

#[test]
fn repeat_to_lines_yields_whole_copies_meeting_the_target() {
    let content = "a\nb\nc\n";
    for target in [1, 3, 4, 10, 1_000] {
        let out = fixtures::repeat_to_lines(content, target);
        let lines = out.lines().count();
        assert!(lines >= target, "{lines} < {target}");
        assert_eq!(lines % 3, 0, "must be a whole number of copies");
        assert!(lines < target + 3, "must not overshoot by a full copy");
    }
}

#[test]
fn repeat_to_lines_handles_degenerate_input() {
    assert_eq!(fixtures::repeat_to_lines("", 100), "");
    assert_eq!(fixtures::repeat_to_lines("x\n", 0), "");
    // A fixture without a trailing newline must not fuse its last and first lines.
    let out = fixtures::repeat_to_lines("a\nb", 4);
    assert_eq!(out.lines().collect::<Vec<_>>(), ["a", "b", "a", "b"]);
}

#[test]
fn scale_notebook_multiplies_cells_and_stays_valid() {
    let base = fixtures::scale_notebook(1);
    let base_cells = serde_json::from_str::<Value>(&base).unwrap()["cells"]
        .as_array()
        .unwrap()
        .len();

    for copies in [2, 7] {
        let doc: Value = serde_json::from_str(&fixtures::scale_notebook(copies)).unwrap();
        assert_eq!(doc["cells"].as_array().unwrap().len(), base_cells * copies);
        assert_eq!(doc["nbformat"], 4, "metadata must be preserved");
    }
}

#[test]
fn scale_notebook_treats_zero_copies_as_one() {
    assert_eq!(fixtures::scale_notebook(0), fixtures::scale_notebook(1));
}

/// The invariant that makes repetition sound: every source fixture starts and ends
/// in a neutral lexer state, so N copies count as exactly N times one copy.
#[test]
fn repeating_a_source_fixture_scales_line_counts_linearly() {
    let dir = TempDir::new().unwrap();

    for f in fixtures::sources() {
        let per_copy = f.content.lines().count();
        let one = scan_text_file(dir.path(), &format!("one{}", f.ext), f.content);
        let three = scan_text_file(
            dir.path(),
            &format!("three{}", f.ext),
            &fixtures::repeat_to_lines(f.content, per_copy * 3),
        );

        for key in ["total_lines", "total_code", "total_comment", "total_blank"] {
            assert_eq!(
                metadata_total(&three, key),
                metadata_total(&one, key) * 3,
                "{}: {key} did not scale linearly",
                f.name
            );
        }
        assert!(
            metadata_total(&one, "total_comment") > 0,
            "{}: fixture should exercise comment counting",
            f.name
        );
    }
}

// Encodings

#[test]
fn encoding_labels_are_unique() {
    let labels: HashSet<_> = encodings::ALL.iter().map(|e| e.label()).collect();
    assert_eq!(labels.len(), encodings::ALL.len());
}

#[test]
fn bom_variants_carry_the_expected_prefix() {
    let text = "héllo\n";
    let cases: [(Encoding, &[u8]); 5] = [
        (Encoding::Utf8Bom, &[0xEF, 0xBB, 0xBF]),
        (Encoding::Utf16LeBom, &[0xFF, 0xFE]),
        (Encoding::Utf16BeBom, &[0xFE, 0xFF]),
        (Encoding::Utf32LeBom, &[0xFF, 0xFE, 0x00, 0x00]),
        (Encoding::Utf32BeBom, &[0x00, 0x00, 0xFE, 0xFF]),
    ];
    for (encoding, prefix) in cases {
        assert!(
            encodings::encode(text, encoding).starts_with(prefix),
            "{} has the wrong BOM",
            encoding.label()
        );
    }
    assert!(!encodings::encode(text, Encoding::Utf8).starts_with(&[0xEF, 0xBB, 0xBF]));
}

#[test]
fn wide_encodings_round_trip_including_non_bmp() {
    let text = "naïve 日本語 🚀\nsecond line\n";

    let le_bytes = encodings::encode(text, Encoding::Utf16LeBom);
    let (le_units, le_rest) = le_bytes[2..].as_chunks::<2>();
    assert!(le_rest.is_empty(), "no trailing partial UTF-16 code unit");
    let le: Vec<u16> = le_units.iter().map(|c| u16::from_le_bytes(*c)).collect();
    assert_eq!(String::from_utf16(&le).unwrap(), text);

    let be_bytes = encodings::encode(text, Encoding::Utf16BeBom);
    let (be_units, be_rest) = be_bytes[2..].as_chunks::<2>();
    assert!(be_rest.is_empty(), "no trailing partial UTF-16 code unit");
    let be: Vec<u16> = be_units.iter().map(|c| u16::from_be_bytes(*c)).collect();
    assert_eq!(String::from_utf16(&be).unwrap(), text);

    let utf32_bytes = encodings::encode(text, Encoding::Utf32LeBom);
    let (utf32_units, utf32_rest) = utf32_bytes[4..].as_chunks::<4>();
    assert!(
        utf32_rest.is_empty(),
        "no trailing partial UTF-32 code unit"
    );
    let utf32: String = utf32_units
        .iter()
        .map(|c| char::from_u32(u32::from_le_bytes(*c)).unwrap())
        .collect();
    assert_eq!(utf32, text);
}

#[test]
fn invalid_utf8_variant_is_actually_invalid_but_keeps_the_text() {
    let text = "let x = 1;\nlet y = 2;\n";
    let bytes = encodings::encode(text, Encoding::Utf8Invalid);
    assert!(std::str::from_utf8(&bytes).is_err());
    assert_eq!(
        String::from_utf8_lossy(&bytes).lines().count(),
        text.lines().count()
    );
}

#[test]
fn binary_variant_is_deterministic_nul_bearing_and_bom_free() {
    for len in [5, 64, 10_000] {
        let a = encodings::pseudo_random_bytes(len);
        assert_eq!(
            a,
            encodings::pseudo_random_bytes(len),
            "must be deterministic"
        );
        assert_eq!(a.len(), len);
        assert!(a.contains(&0), "needs a NUL inside the detection window");
        assert_ne!(a[0], 0xEF);
        assert_ne!(a[0], 0xFF);
        assert_ne!(a[0], 0xFE);
    }
    assert_eq!(
        encodings::encode("", Encoding::Binary).len(),
        64,
        "minimum length applies"
    );
    assert!(encodings::pseudo_random_bytes(0).is_empty());
}

// Corpus

fn spec(files: usize, dirs: usize, noise: bool, ignore: IgnoreProfile) -> CorpusSpec {
    CorpusSpec {
        files,
        dirs,
        lines_per_file: 30,
        noise,
        ignore,
    }
}

#[test]
fn corpus_without_noise_contains_exactly_the_requested_files() {
    let corpus = Corpus::build(spec(26, 4, false, IgnoreProfile::None)).unwrap();
    assert_eq!(corpus.visible_entries(), 26);
    assert!(!corpus.root().join(".locignore").exists());

    let doc = scan_json(corpus.root());
    assert_eq!(doc["files"].as_array().unwrap().len(), 26);
}

#[test]
fn corpus_visible_entries_matches_the_binary_for_every_profile() {
    for profile in [
        IgnoreProfile::None,
        IgnoreProfile::Simple,
        IgnoreProfile::WithNegation,
    ] {
        let corpus = Corpus::build(spec(30, 5, true, profile)).unwrap();
        let doc = scan_json(corpus.root());

        // The JSON `files` array lists countable files only; lockfiles and binaries
        // are surfaced by discovery but reported through `metadata` instead.
        let surfaced = doc["files"].as_array().unwrap().len() as u64
            + metadata_total(&doc, "lockfiles")
            + metadata_total(&doc, "binary_files");
        assert_eq!(
            surfaced,
            corpus.visible_entries() as u64,
            "profile {} disagrees with the scanner",
            profile.label()
        );
    }
}

#[test]
fn corpus_noise_is_classified_as_lockfile_and_binary() {
    let corpus = Corpus::build(spec(13, 3, true, IgnoreProfile::Simple)).unwrap();
    let doc = scan_json(corpus.root());
    assert_eq!(metadata_total(&doc, "lockfiles"), 1);
    assert_eq!(metadata_total(&doc, "binary_files"), 1);
}

#[test]
fn corpus_spreads_files_across_directories_and_tolerates_zero_dirs() {
    let corpus = Corpus::build(spec(9, 3, false, IgnoreProfile::None)).unwrap();
    let pkgs: HashSet<_> = fs::read_dir(corpus.root())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(pkgs.len(), 3);

    let flat = Corpus::build(spec(4, 0, false, IgnoreProfile::None)).unwrap();
    assert_eq!(flat.visible_entries(), 4);
    assert!(
        flat.root().join("pkg_000").is_dir(),
        "dirs=0 must behave as one directory"
    );
}

#[test]
fn corpus_is_removed_on_drop() {
    let root = {
        let corpus = Corpus::build(spec(3, 1, false, IgnoreProfile::None)).unwrap();
        corpus.root().to_path_buf()
    };
    assert!(!root.exists());
}

#[test]
fn corpus_notebooks_remain_valid_json_at_any_scale() {
    // 13 fixtures per cycle, so 13 files guarantees exactly one notebook per corpus.
    for lines in [1, 12, 500] {
        let corpus = Corpus::build(CorpusSpec {
            files: 13,
            dirs: 2,
            lines_per_file: lines,
            noise: false,
            ignore: IgnoreProfile::None,
        })
        .unwrap();

        let notebook = walk_files(corpus.root())
            .into_iter()
            .find(|p| p.extension().is_some_and(|e| e == "ipynb"))
            .expect("corpus must contain a notebook");
        let raw = fs::read_to_string(notebook).unwrap();
        serde_json::from_str::<Value>(&raw).expect("notebook must be valid JSON");
    }
}

fn walk_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}
