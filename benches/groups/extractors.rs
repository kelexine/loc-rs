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
    bench_nested(c);
    bench_minified_guard(c);
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

/// Sweeps nested function depths (50, 200, 500 levels of nesting) in JavaScript
/// to track the single-pass complexity index O(N) performance vs deep AST nesting.
fn bench_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("extractors/nested");
    let path = PathBuf::from("sample.js");
    let extractor = get_extractor(&path).expect("JavaScript extractor must exist");

    for depth in [50, 200, 500] {
        let text = (0..depth)
            .map(|i| format!("function f{i}(a, b) {{ if (a && b) {{ a++; }}\n"))
            .collect::<String>()
            + &"}\n".repeat(depth);

        assert!(
            !extractor.extract(&text).is_empty(),
            "nested JS fixture produced no functions at depth {depth}"
        );

        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("depth", depth), &text, |b, t| {
            b.iter(|| extractor.extract(black_box(t)));
        });
    }
    group.finish();
}

/// Benchmarks processing of a file containing a minified-length line (> 10,000 characters)
/// to track the fast-path minified line guard in process_file.
fn bench_minified_guard(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("bundle.min.js");
    let unit = "function a(b){if(b&&c){return b}else{return c||d}};";
    let content = unit.repeat(250) + "\n"; // ~13,000 chars on 1 line (> 10,000 guard)
    std::fs::write(&file_path, &content).expect("write temp file");

    let config = crate::counter::ScanConfig {
        target_dir: dir.path().to_path_buf(),
        target_paths: vec![dir.path().to_path_buf()],
        allowed_extensions: None,
        warn_size: None,
        parallel: false,
        extract_functions: true,
        locignore: crate::locignore::LocIgnore::empty(),
        include_hidden: false,
    };

    let mut group = c.benchmark_group("extractors/minified_guard");
    group.throughput(Throughput::Bytes(content.len() as u64));
    group.bench_function("minified_line_skip_ast", |b| {
        b.iter(|| {
            black_box(crate::counter::process::process_file(
                black_box(&file_path),
                black_box(&config),
            ))
            .unwrap()
        });
    });
    group.finish();
}
