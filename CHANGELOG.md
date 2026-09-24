# Changelog

All notable changes to `loc-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Lossy UTF-8 fallback (`String::from_utf8_lossy`) for text files containing non-UTF-8 characters (e.g. ISO-8859/Latin-1 or legacy byte lookup tables).
- Unit test for non-UTF-8 encoded source file analysis.

### Changed
- Modularized `src/counter/mod.rs` into focused submodules:
  - `src/counter/lines.rs`: line counting, comment classification.
  - `src/counter/discovery.rs`: filesystem walking and ignore pruning.
  - `src/counter/git.rs`: libgit2 repo status, file enumeration, and revwalk commit history.
  - `src/counter/process.rs`: single-read buffer loading, binary checks, and UTF-8 handling.
- Optimized git index enumeration by skipping redundant `is_file()` syscalls on known regular blob index entries (`0o100000`).
- Cut scan time on large repos (82k+ files) from ~10.07s (14.48s sys time) down to ~2.88s (2.62s sys time).

## [0.2.9] - 2026-09-24

### Added
- Native libgit2 integration via `git2` crate (`default-features = false`).
- Repository detection using `git2::Repository::discover`.
- Index and status-based file enumeration replacing `git ls-files`.
- Direct commit history traversal using `git2::Revwalk` and tree diffs for `--git-dates`, eliminating `git log` shell-outs.
- Unit test suite for `git2` repository check, discovery, negation handling, and commit timestamps.

### Changed
- Removed external `git` process execution dependencies.
- Updated documentation across `ARCHITECTURE.md`, `README.md`, and `docs/FAQ.md` to reflect native git integration.

## [0.2.8] - 2026-09-24

### Added
- Testable agent harness detection via dependency injection.
- Modernized extractor match patterns.
- Upgraded dependencies (`colored`, `toml`, `dirs`).
