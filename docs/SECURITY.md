# Security Notes

`loc-rs` is a local CLI scanner. This document covers practical security concerns and safe usage patterns.

## Scope

- Scans files and optionally reads git metadata.
- Generates local reports (JSON/JSONL/CSV/HTML).
- Includes a composite GitHub Action for CI integration.

## Threat Model Highlights

1. Untrusted repository content
  - File names and file contents may be attacker-controlled.
2. CI input injection
  - Shell-backed workflows can be abused if untrusted strings are interpolated.
3. Report rendering
  - HTML output should avoid unsafe dynamic interpolation paths.

## Safe CI Usage for GitHub Action

Use static values for:
- `target_dir`
- `warn_size`
- `args`

Do not pass untrusted user content (issue text, PR comments, chat input) into action inputs consumed by shell `run` blocks.

Current hardening in `.github/actions/loc-rs/action.yml`:
- Inputs are passed via `env` instead of direct shell interpolation.
- Command is built as a bash array (`cmd=(...)`) and executed as `"${cmd[@]}"`.
- `warn_size` is validated as numeric before use.
- `args` is tokenized as whitespace-delimited flags.

## Local Usage Guidance

- Prefer scanning trusted directories.
- Review `.locignore` and `.gitignore` behavior before using scan output as compliance evidence.
- Treat HTML reports as local artifacts; do not host them publicly without review.

## Disclosure

If you find a security issue:

1. Do not open a public exploit issue first.
2. Report privately to the maintainer:
  - https://github.com/kelexine
