// Author: kelexine <https://github.com/kelexine>
// groups/lines.rs — Throughput of the byte-level comment/blank/code tokenizer.
//
// Targets `counter::lines::analyze_content_with_spec`, the innermost loop of every scan.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};

use crate::counter::lines::analyze_content_with_spec;
use crate::language::COMMENT_REGISTRY;
use crate::support::fixtures;

/// (label, approximate line count) pairs benchmarked per language.
const SIZES: [(&str, usize); 2] = [("1k_lines", 1_000), ("50k_lines", 50_000)];

pub fn bench(c: &mut Criterion) {
    bench_languages(c);
    bench_pathological(c);
}

/// Every plain-source fixture at every size, using its real comment spec.
fn bench_languages(c: &mut Criterion) {
    let mut group = c.benchmark_group("lines/languages");

    for fixture in fixtures::sources() {
        let spec = COMMENT_REGISTRY.get(fixture.ext).unwrap_or_else(|| {
            panic!(
                "no comment spec registered for `{}`; the fixture would benchmark the no-spec path",
                fixture.ext
            )
        });

        for (label, lines) in SIZES {
            let text = fixtures::repeat_to_lines(fixture.content, lines);
            assert_partitioned(&text, Some(spec));

            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_with_input(BenchmarkId::new(fixture.name, label), &text, |b, t| {
                b.iter(|| analyze_content_with_spec(black_box(t), black_box(Some(spec))));
            });
        }
    }

    group.finish();
}

/// Inputs that stress specific tokenizer branches rather than typical code.
fn bench_pathological(c: &mut Criterion) {
    let rust = COMMENT_REGISTRY
        .get(".rs")
        .expect("Rust comment spec must be registered");

    let cases: Vec<(&str, String, Option<&crate::language::CommentSpec>)> = vec![
        (
            "no_spec_50k",
            fixtures::repeat_to_lines(fixtures::get("rust").content, 50_000),
            None,
        ),
        ("all_blank_50k", "\n".repeat(50_000), Some(rust)),
        (
            "all_line_comments_50k",
            "// filler comment line\n".repeat(50_000),
            Some(rust),
        ),
        (
            "single_1mib_line",
            "let a = \"x\"; ".repeat(1_048_576 / 13),
            Some(rust),
        ),
        (
            "nested_block_depth_2000",
            format!("{}\n{}\n", "/* ".repeat(2_000), "*/ ".repeat(2_000)),
            Some(rust),
        ),
    ];

    let mut group = c.benchmark_group("lines/pathological");
    for (label, text, spec) in &cases {
        assert_partitioned(text, *spec);

        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, t| {
            b.iter(|| analyze_content_with_spec(black_box(t), black_box(*spec)));
        });
    }
    group.finish();
}

/// Guard: the tokenizer must classify every line exactly once, and the input
/// must be non-empty, so a broken fixture can never yield a meaningless timing.
fn assert_partitioned(text: &str, spec: Option<&crate::language::CommentSpec>) {
    let (total, code, comment, blank) = analyze_content_with_spec(text, spec);
    assert!(total > 0, "benchmark input produced zero lines");
    assert_eq!(
        total,
        code + comment + blank,
        "every line must land in exactly one bucket"
    );
}
