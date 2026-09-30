// Author: kelexine <https://github.com/kelexine>
// support/mod.rs — Shared benchmark inputs: committed fixtures, encodings, corpora.
//
// Deliberately free of any dependency on loc-rs internals so integration tests can
// mount this module directly via `#[path]` and validate it in isolation.

#![allow(dead_code)]

pub mod corpus;
pub mod encodings;
pub mod fixtures;
