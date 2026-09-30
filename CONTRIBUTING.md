# Contributing to loc-rs

Thanks for contributing.

## Prerequisites

- Rust `1.92.0` or newer
- `git`

## Local Setup

```bash
git clone https://github.com/kelexine/loc-rs
cd loc-rs
cargo build
```

## Development Commands

```bash
cargo fmt --all
cargo clippy --all-targets --all-features
cargo test
```

Run the binary locally:

```bash
cargo run -- -d
cargo run -- -d -f
```

## Benchmarks

Criterion benchmarks live in `benches/`. Run them when touching anything on the
scan path (tokenizer, encoding, extractors, discovery, `run_scan`).

```bash
cargo bench                                    # everything
cargo bench --bench loc_bench -- lines/        # one group (name prefix filter)
cargo bench --bench loc_bench -- --test        # smoke pass: run each bench once
cargo bench --bench loc_bench -- --save-baseline before   # then compare with
cargo bench --bench loc_bench -- --baseline before        # after your change
```

Groups: `lines`, `encoding`, `embedded`, `extractors`, `parse`, `discovery`, `pipeline`, `e2e`.

Rules for benchmark code:

- Inputs come from the committed files in `benches/fixtures/`; larger inputs are
  built by repeating them (`benches/support/`). Add a new language by adding a
  fixture and one entry in `benches/support/fixtures.rs`.
- Every fixture starts and ends in a neutral lexer state (no unterminated block
  comment or string) so repetition scales counts linearly. Repeated copies must
  also parse without syntax errors (for example PHP closes its tag with `?>`);
  the `parse` group asserts this, because error recovery skews timings.
- Every bench asserts its setup (expected counts, classification) before timing,
  so a broken input fails loudly instead of producing a meaningless number.
- `benches/support/` must not depend on `loc-rs` internals; it is validated by
  `tests/bench_support.rs` (`cargo test`).
- Keep `cargo clippy --all-targets --all-features` clean; it covers `benches/`.

## Project Conventions

- Keep behavior changes covered by tests.
- Prefer explicit errors over silent fallback when behavior would be ambiguous.
- Keep CLI help text and README aligned with implementation.

## Pull Requests

Before opening a PR:

1. Ensure `cargo test` passes.
2. Ensure `cargo clippy --all-targets --all-features` is clean or warnings are justified.
3. Update docs when flags, behavior, or output format changes.

Recommended commit style:

```text
<type>(<scope>): <summary>
```

Examples:

- `fix(display): hide function metrics unless extraction is enabled`
- `docs(readme): refresh configuration and workflow examples`

## Release Workflow

`loc-rs` uses `cargo release`.

Prepare local release (no publish/push):

```bash
cargo release patch --no-publish --no-push --execute --no-confirm
```

Publish prepared release:

```bash
cargo release publish --execute --no-confirm
```

Push `main` and tag:

```bash
git push origin main refs/tags/vX.Y.Z
```
