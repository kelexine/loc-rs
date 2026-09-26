# FAQ

## Why does summary show `Functions: 0`?

Function extraction is opt-in. Use:

```bash
loc -f
```

or

```bash
loc --func-analysis
```

## Why do I see unknown language warnings?

The language name is not mapped in resolver aliases. Use a supported language or a direct extension:

```bash
loc -t .rs .py
```

## How do I ignore project-specific paths?

Create `.locignore` in project root:

```text
dist
generated
```

## What happened to `--git-dates`?

`--git-dates` has been deprecated and disabled. Traversing commit history via `git2` revwalk caused extreme latency and hangs on repositories with large histories (such as Linux kernel or monorepos). `loc-rs` now operates as a pure Rust binary with zero C library dependencies.

## How does `loc-rs` handle multi-language files like HTML and Jupyter notebooks?

- **HTML, Vue, Svelte**: `loc-rs` inspects `<script>` and `<style>` blocks, crediting lines to JavaScript/TypeScript and CSS/SCSS respectively, while non-script/style markup is attributed to the host template extension (`.html`, `.vue`, `.svelte`).
- **Jupyter Notebooks (`.ipynb`)**: Notebook cells are deserialized from JSON, the kernel language is determined from notebook metadata (defaulting to Python), and code/markdown cells are accurately attributed to their respective language metrics.

## Does `loc-rs` scan binary files?

Binary files are detected and excluded from line metrics. They can be shown in tree output with `--binary`.

## Why are my lockfiles showing 0 lines?

`loc-rs` automatically detects over 30 common dependency lockfiles (like `Cargo.lock`, `package-lock.json`, `go.sum`). These files are tagged as `[lockfile]` in the tree view but are explicitly excluded from all line-count statistics to prevent them from skewing your codebase metrics.

## Which formats can I export?

- `.json`
- `.jsonl`
- `.csv`
- `.tsv`
- `.html` / `.htm`

## How do I generate API docs?

```bash
cargo doc --no-deps
```

Open docs:

```bash
cargo doc --no-deps --open
```
