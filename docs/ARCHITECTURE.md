# loc-rs Architecture

This document explains how `loc-rs` is organized and how data flows through the scan pipeline.

## High-Level Flow

1. `src/main.rs` parses CLI args.
2. `src/counter/mod.rs` builds scan config and discovers files.
3. Files are classified as text/binary and analyzed for line metrics.
4. Optional function extraction runs per supported language.
5. `src/display/mod.rs` renders terminal output.
6. `src/export/*` writes JSON/JSONL/CSV/HTML reports when requested.

## Module Responsibilities

- `src/cli/mod.rs`
  - Clap argument definitions and help text.
- `src/config/mod.rs`
  - Global config loading from platform config directory.
- `src/language/mod.rs`
  - Language-extension mapping, alias resolution, comment specs, default exclusions.
- `src/counter/mod.rs`
  - File discovery, content analysis, binary detection, git integration, scan orchestration.
- `src/extractors/*`
  - Tree-sitter-backed per-language function extraction implementations.
- `src/models/mod.rs`
  - Core data structures (`FileInfo`, `FunctionInfo`, `ScanResult`, extension stats).
- `src/display/mod.rs`
  - Summary, breakdown, tree view, and function analysis output.
- `src/export/*`
  - Report serialization and export format handling.

## Discovery Strategy

- In git repos (default): uses `git ls-files` to align with tracked/unignored files.
- Outside git: uses recursive filesystem walk with exclusion sets.
- `.locignore` allows project-specific ignores.
- Hidden files are skipped by default unless `--include-hidden` is used.

## Function Extraction Model

- Extraction is opt-in (`-f` or `--func-analysis`, or config default).
- Extractors parse ASTs via Tree-sitter grammar crates.
- Output includes name, line range, method/class flags, and complexity estimate.
- Complexity is a keyword-based cyclomatic approximation used consistently across extractors.

## Output Model

- Summary metrics always include total lines, text file count, and code/comment/blank splits.
- Function/class lines appear only when extraction is enabled.
- Detailed breakdown (`-d`) includes per-extension metrics, plus function counts when extraction is enabled.

## Testing Surface

- Unit tests: comment parsing, binary detection, extension resolution, extractor behavior.
- Integration tests: CLI behavior and export format validity.
- Key integration files:
  - `tests/cli.rs`
  - `tests/core.rs`
  - `tests/export.rs`
