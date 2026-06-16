// tests/cli.rs — Testing CLI flags, outputs, and errors

mod common;
use common::{make_fixture, run_loc, run_loc_with_env};

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

    // -d (detailed) is required to render the breakdown table where extensions appear.
    // Without it the summary shows totals only — no per-extension or per-file names.
    let out = run_loc(&[fixture.path().to_str().unwrap(), "-t", "rust", "-d"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Python and markdown files should not appear in the breakdown
    assert!(
        !stdout.contains("py"),
        "Python extension should be filtered out:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("md"),
        "Markdown extension should be filtered out:\n{}",
        stdout
    );
    // The "rs" extension row must appear in the detailed breakdown
    assert!(
        stdout.contains("rs"),
        "Rust extension should appear in breakdown:\n{}",
        stdout
    );
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
        stdout.contains("Functions"),
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

// ─── Agent / format mode tests ────────────────────────────────────────────────

#[test]
fn test_format_agent_produces_tsv_summary() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {\n    println!(\"hi\");\n}\n"),
        ("lib.rs",  "pub fn add(a: i32, b: i32) -> i32 { a + b }\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "--format", "agent"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# SUMMARY\n"), "Missing SUMMARY section:\n{}", stdout);
    assert!(stdout.contains("metric\tvalue\n"), "Missing TSV header:\n{}", stdout);
    assert!(stdout.contains("total_lines\t"), "Missing total_lines:\n{}", stdout);
    assert!(stdout.contains("total_code\t"), "Missing total_code:\n{}", stdout);
}

#[test]
fn test_format_agent_with_detailed_produces_breakdown_section() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("util.py", "def foo(): pass\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "--format", "agent", "-d"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# BREAKDOWN\n"), "Missing BREAKDOWN section:\n{}", stdout);
    assert!(stdout.contains("extension\tfiles\t"), "Missing breakdown header:\n{}", stdout);
}

#[test]
fn test_format_agent_with_tree_produces_files_section() {
    let fixture = make_fixture(&[("src/main.rs", "fn main() {}\n")]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "--format", "agent", "--tree"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# FILES\n"), "Missing FILES section:\n{}", stdout);
    assert!(stdout.contains("path\tlines\t"), "Missing file header:\n{}", stdout);
}

#[test]
fn test_format_agent_no_ansi_codes() {
    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "--format", "agent"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // ANSI escape codes start with ESC (\x1b)
    assert!(
        !stdout.contains('\x1b'),
        "Agent output must not contain ANSI escape codes:\n{}",
        stdout
    );
}

#[test]
fn test_quiet_flag_prints_one_path_per_line() {
    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("lib.rs",  "pub fn foo() {}\n"),
    ]);

    let out = run_loc(&[fixture.path().to_str().unwrap(), "-q"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "Expected 2 paths, got:\n{}", stdout);
    assert!(
        lines.iter().all(|l| l.ends_with(".rs")),
        "All lines should be .rs paths:\n{}",
        stdout
    );
}

#[test]
fn test_quiet_long_flag_equivalent_to_short() {
    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);

    let short = run_loc(&[fixture.path().to_str().unwrap(), "-q"]);
    let long  = run_loc(&[fixture.path().to_str().unwrap(), "--quiet"]);
    assert_eq!(
        String::from_utf8_lossy(&short.stdout),
        String::from_utf8_lossy(&long.stdout),
        "--quiet and -q must produce identical output"
    );
}

#[test]
fn test_format_json_equivalent_to_json_flag() {
    use std::collections::HashMap;

    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let path = fixture.path().to_str().unwrap();

    let via_flag   = run_loc_with_env(&[path, "--json"], &HashMap::new());
    let via_format = run_loc_with_env(&[path, "--format", "json"], &HashMap::new());

    assert!(via_flag.status.success());
    assert!(via_format.status.success());

    // Strip the timestamp field before comparing — two separate process
    // invocations will have different wall-clock timestamps.
    let strip_timestamp = |s: &str| -> String {
        // Replace "timestamp":"<value>" with a stable placeholder.
        let re = regex::Regex::new(r#""timestamp":"[^"]*""#).unwrap();
        re.replace_all(s, r#""timestamp":"<stripped>""#).to_string()
    };

    let flag_json   = strip_timestamp(&String::from_utf8_lossy(&via_flag.stdout));
    let format_json = strip_timestamp(&String::from_utf8_lossy(&via_format.stdout));
    assert_eq!(flag_json, format_json, "--json and --format json must produce identical stdout");
}

#[test]
fn test_agent_auto_detected_from_env_var() {
    use std::collections::HashMap;

    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let mut env = HashMap::new();
    env.insert("CRUSH", "1");

    let out = run_loc_with_env(&[fixture.path().to_str().unwrap()], &env);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("# SUMMARY"),
        "CRUSH env var should trigger agent mode:\n{}",
        stdout
    );
}

#[test]
fn test_format_human_overrides_agent_env_var() {
    use std::collections::HashMap;

    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let mut env = HashMap::new();
    env.insert("CRUSH", "1");

    let out = run_loc_with_env(&[fixture.path().to_str().unwrap(), "--format", "human"], &env);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("# SUMMARY"),
        "--format human must override agent auto-detection:\n{}",
        stdout
    );
    assert!(
        stdout.contains("LOC-RS ANALYSIS SUMMARY"),
        "Human mode header expected:\n{}",
        stdout
    );
}

#[test]
fn test_export_tsv_creates_valid_file() {
    use std::collections::HashMap;

    let fixture = make_fixture(&[
        ("main.rs", "fn main() {}\n"),
        ("lib.rs",  "pub fn foo() {}\n"),
    ]);
    let out_path = fixture.path().join("out.tsv");

    let out = run_loc_with_env(
        &[fixture.path().to_str().unwrap(), "-e", out_path.to_str().unwrap()],
        &HashMap::new(),
    );
    assert!(out.status.success(), "loc exited non-zero: {:?}", out.status);
    assert!(out_path.exists(), "out.tsv was not created");

    let contents = std::fs::read_to_string(&out_path).unwrap();
    assert!(contents.contains("# SUMMARY"),   "Missing SUMMARY in TSV:\n{}", contents);
    assert!(contents.contains("# BREAKDOWN"), "Missing BREAKDOWN in TSV:\n{}", contents);
    assert!(contents.contains("# FILES"),     "Missing FILES in TSV:\n{}", contents);
}

#[test]
fn test_hints_go_to_stderr_not_stdout() {
    use std::collections::HashMap;

    let fixture = make_fixture(&[("main.rs", "fn main() {}\n")]);
    let out = run_loc_with_env(
        &[fixture.path().to_str().unwrap(), "--format", "agent"],
        &HashMap::new(),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stdout.contains("Hint:"), "Hints must not appear on stdout:\n{}", stdout);
    assert!(stderr.contains("Hint:"), "Hints must appear on stderr:\n{}", stderr);
}
