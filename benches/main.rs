//! Author: kelexine <https://github.com/kelexine>
//!
//! Criterion benchmark suite for loc-rs.
//!
//! `loc-rs` is a binary-only crate, so its internals are not importable from
//! `benches/`.  Rather than splitting out a library target, this crate root mounts
//! the pipeline modules straight from `src/` with `#[path]`; `src/` stays
//! untouched and the benchmarks exercise the exact code the binary ships.
//! Presentation modules (`agent`, `display`, `export`) are intentionally not
//! mounted: nothing on the scan path depends on them.
//!
#![allow(dead_code, unused_imports, missing_docs)]

#[path = "../src/cli/mod.rs"]
mod cli;
#[path = "../src/config/mod.rs"]
mod config;
#[path = "../src/counter/mod.rs"]
mod counter;
#[path = "../src/extractors/mod.rs"]
mod extractors;
#[path = "../src/language/mod.rs"]
mod language;
#[path = "../src/locignore/mod.rs"]
mod locignore;
#[path = "../src/models/mod.rs"]
mod models;

mod groups;
mod support;

use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets =
        groups::lines::bench,
        groups::encoding::bench,
        groups::embedded::bench,
        groups::extractors::bench,
        groups::parse::bench,
        groups::discovery::bench,
        groups::pipeline::bench,
        groups::e2e::bench,
}
criterion_main!(benches);
