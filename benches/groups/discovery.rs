// Author: kelexine <https://github.com/kelexine>
// groups/discovery.rs — Ignore-rule compilation and filesystem traversal.
//
// Targets `locignore::LocIgnore::build` and `counter::discovery::get_manual_files`
// across ignore profiles (none / simple / with negations, which disables directory
// pruning) and tree sizes.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};

use crate::counter::discovery::get_manual_files;
use crate::locignore::LocIgnore;
use crate::support::corpus::{Corpus, CorpusSpec, IgnoreProfile};

const TREE_SIZES: [(&str, usize, usize); 2] = [("100_files", 100, 10), ("2000_files", 2_000, 50)];
const PROFILES: [IgnoreProfile; 3] = [
    IgnoreProfile::None,
    IgnoreProfile::Simple,
    IgnoreProfile::WithNegation,
];

/// A materialised corpus plus its precompiled ignore rules and guarded entry count.
struct Case {
    profile: IgnoreProfile,
    size_label: &'static str,
    corpus: Corpus,
    locignore: LocIgnore,
    entries: usize,
}

fn prepare() -> Vec<Case> {
    let mut cases = Vec::new();

    for (size_label, files, dirs) in TREE_SIZES {
        for profile in PROFILES {
            let corpus = Corpus::build(CorpusSpec {
                files,
                dirs,
                lines_per_file: 20,
                noise: true,
                ignore: profile,
            })
            .expect("failed to materialise discovery corpus");

            // Guard: traversal must surface exactly the entries the corpus promises,
            // proving pruning and ignore rules behaved as the profile intends.
            let locignore = LocIgnore::build(corpus.root());
            let entries = get_manual_files(corpus.root(), &locignore, false).len();
            assert_eq!(
                entries,
                corpus.visible_entries(),
                "unexpected entry count for {} / {size_label}",
                profile.label()
            );

            cases.push(Case {
                profile,
                size_label,
                corpus,
                locignore,
                entries,
            });
        }
    }

    cases
}

pub fn bench(c: &mut Criterion) {
    let cases = prepare();

    let mut build = c.benchmark_group("discovery/locignore_build");
    for case in &cases {
        let root = case.corpus.root();
        build.bench_function(
            BenchmarkId::new(case.profile.label(), case.size_label),
            |b| {
                b.iter(|| LocIgnore::build(black_box(root)));
            },
        );
    }
    build.finish();

    let mut walk = c.benchmark_group("discovery/walk");
    for case in &cases {
        let root = case.corpus.root();
        walk.throughput(Throughput::Elements(case.entries as u64));
        walk.bench_function(
            BenchmarkId::new(case.profile.label(), case.size_label),
            |b| {
                b.iter(|| get_manual_files(black_box(root), black_box(&case.locignore), false));
            },
        );
    }
    walk.finish();
}
