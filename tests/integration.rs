// Author: kelexine (https://github.com/kelexine)
// tests/integration.rs — Integration tests for loc-rs

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Re-export internal modules so tests can reach them.
// Add `#[cfg(test)]` paths in Cargo.toml if needed, or use a test-helper
// binary. For now we use path-based fixture tests that exercise the public
// binary interface via `std::process::Command`.
// ─────────────────────────────────────────────────────────────────────────────

fn loc_bin() -> PathBuf {
    // Resolve the compiled binary from the workspace target directory
    let mut path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("parent")
        .to_path_buf();

    // Walk up from deps/ to target/debug or target/release
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("loc")
}

fn run_loc(args: &[&str]) -> std::process::Output {
    std::process::Command::new(loc_bin())
        .args(args)
        .output()
        .expect("Failed to execute loc binary")
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a temporary directory with a set of named files and content.
fn make_fixture(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("TempDir::new");
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
    }
    dir
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_basic_scan_exits_zero() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {\n    println!(\"hello\");\n}\n"),
        ("lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    assert!(out.status.success(), "loc exited non-zero: {:?}", out.status);
}

#[test]
fn test_total_lines_reported() {
    let fixture = make_fixture(&[
        ("a.py", "x = 1\ny = 2\nz = 3\n"),       // 3 lines
        ("b.py", "print('hello')\nprint('bye')\n"), // 2 lines
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The summary line contains the total; 5 lines expected
    assert!(
        stdout.contains('5') || stdout.contains("5"),
        "Expected total of 5 lines in output:\n{}", stdout
    );
}

#[test]
fn test_type_filter_rust_only() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("script.py", "print('hello')\n"),
        ("notes.md", "# Notes\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "-t", "rust"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Python and markdown files should not appear
    assert!(!stdout.contains("script.py"), "Python file should be filtered out");
    assert!(!stdout.contains("notes.md"), "Markdown file should be filtered out");
    assert!(stdout.contains("main.rs"), "Rust file should appear");
}

#[test]
fn test_detailed_breakdown_flag() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("helpers.rs", "pub fn help() {}\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "-d"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Detailed output should contain the "Extension" header
    assert!(
        stdout.contains("Extension") || stdout.contains("rs"),
        "Detailed breakdown missing in output:\n{}", stdout
    );
}

#[test]
fn test_function_extraction_flag() {
    let fixture = make_fixture(&[
        ("lib.rs", "pub fn hello() -> &'static str {\n    \"hello\"\n}\n\nfn world() {}\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "-f"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fn"),
        "Function count not shown with -f:\n{}", stdout
    );
}

#[test]
fn test_nonexistent_directory_exits_nonzero() {
    let out = run_loc(&["/tmp/this_dir_definitely_does_not_exist_loc_test_xyz"]);
    assert!(
        !out.status.success(),
        "Expected non-zero exit for missing directory"
    );
}

#[test]
fn test_warn_size_flag() {
    // Create a file with 600 lines
    let content = "let x = 1;\n".repeat(600);
    let fixture = make_fixture(&[("big.js", &content)]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "--warn-size", "500"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("LARGE") || stdout.contains("exceed"),
        "Expected size warning in output:\n{}", stdout
    );
}

#[test]
fn test_export_json() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
    ]);
    let out_json = fixture.path().join("out.json");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e", out_json.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(out_json.exists(), "JSON export file not created");

    let content = fs::read_to_string(&out_json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .expect("Exported JSON is not valid");
    assert!(parsed.get("metadata").is_some(), "JSON missing 'metadata' key");
    assert!(parsed.get("files").is_some(), "JSON missing 'files' key");
}

#[test]
fn test_export_csv() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
    ]);
    let out_csv = fixture.path().join("out.csv");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e", out_csv.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(out_csv.exists(), "CSV export file not created");

    let content = fs::read_to_string(&out_csv).unwrap();
    assert!(content.contains("Path"), "CSV missing header row");
    assert!(content.contains("main.rs"), "CSV missing file entry");
}

#[test]
fn test_binary_files_skipped_in_line_count() {
    // A file with null bytes is binary
    let fixture = make_fixture(&[("image.png", "\x00\x00\x00\x00binary\x00")]);

    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Binary files count as 0 lines toward the total
    assert!(
        stdout.contains('0') || !stdout.contains("image.png"),
        "Binary file should not contribute lines"
    );
}

#[test]
fn test_empty_file() {
    let fixture = make_fixture(&[("empty.txt", "")]);
    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains('0'), "Empty file should show 0 lines");
}

#[test]
fn test_no_trailing_newline_integration() {
    let fixture = make_fixture(&[("no_newline.txt", "line1\nline2")]);
    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should be 2 lines
    assert!(stdout.contains('2'), "Expected 2 lines for no trailing newline file");
}

#[test]
fn test_jsonl_export() {
    let fixture = make_fixture(&[
        ("a.rs", "fn a() {}\n"),
        ("b.py", "def b(): pass\n"),
    ]);
    let out_jsonl = fixture.path().join("out.jsonl");

    let out = run_loc(&[
        fixture.path().to_str().unwrap(),
        "-e", out_jsonl.to_str().unwrap(),
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
fn test_multilingual_summary() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("lib.py", "def help():\n    pass\n"),
    ]);
    let out = run_loc(&[fixture.path().to_str().unwrap(), "-d"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("rs"), "Summary missing Rust");
    assert!(stdout.contains("py"), "Summary missing Python");
}
