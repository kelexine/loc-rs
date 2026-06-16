// tests/export.rs — Testing exporting to CSV, JSON, HTML
// Author: kelexine (https://github.com/kelexine)

mod common;
use common::{make_fixture, run_loc};
use std::fs;

#[test]
fn test_export_json() {
    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let out_json = fixture.path().join("out.json");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_json.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(out_json.exists(), "JSON export file not created");

    let content = fs::read_to_string(&out_json).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("Exported JSON is not valid");
    assert!(
        parsed.get("metadata").is_some(),
        "JSON missing 'metadata' key"
    );
    assert!(parsed.get("files").is_some(), "JSON missing 'files' key");
}

#[test]
fn test_export_csv() {
    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let out_csv = fixture.path().join("out.csv");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_csv.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(out_csv.exists(), "CSV export file not created");

    let content = fs::read_to_string(&out_csv).unwrap();
    assert!(content.contains("Path"), "CSV missing header row");
    assert!(content.contains("main.rs"), "CSV missing file entry");
}

#[test]
fn test_jsonl_export() {
    let fixture = make_fixture(&[("a.rs", "fn a() {}\n"), ("b.py", "def b(): pass\n")]);
    let out_jsonl = fixture.path().join("out.jsonl");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_jsonl.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(out_jsonl.exists());

    let content = fs::read_to_string(&out_jsonl).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2, "Expected 2 JSON objects in JSONL export");
    for line in lines {
        let _: serde_json::Value = serde_json::from_str(line).expect("Invalid JSONL line");
    }
}

#[test]
fn test_export_html() {
    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let out_html = fixture.path().join("report.html");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_html.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(out_html.exists(), "HTML export file not created");

    let content = fs::read_to_string(&out_html).unwrap();
    assert!(content.contains("<!DOCTYPE html>"), "HTML missing doctype");
    assert!(
        content.contains("const reportData = {"),
        "HTML missing injected JSON data"
    );
    assert!(content.contains("main.rs"), "HTML missing file data");
}

// ── Lockfile exclusion in JSON export ────────────────────────────────────────

#[test]
fn test_export_json_excludes_lockfiles_from_files_array() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("Cargo.lock", "version = 3\n[[package]]\n"),
    ]);
    let out_json = fixture.path().join("out.json");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_json.to_str().unwrap(),
    ]);
    assert!(out.status.success());

    let content = fs::read_to_string(&out_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    let files = parsed["files"].as_array().expect("files must be array");
    // Cargo.lock must not appear in the files array
    assert!(
        !files
            .iter()
            .any(|f| f["path"].as_str().unwrap_or("").contains("Cargo.lock")),
        "Cargo.lock must not appear in export JSON files array: {content}"
    );
}

#[test]
fn test_export_json_metadata_has_lockfile_count() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("Cargo.lock", "version = 3\n"),
        ("yarn.lock", "# yarn lockfile v1\n"),
    ]);
    let out_json = fixture.path().join("out.json");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_json.to_str().unwrap(),
    ]);
    assert!(out.status.success());

    let content = fs::read_to_string(&out_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    let meta = &parsed["metadata"];
    assert!(
        meta.get("lockfiles").is_some(),
        "metadata must include 'lockfiles' key"
    );
    assert_eq!(
        meta["lockfiles"].as_u64().unwrap(),
        2,
        "lockfile count must be 2"
    );
    // Alignment check: all stat keys present (matches print_json_stats shape)
    for key in &[
        "total_lines",
        "total_code",
        "total_comment",
        "total_blank",
        "total_files",
        "binary_files",
        "lockfiles",
    ] {
        assert!(
            meta.get(key).is_some(),
            "metadata missing key '{key}'"
        );
    }
}

#[test]
fn test_export_json_file_objects_include_is_lockfile_field() {
    // Even though lockfiles are excluded from the files array, regular source
    // files must carry the is_lockfile field (false) so consumers can rely on
    // a stable schema.
    let fixture = make_fixture(&[("app.py", "print('hi')\n")]);
    let out_json = fixture.path().join("out.json");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_json.to_str().unwrap(),
    ]);
    assert!(out.status.success());

    let content = fs::read_to_string(&out_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let files = parsed["files"].as_array().unwrap();
    assert!(!files.is_empty(), "expected at least one file entry");
    for f in files {
        assert!(
            f.get("is_lockfile").is_some(),
            "file object missing 'is_lockfile' field: {f}"
        );
    }
}

#[test]
fn test_jsonl_excludes_lockfiles() {
    let fixture = make_fixture(&[
        ("a.rs", "fn a() {}\n"),
        ("Cargo.lock", "version = 3\n"),
    ]);
    let out_jsonl = fixture.path().join("out.jsonl");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e",
        out_jsonl.to_str().unwrap(),
    ]);
    assert!(out.status.success());

    let content = fs::read_to_string(&out_jsonl).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    // Only a.rs should appear — Cargo.lock is excluded
    assert_eq!(lines.len(), 1, "JSONL must contain exactly 1 record (lockfile excluded)");
    let record: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert!(
        record["path"].as_str().unwrap_or("").contains("a.rs"),
        "JSONL record must be a.rs"
    );
}

#[test]
fn test_stdout_json_excludes_lockfiles_from_files_array() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("Cargo.lock", "version = 3\n"),
    ]);

    let out = run_loc(&["--json", fixture.path().to_str().unwrap()]);
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("--json stdout must be valid JSON");

    let files = parsed["files"].as_array().expect("files must be array");
    assert!(
        !files
            .iter()
            .any(|f| f["path"].as_str().unwrap_or("").contains("Cargo.lock")),
        "Cargo.lock must not appear in --json files array"
    );
}

// ── Lockfile exclusion in JSON export ────────────────────────────────────────
