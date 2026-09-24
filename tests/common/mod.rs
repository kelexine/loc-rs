// tests/common/mod.rs — Shared helpers for integration tests
// Author: kelexine (https://github.com/kelexine)

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
pub use tempfile::TempDir;

/// Resolve the compiled binary from the workspace target directory.
pub fn loc_bin() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("parent")
        .to_path_buf();

    if path.ends_with("deps") {
        path.pop();
    }
    path.join("loc")
}

/// List of all agent detection environment variables.
const AGENT_ENV_VARS: &[&str] = &[
    "ANTIGRAVITY_AGENT",
    "AUGMENT_AGENT",
    "CLINE_ACTIVE",
    "CLAUDE_CODE_IS_COWORK",
    "CLAUDECODE",
    "CLAUDE_CODE",
    "CODEX_SANDBOX",
    "CODEX_CI",
    "CODEX_THREAD_ID",
    "CRUSH",
    "GEMINI_CLI",
    "COPILOT_MODEL",
    "COPILOT_ALLOW_ALL",
    "COPILOT_GITHUB_TOKEN",
    "GOOSE_TERMINAL",
    "HERMES_SESSION_ID",
    "KILOCODE_FEATURE",
    "AGENT_CONTEXT_OUT",
    "OPENCLAW_SHELL",
    "OPENCODE_CLIENT",
    "PI_CODING_AGENT",
    "REPL_ID",
    "TRAE_AI_SHELL_ID",
    "ZED_TERM",
    "CURSOR_AGENT",
    "CURSOR_TRACE_ID",
    "AI_AGENT",
    "AGENT",
];

/// Execute the loc binary with given arguments.
pub fn run_loc(args: &[&str]) -> std::process::Output {
    run_loc_with_env(args, &HashMap::new())
}

/// Execute the loc binary with additional environment variables injected.
///
/// Used to test agent auto-detection without permanently mutating the test
/// process environment.
#[allow(dead_code)]
pub fn run_loc_with_env(args: &[&str], env: &HashMap<&str, &str>) -> std::process::Output {
    let mut cmd = std::process::Command::new(loc_bin());
    cmd.args(args);
    // Ensure agent detection vars from the outer test env don't bleed in.
    for key in AGENT_ENV_VARS {
        cmd.env_remove(key);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().expect("Failed to execute loc binary")
}

/// Create a temporary directory with a set of named files and content.
pub fn make_fixture(files: &[(&str, &str)]) -> TempDir {
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
