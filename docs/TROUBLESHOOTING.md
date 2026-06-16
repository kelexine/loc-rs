# Troubleshooting

Common issues and how to resolve them.

## `Functions: 0` in summary

Cause:
- Function extraction is disabled.

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
- Use supported names from `README.md`.
- Or pass a direct extension:

```bash
loc -t .rs .py
```

## Scans feel slow with `--git-dates`

Cause:
- `--git-dates` walks git history to determine last-modified timestamps.

Fix:
- Omit `--git-dates` for faster scans.
- Restrict scan surface with `-t` filters or a narrower target directory.

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
```

## HTML export not generated

Cause:
- Output filename does not end with `.html`/`.htm`.

Fix:

```bash
loc -e report.html
```

## `stream did not contain valid UTF-8` warning

Cause:
- The file is encoded in UTF-16 or UTF-32.
- While `loc-rs` correctly identifies these as non-binary files via BOM detection, the current file reader only supports UTF-8 encoded text.

Fix:
- Convert the file to UTF-8 if you need it included in line counts.
- `loc-rs` will skip these files and emit a warning to ensure results for other files remain accurate.

## Command fails with directory error

Cause:
- Target path is not a directory or cannot be resolved.

Fix:
- Check path correctness:

```bash
loc .
loc ./src
```
