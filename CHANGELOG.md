# Changelog

All notable changes to `loc-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Native git integration via `git2` crate replacing git CLI shell-outs.

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
