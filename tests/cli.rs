// tests/cli.rs — Testing CLI flags, outputs, and errors

mod common;
use common::{make_fixture, run_loc};

#[test]
fn test_basic_scan_exits_zero() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {\n    println!(\"hello\");\n}\n"),
        ("lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    assert!(
        out.status.success(),
        "loc exited non-zero: {:?}",
        out.status
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
    assert!(
        stdout.contains("Extension") || stdout.contains("rs"),
        "Detailed breakdown missing in output:\n{}",
        stdout
    );
}

#[test]
fn test_function_extraction_flag() {
    let fixture = make_fixture(&[(
        "lib.rs",
        "pub fn hello() -> &'static str {\n    \"hello\"\n}\n\nfn world() {}\n",
    )]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "-f"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fn"),
        "Function count not shown with -f:\n{}",
        stdout
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
    let content = "let x = 1;\n".repeat(600);
    let fixture = make_fixture(&[("big.js", &content)]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "--warn-size", "500"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("LARGE") || stdout.contains("exceed"),
        "Expected size warning in output:\n{}",
        stdout
    );
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
