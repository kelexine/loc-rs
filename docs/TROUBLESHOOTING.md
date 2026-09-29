# Troubleshooting

Common issues and how to resolve them.

## `Functions: 0` in summary

Cause:
- Function extraction is disabled or lamguage might not be supported.

Fix:

```bash
loc -f
# or
loc --func-analysis
```

You can also enable it by default in config:

```toml
always_extract_functions = true
```

## Unknown language filter warning

Example:
- `[WARNING] Unknown language filter: dart`

Cause:
- The language name is not mapped in resolver aliases/language map.

Fix:
- Use supported names from the `README.md`.
- Or pass a direct extension:

```bash
loc -t .rs .py
```

## Hidden files are missing

Cause:
- Hidden files/directories are skipped by default.

Fix:

```bash
loc --include-hidden
```

## Output includes files you want ignored

Cause:
- File is not excluded by `.gitignore`, built-in excludes, or `.locignore`.

Fix:
- Add a `.locignore` file in project root:

```text
node_modules
dist
generated
ignoreme
thisisignored
```

## HTML export not generated

Cause:
- Output filename does not end with `.html`/`.htm`.

Fix:

```bash
loc -e report.html
```

## Non-UTF-8, UTF-16, UTF-32, or legacy encoded files

Behavior:
- In `loc-rs` v0.2.11+, files encoded in `UTF-16LE`, `UTF-16BE`, `UTF-32LE`, and `UTF-32BE` (with or without BOM) and `UTF-8` with BOM are automatically detected and decoded natively.
- Non-UTF-8 8-bit text files (such as Latin-1/ISO-8859 comments or binary look-up tables in source files) are automatically parsed via a lossy UTF-8 fallback (`String::from_utf8_lossy`).
- This eliminates `stream did not contain valid UTF-8` warning skips and ensures full line metrics are recorded without crashing or skipping valid source code.
- Files identified as binary via null-byte inspection or known binary extensions are skipped automatically.

## Command fails with directory error

Cause:
- Target path does not exist or cannot be resolved.

Fix:
- Check path correctness:

```shell
loc .
loc ./src
loc <path to file/dir>
```
