// tests/export.rs — Testing exporting to CSV, JSON, HTML

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
