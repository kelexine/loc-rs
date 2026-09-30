// Author: kelexine <https://github.com/kelexine>
// groups/e2e.rs — Whole-process cost of the shipped `loc` binary.
//
// Measures what a user pays per invocation: process spawn, arg parsing, scan,
// rendering and exit.  The binary path comes from `CARGO_BIN_EXE_loc`, which
// Cargo sets for benchmarks and builds with the release-derived bench profile.

use std::path::Path;
use std::process::{Command, Stdio};

use criterion::{BenchmarkId, Criterion, Throughput};
use tempfile::TempDir;

use crate::support::corpus::{Corpus, CorpusSpec, IgnoreProfile};

const LOC_BIN: &str = env!("CARGO_BIN_EXE_loc");

/// A labelled argument set (the corpus path is prepended automatically).
struct Invocation {
    label: &'static str,
    args: &'static [&'static str],
    expects_json: bool,
}

const INVOCATIONS: [Invocation; 5] = [
    Invocation {
        label: "json",
        args: &["--format", "json"],
        expects_json: true,
    },
    Invocation {
        label: "json_sequential",
        args: &["--format", "json", "--no-parallel"],
        expects_json: true,
    },
    Invocation {
        label: "agent_detailed",
        args: &["--format", "agent", "-d"],
        expects_json: false,
    },
    Invocation {
        label: "agent_functions",
        args: &["--format", "agent", "-d", "-f"],
        expects_json: false,
    },
    Invocation {
        label: "quiet",
        args: &["--format", "quiet"],
        expects_json: false,
    },
];

fn run(root: &Path, config_home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(LOC_BIN)
        .arg(root)
        .args(args)
        .env("XDG_CONFIG_HOME", config_home)
        .env("HOME", config_home)
        .stdin(Stdio::null())
        .output()
        .expect("failed to spawn the loc binary")
}

pub fn bench(c: &mut Criterion) {
    let files = 500;
    let corpus = Corpus::build(CorpusSpec {
        files,
        dirs: 25,
        lines_per_file: 200,
        noise: true,
        ignore: IgnoreProfile::Simple,
    })
    .expect("failed to materialise e2e corpus");
    let config_home = TempDir::new().expect("failed to create isolated config dir");

    let mut group = c.benchmark_group("e2e/binary");
    group.sample_size(20);
    group.throughput(Throughput::Elements(corpus.visible_entries() as u64));

    for invocation in &INVOCATIONS {
        // Guard: a failing or silent invocation would time an error path.
        let output = run(corpus.root(), config_home.path(), invocation.args);
        assert!(
            output.status.success(),
            "`loc {}` failed: {}",
            invocation.label,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !output.stdout.is_empty(),
            "`loc {}` produced no output",
            invocation.label
        );
        if invocation.expects_json {
            serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .unwrap_or_else(|e| panic!("`loc {}` emitted invalid JSON: {e}", invocation.label));
        }

        group.bench_function(BenchmarkId::from_parameter(invocation.label), |b| {
            b.iter(|| run(corpus.root(), config_home.path(), invocation.args));
        });
    }

    group.finish();
}
