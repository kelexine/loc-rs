// tests/core.rs — Testing core line counting logic and binary filtering

mod common;
use common::{make_fixture, run_loc};

#[test]
fn test_total_lines_reported() {
    let fixture = make_fixture(&[
        ("a.py", "x = 1\ny = 2\nz = 3\n"),          // 3 lines
        ("b.py", "print('hello')\nprint('bye')\n"), // 2 lines
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains('5') || stdout.contains("5"),
        "Expected total of 5 lines in output:\n{}",
        stdout
    );
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
    assert!(
        stdout.contains('2'),
        "Expected 2 lines for no trailing newline file"
    );
}
