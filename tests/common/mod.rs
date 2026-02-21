// tests/common/mod.rs — Shared helpers for integration tests

use std::fs;
use std::path::PathBuf;
pub use tempfile::TempDir;

/// Resolve the compiled binary from the workspace target directory
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

/// Execute the loc binary with given arguments
pub fn run_loc(args: &[&str]) -> std::process::Output {
    std::process::Command::new(loc_bin())
        .args(args)
        .output()
        .expect("Failed to execute loc binary")
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
