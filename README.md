# loc — Advanced Lines of Code Counter

> A fast, feature-rich LOC tool written in Rust.  
> **Author:** [kelexine](https://github.com/kelexine) · **Version:** 0.1.4

---

## Features

- **Tree view** with per-file line counts, function counts, and last-modified dates
- **Parallel scanning** via [Rayon](https://docs.rs/rayon) — uses all CPU cores
- **Function extraction** for 10 languages including Rust, Python, JS, Go, PHP, Swift, etc.
- **Cyclomatic complexity** estimates per function
- **Git integration** — respects `.gitignore` and `.locignore`, optional `git log` dates
- **35+ languages** supported with aliases
- **Multi-format export** — JSON, JSONL, CSV
- **Size warnings** for oversized files
- **BOM-aware binary detection** for UTF-16/32 text files

---

## Installation

### From source (requires Rust ≥ 1.70)

```bash
git clone https://github.com/kelexine/loc-rs
cd loc-rs
cargo build --release
# Binary at: ./target/release/loc

# Install globally
cargo install --path .
```

---

## Usage

```
loc [OPTIONS] [DIRECTORY]
```

### Examples

```bash
loc                            # Scan current directory
loc src/                       # Scan a specific path
loc -d                         # Breakdown by extension
loc -f                         # Extract functions/methods
loc -f --func-analysis         # Full complexity report
loc -t rust python             # Filter to Rust + Python only
loc -e results.json            # Export to JSON
loc -e stats.csv -f            # CSV with function data
loc --warn-size 500            # Warn on files > 500 lines
loc --git-dates                # Use git log for last-modified
loc --no-parallel              # Disable parallel processing
```

### All Flags

| Flag | Short | Description |
|---|---|---|
| `--detailed` | `-d` | Per-extension breakdown table |
| `--binary` | `-b` | Show binary files in tree |
| `--functions` | `-f` | Extract functions, methods, classes |
| `--func-analysis` | | Full analysis report (auto-enables `-f`) |
| `--type LANG...` | `-t` | Filter by language(s) |
| `--export FILE` | `-e` | Export results (`.json` / `.jsonl` / `.csv`) |
| `--warn-size N` | | Warn for files exceeding N lines |
| `--git-dates` | | Use `git log` for last-modified dates |
| `--no-parallel` | | Disable Rayon parallelism |

---

## Supported Languages

| Name | Extensions |
|---|---|
| `rust` | `.rs` |
| `python` | `.py` `.pyw` `.pyi` |
| `javascript` | `.js` `.mjs` `.cjs` |
| `typescript` | `.ts` `.tsx` `.mts` |
| `go` | `.go` |
| `java` | `.java` |
| `kotlin` | `.kt` `.kts` |
| `c` | `.c` `.h` |
| `cpp` | `.cpp` `.cc` `.cxx` `.hpp` |
| `csharp` | `.cs` |
| `swift` | `.swift` |
| `ruby` | `.rb` |
| `php` | `.php` |
| `html` | `.html` `.htm` |
| `css` | `.css` `.scss` `.sass` `.less` |
| `shell` | `.sh` `.bash` `.zsh` `.fish` |
| `markdown` | `.md` `.markdown` `.mdx` |
| `json` | `.json` `.jsonl` |
| `yaml` | `.yml` `.yaml` |
| `toml` | `.toml` |
| `xml` | `.xml` |
| `vue` | `.vue` |
| `svelte` | `.svelte` |
| `scala` | `.scala` `.sc` |
| `haskell` | `.hs` `.lhs` |
| `elixir` | `.ex` `.exs` |
| `lua` | `.lua` |
| `dart` | `.dart` |
| `zig` | `.zig` |
| `nim` | `.nim` `.nims` |

Language aliases are supported: `py`, `js`, `ts`, `rs`, `rb`, `sh`, `md`, `yml`, `c++`, etc.

---

## Function Extraction Support

| Language | Functions | Methods | Classes/Structs | Async | Decorators | Docstrings |
|---|---|---|---|---|---|---|
| Rust | ✓ | ✓ | ✓ (struct/impl) | ✓ | pub flag | — |
| Python | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| JavaScript/TS | ✓ | ✓ | ✓ | ✓ | — | — |
| Go | ✓ | ✓ | — | — | — | — |
| C/C++ | ✓ | — | — | — | — | — |
| Java/Kotlin/C# | ✓ | ✓ | — | — | — | — |
| PHP | ✓ | ✓ | ✓ | — | — | — |
| Swift | ✓ | ✓ | ✓ | ✓ | — | — |
| Ruby | ✓ | ✓ | ✓ | — | — | — |
| Nim | ✓ | ✓ | — | — | public(*) | — |

---

## License

MIT © [kelexine](https://github.com/kelexine)
