// Author: kelexine <https://github.com/kelexine>
// groups/extractors.rs — Tree-sitter function extraction and AST complexity cost.
//
// Targets `extractors::get_extractor(..).extract(..)` for every bundled grammar.
// Extraction includes parsing, query execution and the per-node complexity walk.

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{BenchmarkId, Criterion, Throughput};

use crate::extractors::get_extractor;
use crate::support::fixtures;

const SIZES: [(&str, usize); 2] = [("500_lines", 500), ("10k_lines", 10_000)];

pub fn bench(c: &mut Criterion) {
    bench_extract(c);
    bench_dispatch(c);
}

fn bench_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("extractors/extract");

    for fixture in fixtures::sources() {
        let path = PathBuf::from(format!("sample{}", fixture.ext));
        let extractor = get_extractor(&path).unwrap_or_else(|| {
            panic!("no extractor registered for `{}`", fixture.ext);
        });

        for (label, lines) in SIZES {
            let text = fixtures::repeat_to_lines(fixture.content, lines);

            // Guard: an extractor that finds nothing would make the timing meaningless.
            assert!(
                !extractor.extract(&text).is_empty(),
                "extractor for `{}` found no functions in its fixture",
                fixture.name
            );

            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_with_input(BenchmarkId::new(fixture.name, label), &text, |b, t| {
                b.iter(|| extractor.extract(black_box(t)));
            });
        }
    }

    group.finish();
}

/// Cost of resolving a path to an extractor (allocates the extension string and,
/// for JS/TS, constructs the grammar handle).
fn bench_dispatch(c: &mut Criterion) {
    let paths: Vec<PathBuf> = fixtures::sources()
        .iter()
        .map(|f| PathBuf::from(format!("src/sample{}", f.ext)))
        .collect();

    let mut group = c.benchmark_group("extractors/dispatch");
    group.throughput(Throughput::Elements(paths.len() as u64));
    group.bench_function("resolve_all_sources", |b| {
        b.iter(|| {
            for path in &paths {
                black_box(get_extractor(black_box(path)));
            }
        });
    });
    group.finish();
}
