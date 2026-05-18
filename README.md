# loc - Advanced Lines of Code Counter

> A fast, feature-rich LOC tool written in Rust.  
> **Author:** [kelexine](https://github.com/kelexine)

---

## Features

- **Fast project scanning**: Counts text files across a target directory with optional Rayon-powered parallel processing.
- **Code/comment/blank split**: Classifies source lines using language-aware single-line and block-comment rules.
- **Tree view**: Renders a recursive project tree when `--tree` is enabled, with optional binary-file display.
- **Function extraction**: Uses Tree-sitter-backed extractors for Rust, Python, JavaScript/TypeScript, Go, C/C++, Java/Kotlin/C#/Scala, PHP, Swift, and Ruby.
- **Complexity analysis**: Reports function length and a branch-count cyclomatic complexity estimate.
- **Git-aware discovery**: Uses `git ls-files` in repositories and can attach last-modified dates from `git log`.
- **Multi-format export**: Writes JSON, JSONL, CSV, and HTML reports.
- **Global configuration**: Reads defaults from `~/.config/loc-rs/config.toml` through the platform config directory.
- **Size warnings**: Flags files above a configured line threshold.
- **BOM-aware binary detection**: Treats UTF-16/UTF-32 files as text instead of false-positive binary files.

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
loc [OPTIONS] [DIRECTORY]
```

`DIRECTORY` defaults to the current directory.

### Common workflows

| Goal | Command |
|---|---|
| Scan the current directory | `loc` |
| Scan a specific directory | `loc src/` |
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

The repository includes a composite GitHub Action for CI line-count and complexity checks.

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: kelexine/loc-rs/.github/actions/loc-rs@main
    with:
      target_dir: .
      warn_size: 500
      functions: true
```

Use static workflow values for `target_dir`, `warn_size`, and `args`; do not pass untrusted pull request, issue, or comment text into shell-backed action inputs.

---

## Supported Languages

| Name | Extensions |
|---|---|
| `c` | `.c` `.h` |
| `csharp` | `.cs` |
| `cpp` | `.cpp` `.cc` `.cxx` `.hpp` `.hxx` `.h++` |
| `css` | `.css` `.scss` `.sass` `.less` |
| `elixir` | `.ex` `.exs` |
| `go` | `.go` |
| `haskell` | `.hs` `.lhs` |
| `html` | `.html` `.htm` |
| `java` | `.java` |
| `javascript` | `.js` `.mjs` `.cjs` |
| `json` | `.json` `.jsonl` `.json5` |
| `jsx` | `.jsx` |
| `kotlin` | `.kt` `.kts` |
| `lua` | `.lua` |
| `markdown` | `.md` `.markdown` `.mdx` |
| `nim` | `.nim` `.nims` |
| `php` | `.php` `.php3` `.php4` `.php5` `.phtml` |
| `python` | `.py` `.pyw` `.pyi` |
| `ruby` | `.rb` `.rake` `.gemspec` |
| `rust` | `.rs` |
| `scala` | `.scala` `.sc` |
| `shell` | `.sh` `.bash` `.zsh` `.fish` |
| `sql` | `.sql` |
| `svelte` | `.svelte` |
| `swift` | `.swift` |
| `toml` | `.toml` |
| `typescript` | `.ts` `.tsx` `.mts` |
| `vue` | `.vue` |
| `xml` | `.xml` `.xsl` `.xslt` |
| `yaml` | `.yml` `.yaml` |
| `zig` | `.zig` |

Language aliases are supported for common names such as `py`, `js`, `ts`, `tsx`, `rs`, `rb`, `sh`, `bash`, `zsh`, `md`, `yml`, `kt`, `hs`, `c++`, `cxx`, `cc`, and `cs`.

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
| Missing untracked files in output | Running inside a git repo with default git-based discovery | Check `.gitignore`, or run with `--include-hidden` / adjust ignore rules |
| `--git-dates` appears slow | Uses `git log` history traversal | Omit `--git-dates` for faster scans |
| HTML report not opening as expected | Output path/extension mismatch | Export with `.html` or `.htm` extension |

---

## License

MIT © [kelexine](https://github.com/kelexine)
