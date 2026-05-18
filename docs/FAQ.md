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

## Why is `--git-dates` slower?

It traverses commit history via `git log` for file timestamps. Omit `--git-dates` when speed matters.

## Does `loc-rs` scan binary files?

Binary files are detected and excluded from line metrics. They can be shown in tree output with `--binary`.

## Which formats can I export?

- `.json`
- `.jsonl`
- `.csv`
- `.html` / `.htm`

## How do I generate API docs?

```bash
cargo doc --no-deps
```

Open docs:

```bash
cargo doc --no-deps --open
```
