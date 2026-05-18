# Release Guide

This guide documents the proven release sequence used in this repository.

## Preconditions

- Clean working tree:

```bash
git status --short
```

- Tests pass:

```bash
cargo test
```

- On `main`:

```bash
git branch --show-current
```

## 1) Commit pending changes

```bash
git add -A
git commit -m "fix(scope): summary"
```

## 2) Prepare local release commit/tag

Patch release preparation:

```bash
cargo release patch --no-publish --no-push --execute --no-confirm
```

This creates:
- Version bump commit (for example `chore: Release loc-rs version 0.2.3`)
- Local tag (for example `v0.2.3`)

## 3) Publish the prepared version

```bash
cargo release publish --execute --no-confirm
```

## 4) Push commit and tag

```bash
git push origin main refs/tags/vX.Y.Z
```

Replace `vX.Y.Z` with the prepared tag.

## 5) Verify remote state

```bash
git status --short --branch
git log -3 --oneline --decorate
git ls-remote --heads --tags origin main refs/tags/vX.Y.Z
```

## Failure Pattern: Publish blocked by dependency availability

If publish fails due to an unavailable dependency on crates.io:

1. Create or switch to a branch that preserves the feature work.
2. Apply a release-only compatibility fix on `main`.
3. Re-run tests and `cargo package`.
4. Cut a new patch release and publish that version.

## Action Safety Check (for CI maintainers)

Before merging action changes, verify:

```bash
sed -n '1,220p' .github/actions/loc-rs/action.yml
```

Expected safety properties:
- No direct concatenation of `${{ inputs.* }}` into executable shell command strings.
- Uses an argument array and executes via `"${cmd[@]}"`.
- Validates numeric inputs such as `warn_size`.
