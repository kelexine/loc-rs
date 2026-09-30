// Author: kelexine <https://github.com/kelexine>
// groups/encoding.rs — Encoding detection and decoding cost.
//
// Targets `counter::process::{detect_encoding, decode_text_buffer}`, which run on
// every non-binary-extension file before any line is counted.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};

use crate::counter::process::{TextEncoding, decode_text_buffer, detect_encoding};
use crate::support::encodings::{self, Encoding};
use crate::support::fixtures;

const LINES: usize = 5_000;

/// The classification `detect_encoding` is required to return for each variant.
fn expected(encoding: Encoding) -> TextEncoding {
    match encoding {
        Encoding::Utf8 | Encoding::Utf8Invalid => TextEncoding::Utf8,
        Encoding::Utf8Bom => TextEncoding::Utf8Bom,
        Encoding::Utf16LeBom | Encoding::Utf16LeNoBom => TextEncoding::Utf16Le,
        Encoding::Utf16BeBom => TextEncoding::Utf16Be,
        Encoding::Utf32LeBom => TextEncoding::Utf32Le,
        Encoding::Utf32BeBom => TextEncoding::Utf32Be,
        Encoding::Binary => TextEncoding::Binary,
    }
}

/// Encode every variant and verify it exercises the intended path.
fn prepare() -> Vec<(Encoding, Vec<u8>)> {
    let text = fixtures::repeat_to_lines(fixtures::get("rust").content, LINES);
    let source_lines = text.lines().count();

    encodings::ALL
        .iter()
        .map(|&encoding| {
            let bytes = encodings::encode(&text, encoding);

            // Guards: the variant must be classified as intended and decode to the
            // same number of lines, otherwise the timing measures the wrong path.
            assert_eq!(
                detect_encoding(&bytes),
                expected(encoding),
                "unexpected classification for {}",
                encoding.label()
            );
            match (encoding, decode_text_buffer(&bytes)) {
                (Encoding::Binary, None) => {}
                (Encoding::Binary, Some(_)) => panic!("binary variant decoded as text"),
                (_, Some(decoded)) => assert_eq!(
                    decoded.lines().count(),
                    source_lines,
                    "line count drifted while decoding {}",
                    encoding.label()
                ),
                (_, None) => panic!("{} unexpectedly classified as binary", encoding.label()),
            }

            (encoding, bytes)
        })
        .collect()
}

pub fn bench(c: &mut Criterion) {
    let variants = prepare();

    let mut detect = c.benchmark_group("encoding/detect");
    for (encoding, bytes) in &variants {
        detect.throughput(Throughput::Bytes(bytes.len() as u64));
        detect.bench_with_input(
            BenchmarkId::from_parameter(encoding.label()),
            bytes,
            |b, buf| {
                b.iter(|| detect_encoding(black_box(buf)));
            },
        );
    }
    detect.finish();

    let mut decode = c.benchmark_group("encoding/decode");
    for (encoding, bytes) in &variants {
        decode.throughput(Throughput::Bytes(bytes.len() as u64));
        decode.bench_with_input(
            BenchmarkId::from_parameter(encoding.label()),
            bytes,
            |b, buf| {
                b.iter(|| decode_text_buffer(black_box(buf)));
            },
        );
    }
    decode.finish();
}
