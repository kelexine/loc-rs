# loc - Advanced Lines of Code Counter

> A fast, feature-rich LOC tool written in Rust.  
> **Author:** [kelexine](https://github.com/kelexine)

---

## Features

- **Fast project scanning**: Counts text files across a target directory with optional Rayon-powered parallel processing.
- **Code/comment/blank split**: Classifies source lines using language-aware single-line, block-comment, and stateful string masking rules.
- **Nested comment support**: Handles nested block comments (e.g. `/* /* nested */ */` in Rust and Swift, `{- {- ... -} -}` in Haskell).
- **Embedded-language analysis**: Accurately extracts and credits embedded code blocks inside HTML, Vue, and Svelte templates (`<script>` to JS/TS, `<style>` to CSS/SCSS).
- **Jupyter Notebook (`.ipynb`) support**: Deserializes notebook JSON, identifies the active kernel language, and parses code and markdown cells into respective language statistics.
- **Tree view**: Renders a recursive project tree when `--tree` is enabled, with optional binary-file display.
- **Function extraction**: Uses Tree-sitter-backed extractors for Rust, Python, JavaScript/TypeScript, Go, C/C++, Java/Kotlin/C#/Scala, PHP, Swift, and Ruby.
- **Complexity analysis**: Reports function length and a branch-count cyclomatic complexity estimate.
- **Git-aware discovery**: Uses native `git2` in repositories and can attach last-modified dates via revwalk commit history.
- **Agent mode auto-detection**: Switches to token-efficient TSV output when run inside AI coding agents (Claude Code, Gemini CLI, etc.).
- **Lockfile awareness**: Automatically detects dependency lockfiles (Cargo.lock, package-lock.json, etc.), excluding them from line metrics to prevent skewed stats.
- **Machine-readable outputs**: Supports direct-to-stdout JSON (`--json`), TSV (`--format agent`), and pipe-friendly raw path lists (`-q`).
- **Multi-format export**: Writes JSON, JSONL, CSV, TSV, and HTML reports.
- **Global configuration**: Reads defaults from `~/.config/loc-rs/config.toml` through the platform config directory.
- **Size warnings**: Flags files above a configured line threshold.
- **Resilient text loading**: Single-pass byte loading with BOM detection and lossy UTF-8 fallback (`String::from_utf8_lossy`) for files with legacy encodings (Latin-1/ISO-8859).

---

## Installation


### From [crates.io](https://crates.io/)

```bash
cargo install loc-rs
```

### From source

Requires Rust 1.92.0 or newer.

```bash
git clone https://github.com/kelexine/loc-rs
cd loc-rs
cargo build --release
cargo install --path .
```

The release binary is built at `./target/release/loc`.

---

## Quick Start

```bash
loc
loc src/
loc -d
loc --tree
loc -f --func-analysis
loc -t rust python typescript
loc -e report.html -f
loc --json
loc -q | head -20
loc --format agent -e out.tsv
```

---

## Documentation

- [CONTRIBUTING](CONTRIBUTING.md): Local setup, test commands, and release workflow.
- [Architecture Guide](docs/ARCHITECTURE.md): Module layout and runtime flow.
- [Release Guide](docs/RELEASE.md): Step-by-step `cargo release` sequence.
- [Security Notes](docs/SECURITY.md): Threat model and CI safety guidance.
- [FAQ](docs/FAQ.md): Quick answers for common usage questions.
- [Troubleshooting Guide](docs/TROUBLESHOOTING.md): Common failure modes and fixes.

Generate Rust API docs locally:

```bash
cargo doc --no-deps
```

Open docs in your browser:

```bash
cargo doc --no-deps --open
```

---

## Usage

```text
loc [OPTIONS] [PATHS]...
```

`PATHS` defaults to the current directory (`.`), and supports single files (`loc src/main.rs`), globs (`loc *.rs *.py`), and multiple files or directories.

### Common workflows

| Goal | Command |
|---|---|
| Scan the current directory | `loc` |
| Scan a specific directory | `loc src/` |
| Count a single file | `loc src/main.rs` |
| Count multiple files or globs | `loc *.rs *.py` |
| Show per-extension metrics | `loc -d` |
| Show a recursive tree | `loc --tree` |
| Include binary files in the tree | `loc --tree -b` |
| Extract functions and methods | `loc -f` |
| Show function complexity analysis | `loc --func-analysis` |
| Scan only selected languages | `loc -t rust python typescript` |
| Warn for files above 500 lines | `loc --warn-size 500` |
| Use Git commit dates | `loc --git-dates` |
| Include hidden files and directories | `loc --include-hidden` |
| Disable parallel processing | `loc --no-parallel` |

### Export examples

```bash
loc -e results.json
loc -e results.jsonl
loc -e stats.csv -f
loc -e report.html -f
```

### All Flags

| Flag | Short | Description |
|---|---|---|
| `--detailed` | `-d` | Per-extension breakdown (Code, Comment, Blank) |
| `--tree` | | Show recursive directory tree (hidden by default) |
| `--binary` | `-b` | Show binary files in tree |
| `--functions` | `-f` | Extract functions, methods, classes |
| `--func-analysis` | | Full analysis report (auto-enables `-f`) |
| `--type LANG...` | `-t` | Filter by language(s) |
| `--export FILE` | `-e` | Export results to `.json`, `.jsonl`, `.csv`, or `.html` |
| `--warn-size N` | | Warn for files exceeding N lines |
| `--git-dates` | | Use `git log` for last-modified dates |
| `--include-hidden` | `-H` | Include hidden files and directories |
| `--no-parallel` | | Disable Rayon parallelism |

---

## Configuration

You can persist defaults in `~/.config/loc-rs/config.toml` on Linux, or the equivalent platform config directory returned by the OS.

```toml
warn_size = 500
default_types = ["rust", "python"]
always_extract_functions = true
```

`.locignore` (project root) is also supported for custom file and directory ignores:

```text
# Ignore generated snapshots
snapshots
*.min.js
```

---

## GitHub Action Integration

The repository includes a comprehensive composite GitHub Action for CI line-count and complexity checks. The action features sub-second installs via pre-built binaries, GitHub Step Summary integration, and automated artifact uploads.

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: kelexine/loc-rs/.github/actions/loc-rs@main
    with:
      target_dir: .
      warn_size: 500
      functions: true
      fail_on_warn: true     # Fails the build if any files exceed warn_size
      export_html: true      # Automatically uploads HTML report to artifacts
      export_json: true      # Automatically uploads JSON report to artifacts
      version: latest        # Uses pre-built binaries for instant execution
```

Use static workflow values for `target_dir`, `warn_size`, and `args`; do not pass untrusted pull request, issue, or comment text into shell-backed action inputs.

---

## Supported Languages

| Name | Canonical Group | Extensions / Filenames |
|---|---|---|
| `c` | `C` | `.c` `.c_shipped` `.i` |
| `c-header` / `h` | `C Header` | `.h` `.h_shipped` `.inl` |
| `cpp` | `C++` | `.cpp` `.cc` `.cxx` `.c++` |
| `cpp-header` / `hpp` | `C++ Header` | `.hpp` `.hxx` `.h++` `.hh` `.tpp` `.ipp` |
| `dts` | `Device Tree` | `.dts` `.dtso` |
| `dtsi` | `Device Tree Include` | `.dtsi` |
| `assembly` | `Assembly` | `.s` `.S` `.asm` `.S_shipped` `.s_shipped` |
| `rust` | `Rust` | `.rs` |
| `python` | `Python` | `.py` `.pyw` `.pyi` `.pyx` `.pxd` |
| `shell` | `Shell` | `.sh` `.bash` `.zsh` `.fish` `.ksh` `.csh` `.tcsh` `.dash` |
| `makefile` | `Makefile` | `Makefile` `GNUmakefile` `Kbuild` `Android.mk` `Makefile.*` `Kbuild.*` `.mk` `.mak` `.make` |
| `kconfig` | `Kconfig` | `Kconfig` `Kconfig.*` `Config.in` `Config.src` `.kconfig` |
| `config` | `Config` | `.conf` `.config` `.cfg` `.ini` `.config` `defconfig` `*_defconfig` |
| `cmake` | `CMake` | `CMakeLists.txt` `.cmake` |
| `meson` | `Meson` | `meson.build` `meson_options.txt` `.meson` |
| `bazel` | `Bazel` | `BUILD` `BUILD.bazel` `WORKSPACE` `.bzl` `.bazel` |
| `dockerfile` | `Dockerfile` | `Dockerfile` `Containerfile` `Dockerfile.*` `Containerfile.*` `.dockerfile` |
| `javascript` | `JavaScript` | `.js` `.mjs` `.cjs` |
| `typescript` | `TypeScript` | `.ts` `.mts` |
| `tsx` | `TSX` | `.tsx` |
| `jsx` | `JSX` | `.jsx` |
| `html` | `HTML` | `.html` `.htm` |
| `css` | `CSS` | `.css` |
| `scss` | `SCSS` | `.scss` |
| `sass` | `Sass` | `.sass` |
| `less` | `Less` | `.less` |
| `vue` | `Vue` | `.vue` |
| `svelte` | `Svelte` | `.svelte` |
| `json` | `JSON` | `.json` `.jsonl` `.json5` |
| `yaml` | `YAML` | `.yml` `.yaml` |
| `toml` | `TOML` | `.toml` |
| `xml` | `XML` | `.xml` `.xsl` `.xslt` |
| `svg` | `SVG` | `.svg` |
| `sql` | `SQL` | `.sql` |
| `coccinelle` | `Coccinelle` | `.cocci` |
| `bison` | `Bison` | `.y` `.yacc` `.yy` |
| `flex` | `Flex` | `.l` `.lex` `.ll` |
| `linker-script` | `Linker Script` | `.lds` `.ld` |
| `android-blueprint` | `Android Blueprint` | `Android.bp` `.bp` |
| `restructuredtext` | `reStructuredText` | `.rst` |
| `markdown` | `Markdown` | `.md` `.markdown` `.mdx` |
| `text` | `Plain Text` | `.txt` `.text` |
| `awk` | `AWK` | `.awk` |
| `sed` | `Sed` | `.sed` |
| `asn1` | `ASN.1` | `.asn1` `.asn` |
| `gettext` | `Gettext` | `.po` `.pot` |
| `graphviz` | `Graphviz` | `.dot` `.gv` |
| `tex` | `TeX` | `.tex` `.sty` `.cls` |
| `vim` | `Vim Script` | `.vim` |
| `dws` | `Mediatek DWS` | `.dws` |
| `jupyter` | `Jupyter` | `.ipynb` |
| `go` | `Go` | `.go` |
| `java` | `Java` | `.java` |
| `kotlin` | `Kotlin` | `.kt` `.kts` |
| `swift` | `Swift` | `.swift` |
| `csharp` | `C#` | `.cs` |
| `ruby` | `Ruby` | `.rb` `.rake` `.gemspec` `Rakefile` `Gemfile` |
| `php` | `PHP` | `.php` `.php3` `.php4` `.php5` `.phtml` |
| `perl` | `Perl` | `.pl` `.pm` `.t` `.xs` `.PL` |
| `lua` | `Lua` | `.lua` |
| `zig` | `Zig` | `.zig` |
| `nim` | `Nim` | `.nim` `.nims` |
| `ocaml` | `OCaml` | `.ml` `.mli` |
| `erlang` | `Erlang` | `.erl` `.hrl` |
| `elixir` | `Elixir` | `.ex` `.exs` |
| `scala` | `Scala` | `.scala` `.sc` |
| `haskell` | `Haskell` | `.hs` `.lhs` |
| `clojure` | `Clojure` | `.clj` `.cljs` `.cljc` `.edn` |
| `r` | `R` | `.r` `.R` |
| `julia` | `Julia` | `.jl` |
| `dart` | `Dart` | `.dart` |
| `fortran` | `Fortran` | `.f` `.for` `.f90` `.f95` `.f03` `.f08` |
| `pascal` | `Pascal` | `.pas` `.pp` `.inc` |
| `v` | `V` | `.v` |
| `odin` | `Odin` | `.odin` |
| `cuda` | `CUDA` | `.cu` `.cuh` |
| `glsl` | `GLSL` | `.glsl` `.vert` `.frag` `.geom` `.comp` `.hlsl` `.wgsl` |
| `protobuf` | `Protobuf` | `.proto` |
| `graphql` | `GraphQL` | `.graphql` `.gql` |

Language aliases are supported for common names such as `py`, `js`, `ts`, `tsx`, `rs`, `c`, `h`, `cpp`, `hpp`, `dts`, `dtsi`, `rb`, `sh`, `bash`, `zsh`, `md`, `yml`, `kt`, `hs`, `c++`, `cxx`, `cc`, and `cs`.

---

## Function Extraction Support

Function extraction is available when `-f` or `--func-analysis` is enabled. The extractors return function names, line ranges, parameters where supported, async markers where supported, class/struct markers, docstrings where supported, decorators where supported, and a complexity estimate.

| Language | Functions | Methods | Classes/Structs | Async | Decorators | Docstrings |
|---|---|---|---|---|---|---|
| Rust | ✓ | ✓ | ✓ (struct/impl) | ✓ | pub flag | — |
| Python | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| JavaScript/TypeScript | ✓ | ✓ | ✓ | ✓ | — | — |
| Go | ✓ | ✓ | — | — | — | — |
| C/C++ | ✓ | ✓ | ✓ | — | — | — |
| Java/Kotlin/C#/Scala | ✓ | ✓ | ✓ | — | — | — |
| PHP | ✓ | ✓ | ✓ | — | — | — |
| Swift | ✓ | ✓ | ✓ | ✓ | — | — |
| Ruby | ✓ | ✓ | ✓ | — | — | — |

---

## Export Formats

| Format | Extension | Contents |
|---|---|---|
| JSON | `.json` | Metadata, extension breakdown, files, and optional function data |
| JSON Lines | `.jsonl` | One file record per line |
| CSV | `.csv` | File-level metrics, with optional function columns |
| HTML | `.html`, `.htm` | Standalone visual report with charts and searchable file table |

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Functions: 0` in summary | Function extraction not enabled | Run with `-f` or `--func-analysis` |
| Unknown language warning (for example `dart`) | Language not in resolver map | Use a supported language or direct extension via `-t .ext` |
| Non-UTF-8 or UTF-16/32 encoding | File is encoded in UTF-16, UTF-32, or legacy 8-bit text | Native decoders automatically handle UTF-16/32 and BOMs; non-UTF-8 uses lossy fallback |
| Missing untracked files in output | Running inside a git repo with default git-based discovery | Check `.gitignore`, or run with `--include-hidden` / adjust ignore rules |
| `--git-dates` appears slow | Traverses git commit history via git2 revwalk | Omit `--git-dates` for faster scans |
| HTML report not opening as expected | Output path/extension mismatch | Export with `.html` or `.htm` extension |

---

## License

MIT © [kelexine](https://github.com/kelexine)
