// Author: kelexine <https://github.com/kelexine>
// groups/embedded.rs — Embedded-language extraction cost (HTML blocks, Jupyter cells).
//
// Targets `counter::embedded::{parse_html_embedded, parse_jupyter_notebook}`: regex
// capture over markup and full JSON parsing respectively.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};

use crate::counter::embedded::{parse_html_embedded, parse_jupyter_notebook};
use crate::support::fixtures;

const HTML_LINES: [(&str, usize); 2] = [("200_lines", 200), ("20k_lines", 20_000)];
const NOTEBOOK_COPIES: [(&str, usize); 2] = [("10x_cells", 10), ("1000x_cells", 1_000)];

pub fn bench(c: &mut Criterion) {
    bench_html(c);
    bench_notebook(c);
}

fn bench_html(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded/html");
    let html = fixtures::get("html");

    for (label, lines) in HTML_LINES {
        let text = fixtures::repeat_to_lines(html.content, lines);

        // Guard: the fixture must actually contain embedded blocks to extract.
        let (total, _, _, _, chunks) = parse_html_embedded(&text, html.ext);
        assert!(total > 0, "html input produced no lines");
        assert!(!chunks.is_empty(), "html input produced no embedded chunks");

        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &text, |b, t| {
            b.iter(|| parse_html_embedded(black_box(t), black_box(html.ext)));
        });
    }

    group.finish();
}

fn bench_notebook(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedded/notebook");

    for (label, copies) in NOTEBOOK_COPIES {
        let text = fixtures::scale_notebook(copies);

        // Guard: parsing must succeed and yield both code and markdown chunks.
        let parsed = parse_jupyter_notebook(&text)
            .expect("scaled notebook fixture must remain a valid notebook");
        assert!(parsed.0 > 0, "notebook produced no lines");
        assert!(parsed.4.len() >= 2, "expected code and markdown chunks");

        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &text, |b, t| {
            b.iter(|| parse_jupyter_notebook(black_box(t)));
        });
    }

    group.finish();
}
