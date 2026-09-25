# Changelog

All notable changes to `loc-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.20] - 2026-09-26

### Performance
- Fast-path git repository check: replaced heavy `git2::Repository::discover` traversal with direct filesystem parent directory checks for `.git`, reducing repository discovery latency from ~13.8ms to <7µs.
- Optimized small repository discovery: routed directory scans through lightweight `WalkDir` combined with pre-compiled `LocIgnore` rules, eliminating costly `repo.statuses()` working-tree diffing and cryptographic hashing during plain scans. Reduced discovery latency from ~14.8ms to <300µs and total scan time on small repos to ~3-11ms (outperforming `tokei`).

### Changed
- Aligned generator identity: updated export metadata in JSON and HTML reports from `loc v<version>` to `loc-rs v<version>`.

## [0.2.19] - 2026-09-26

### Added
- Strict JSON encapsulation: in JSON mode (`--json` or `--format json`), all warnings and hints are now first-class attributes (`"warnings"` and `"hints"`) of the JSON object, completely eliminating rogue stderr/stdout output.
- Gated agent hints: terminal hints to stderr are now strictly constrained to invocations where an AI coding agent is genuinely auto-detected from the environment. Plain CLI and manual `--format agent` runs omit hints.

## [0.2.18] - 2026-09-26

### Fixed
- Fixed display logic in human output mode so that the per-language breakdown table is only rendered when `-d` or `--detailed` is specified, keeping default `loc` execution concise with the summary overview table.

## [0.2.17] - 2026-09-25

### Documentation
- Updated CLI `--help` menu: expanded the `SUPPORTED LANGUAGES` reference with all newly added languages (Device Tree, C/C++ Headers, Assembly, Linker Scripts, Coccinelle, Bison/Flex, Android Blueprint, Config, Kconfig, Shaders, etc.) and updated the tool identity to `loc-rs`.

## [0.2.16] - 2026-09-25

### Changed
- Differentiate CLI identity as `loc-rs` while retaining the `loc` binary command (`loc --version` outputs `loc-rs 0.2.16`, distinguishing it from the older `loc` tool by cgag).

## [0.2.15] - 2026-09-25

### Added
- Canonical full language names across all reports: breakdown table now reports "Language" (e.g. `Rust`, `C`, `C Header`, `C++`, `C++ Header`, `Device Tree`, `Python`, `Shell`, `Makefile`, etc.) instead of bare extensions.
- Explicit language separation for source and header files (`.c` $\rightarrow$ `C`, `.h` $\rightarrow$ `C Header`; `.cpp`/`.cc` $\rightarrow$ `C++`, `.hpp` $\rightarrow$ `C++ Header`; `.dts` $\rightarrow$ `Device Tree`, `.dtsi` $\rightarrow$ `Device Tree Include`).
- Clean `Unknown` category bundling: unidentifiable extensionless files and unrecognized extensions are cleanly categorized under `Unknown` instead of inflating the table with dozens of one-off rows.
- Deep Linux kernel & systems file support: classification and comment parsing for Device Tree (`.dts`, `.dtsi`), kernel build scripts (`Makefile.*`, `Kbuild.*`, `Android.mk`), kernel configs (`.config`, `defconfig`, `*_defconfig`), Coccinelle (`.cocci`), Bison/Flex (`.y`, `.l`), Linker Scripts (`.lds`, `.ld`), Android Blueprint (`.bp`, `Android.bp`), AWK, Sed, ASN.1, Gettext PO, Graphviz DOT, TeX, and Mediatek DWS.

## [0.2.14] - 2026-09-25

### Added
- `.gitignore` awareness across all scans (both git repositories and non-git directories) with cascading subdirectory loading, while maintaining `.locignore` priority.
- Structured Unicode table reporting: redesigned summary output and per-language breakdown into aligned terminal grid boxes.
- Default per-language breakdown: always displayed in human output mode without requiring explicit `-d` flag.
- Extensionless file classification: automatic language and comment resolution for known filenames (`Makefile`, `Kconfig`, `Dockerfile`, `CMakeLists.txt`, `Meson`, `Jenkinsfile`, etc.) and `#!` shebang scripts (`bash`, `sh`, `python`, `perl`, `ruby`, `node`, `php`).
- Expanded language support for raw LOC counting: Assembly, Perl, OCaml, Erlang, Dart, Fortran, Pascal, Clojure, R, Julia, V, Odin, CUDA, GLSL/Shaders, Protobuf, GraphQL, CMake, Meson, Kconfig, and Makefile.
- Unit and integration tests for `.gitignore` support, `.locignore` precedence, shebang detection, known filenames, and comment counting.

## [0.2.13] - 2026-09-25

### Fixed
- Preserved `<script>` and `<style>` tag lines when stripping embedded bodies in HTML, Vue, and Svelte templates (`strip_block`), preventing tag lines from vanishing from the container breakdown.
- Synchronized top-level summary metrics with embedded chunks in `parse_html_embedded`, ensuring primary code, comment, and blank totals accurately reflect embedded language comments (e.g. `//` inside `<script>`) rather than evaluating entire template files against HTML-only comment rules.

## [0.2.12] - 2026-09-25

### Added
- Direct file counting support: run `loc` directly on individual files (e.g. `loc src/main.rs`).
- Multi-path and glob scanning: pass multiple files, directories, or shell glob patterns (e.g. `loc *.rs *.py`, `loc dir1/ dir2/ file.rs`).
- Automatic base directory resolution with fallback for paths outside current working directory in tree rendering.
- Integration test coverage for single-file arguments, multiple file arguments, and mixed file/directory inputs.

## [0.2.11] - 2026-09-24

### Added
- Embedded-language handling for HTML, Vue, Svelte (`<script>` to JavaScript/TypeScript, `<style>` to CSS/SCSS).
- Jupyter Notebook (`.ipynb`) support with JSON cell parsing and kernel language extraction (Python, R, Julia, Rust, JS/TS).
- String-aware and escape-aware tokenizer in line analysis preventing string literals (`"// ..."`, `"/* ... */"`) from misclassifying code as comments.
- Nested block comment support (`/* /* nested */ */`) for Rust, Swift, and Haskell (`{- {- ... -} -}`).
- `EmbeddedChunk` model tracking multi-language container distributions in scan results and extension breakdowns.
- Unit and integration tests for embedded script/style tags, Jupyter notebooks, nested comments, and string-contained comments.
- Native decoding and automatic detection for all UTF variants (`UTF-8` with/without BOM, `UTF-16LE` & `UTF-16BE` with/without BOM, and `UTF-32LE` & `UTF-32BE` with BOM).
- Multiline double-quoted and backtick string literal persistence across lines in the tokenizer.
- Accurate classification of mixed lines containing code followed by opening block comments (`code, then /* start`).
- Unit test suite verifying UTF-8 BOM, UTF-16LE/BE, UTF-32LE/BE, and all edge-case fixtures reported by benchmark harness.
- Fixed spurious blank line counts in embedded HTML `<script>` and `<style>` blocks by trimming structural tag-boundary newlines.
### Changed
- Added `src/counter/embedded.rs` for dedicated script, style, and notebook extraction.
- Modularized scanner into zero-allocation byte slice tokenizer in `src/counter/lines.rs`.
- Added trigger character fast-filter (`[bool; 256]`) and whole-line single comment fast-path, bypassing the token scanner for ~80% of lines.
- Eliminated redundant `is_file()` `stat` syscalls before file reading on the hot path in `src/counter/process.rs`.
- Single-pass extension resolution with zero-allocation stack buffer `[u8; 16]` for registry lookups.
- Pre-allocated `HashSet` capacity from git index length and added byte-prefix filtering for repository subdirectory scans in `src/counter/git.rs`.
- Cached embedded HTML `<script>` and `<style>` regular expressions globally with `Lazy<Regex>` in `src/counter/embedded.rs`.
- Early-exit check on empty include/exclude sets and eliminated unnecessary backslash string replacement in `src/locignore/mod.rs`.
- Added `panic = "abort"` to release profile for reduced binary overhead and smaller call-site frames.

## [0.2.10] - 2026-09-24

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
