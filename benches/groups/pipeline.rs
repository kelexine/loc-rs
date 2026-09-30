// Author: kelexine <https://github.com/kelexine>
// groups/pipeline.rs — End-to-end in-process scan cost (`counter::run_scan`).
//
// Covers the rayon threshold (parallel only engages above 50 files), function
// extraction overhead, and the extension-filter fast path.

use std::collections::HashSet;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};

use crate::counter::{ScanConfig, run_scan};
use crate::language::resolve_extensions;
use crate::locignore::LocIgnore;
use crate::support::corpus::{Corpus, CorpusSpec, IgnoreProfile};

/// (label, files, dirs).  30 files stays below the parallel threshold on purpose.
const CORPORA: [(&str, usize, usize); 3] = [
    ("30_files", 30, 5),
    ("300_files", 300, 20),
    ("3000_files", 3_000, 60),
];

/// Behaviour switches for one benchmarked configuration.
#[derive(Clone, Copy)]
struct Mode {
    label: &'static str,
    parallel: bool,
    functions: bool,
    filter: bool,
}

const MODES: [Mode; 5] = [
    Mode {
        label: "parallel",
        parallel: true,
        functions: false,
        filter: false,
    },
    Mode {
        label: "sequential",
        parallel: false,
        functions: false,
        filter: false,
    },
    Mode {
        label: "parallel_functions",
        parallel: true,
        functions: true,
        filter: false,
    },
    Mode {
        label: "sequential_functions",
        parallel: false,
        functions: true,
        filter: false,
    },
    Mode {
        label: "parallel_type_filter",
        parallel: true,
        functions: false,
        filter: true,
    },
];

fn scan_config(corpus: &Corpus, mode: Mode) -> ScanConfig {
    let allowed_extensions = mode.filter.then(|| {
        ["rust", "python"]
            .iter()
            .flat_map(|lang| resolve_extensions(lang))
            .collect::<HashSet<String>>()
    });

    ScanConfig {
        target_dir: corpus.root().to_path_buf(),
        target_paths: Vec::new(),
        allowed_extensions,
        warn_size: None,
        parallel: mode.parallel,
        extract_functions: mode.functions,
        locignore: LocIgnore::build(corpus.root()),
        include_hidden: false,
    }
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline/run_scan");
    group.sample_size(20);

    for (label, files, dirs) in CORPORA {
        let corpus = Corpus::build(CorpusSpec {
            files,
            dirs,
            lines_per_file: 200,
            noise: true,
            ignore: IgnoreProfile::Simple,
        })
        .expect("failed to materialise pipeline corpus");

        for mode in MODES {
            let config = scan_config(&corpus, mode);

            // Guards: the scan must succeed, and an unfiltered scan must surface
            // exactly the entries the corpus promises.  A filtered scan must keep a
            // non-empty strict subset.
            let result = run_scan(&config).expect("guard scan must succeed");
            if mode.filter {
                assert!(
                    !result.files.is_empty() && result.files.len() < corpus.visible_entries(),
                    "type filter should keep a non-empty strict subset"
                );
            } else {
                assert_eq!(
                    result.files.len(),
                    corpus.visible_entries(),
                    "unexpected file count for {} / {label}",
                    mode.label
                );
            }
            if mode.functions {
                assert!(
                    result.files.iter().any(|f| f.function_count() > 0),
                    "function extraction produced no functions"
                );
            }

            group.throughput(Throughput::Elements(result.files.len() as u64));
            group.bench_function(BenchmarkId::new(mode.label, label), |b| {
                b.iter(|| run_scan(black_box(&config)));
            });
        }
    }

    group.finish();
}
