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
