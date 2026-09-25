// Author: kelexine (https://github.com/kelexine)
// locignore.rs — .locignore engine: glob patterns, per-directory cascade,
//                negation overrides, and precedence over .gitignore.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};

/// A compiled `.locignore` ruleset.
///
/// # Features
/// - **Glob patterns** — `*.lock`, `dist/`, `src/**/*.generated.rs`
/// - **Per-directory cascade** — a `.locignore` in any subdirectory adds rules
///   scoped to that subtree, mirroring `.gitignore` semantics
/// - **Negation** — `!src/keep.rs` re-includes a path even if a broader rule
///   would otherwise exclude it
/// - **Precedence** — `.locignore` rules always win over `.gitignore` rules:
///   a negation overrides a `.gitignore` exclude; an exclude overrides a
///   `.gitignore` include
///
/// # Pattern semantics (per file containing the pattern)
/// | Pattern form          | Meaning                                              |
/// |-----------------------|------------------------------------------------------|
/// | `*.lock`              | Match `*.lock` anywhere under the containing dir     |
/// | `/Cargo.lock`         | Match `Cargo.lock` only in the containing dir        |
/// | `dist/`               | Match the `dist/` directory and all its contents     |
/// | `src/**/*.min.js`     | Match as a root-relative glob from containing dir    |
/// | `!keep.rs`            | Re-include `keep.rs` anywhere (override any exclude) |
#[derive(Clone)]
pub struct LocIgnore {
    /// Compiled .locignore patterns that exclude files.
    loc_exclude: GlobSet,
    /// Compiled .locignore patterns that force-include files.
    loc_include: GlobSet,
    /// Compiled .gitignore patterns that exclude files.
    git_exclude: GlobSet,
    /// Compiled .gitignore patterns that force-include files.
    git_include: GlobSet,
    /// Absolute path to the scan root.
    root: PathBuf,
    /// Set to `true` when any `!` negation pattern was loaded.
    has_negations: bool,
}

impl LocIgnore {
    /// Build a `LocIgnore` by recursively scanning `root` for `.locignore`
    /// and `.gitignore` files, compiling their patterns into prioritized sets.
    pub fn build(root: &Path) -> Self {
        let mut loc_excl = GlobSetBuilder::new();
        let mut loc_incl = GlobSetBuilder::new();
        let mut git_excl = GlobSetBuilder::new();
        let mut git_incl = GlobSetBuilder::new();
        let mut has_negations = false;

        Self::collect(
            root,
            root,
            &mut loc_excl,
            &mut loc_incl,
            &mut git_excl,
            &mut git_incl,
            &mut has_negations,
        );

        Self {
            loc_exclude: loc_excl.build().unwrap_or_else(|_| GlobSet::empty()),
            loc_include: loc_incl.build().unwrap_or_else(|_| GlobSet::empty()),
            git_exclude: git_excl.build().unwrap_or_else(|_| GlobSet::empty()),
            git_include: git_incl.build().unwrap_or_else(|_| GlobSet::empty()),
            root: root.to_path_buf(),
            has_negations,
        }
    }

    /// Returns `true` when any `!` negation pattern is loaded.
    pub fn has_negations(&self) -> bool {
        self.has_negations
    }

    /// Returns `true` if `path` should be **excluded** from the scan.
    ///
    /// Priority hierarchy:
    /// 1. `.locignore` inclusion (`!pattern`) -> **include** (highest priority, overrides all)
    /// 2. `.locignore` exclusion -> **exclude** (overrides .gitignore)
    /// 3. `.gitignore` inclusion (`!pattern`) -> **include**
    /// 4. `.gitignore` exclusion -> **exclude**
    /// 5. Default -> **include**
    pub fn is_excluded(&self, path: &Path) -> bool {
        if self.loc_include.is_empty()
            && self.loc_exclude.is_empty()
            && self.git_include.is_empty()
            && self.git_exclude.is_empty()
        {
            return false;
        }

        let rel = self.rel(path);
        let lossy = rel.to_string_lossy();
        let s = if lossy.contains('\\') {
            std::borrow::Cow::Owned(lossy.replace('\\', "/"))
        } else {
            lossy
        };

        // 1. .locignore negation takes highest precedence
        if !self.loc_include.is_empty() && self.loc_include.is_match(s.as_ref()) {
            return false;
        }

        // 2. .locignore exclude overrides .gitignore
        if !self.loc_exclude.is_empty() && self.loc_exclude.is_match(s.as_ref()) {
            return true;
        }

        // 3. .gitignore negation
        if !self.git_include.is_empty() && self.git_include.is_match(s.as_ref()) {
            return false;
        }

        // 4. .gitignore exclude
        !self.git_exclude.is_empty() && self.git_exclude.is_match(s.as_ref())
    }

    /// Compute path relative to the scan root (for matching).
    fn rel<'p>(&self, path: &'p Path) -> std::borrow::Cow<'p, Path> {
        match path.strip_prefix(&self.root) {
            Ok(r) => std::borrow::Cow::Borrowed(r),
            // Path outside the root — use as-is (edge case: manual file args).
            Err(_) => std::borrow::Cow::Borrowed(path),
        }
    }

    /// Parse pattern lines from ignore file content into exclude / include builders.
    fn parse_patterns(
        content: &str,
        dir_rel: &str,
        excl: &mut GlobSetBuilder,
        incl: &mut GlobSetBuilder,
        has_negations: &mut bool,
    ) {
        for raw in content.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (builder, pattern): (&mut GlobSetBuilder, &str) =
                if let Some(rest) = line.strip_prefix('!') {
                    *has_negations = true;
                    (incl, rest)
                } else {
                    (excl, line)
                };

            Self::add_pattern(builder, pattern, dir_rel);
        }
    }

    /// Recursively descend into `dir`, loading any `.gitignore` and `.locignore` found there
    /// and adding compiled globs to the shared builders.
    fn collect(
        dir: &Path,
        root: &Path,
        loc_excl: &mut GlobSetBuilder,
        loc_incl: &mut GlobSetBuilder,
        git_excl: &mut GlobSetBuilder,
        git_incl: &mut GlobSetBuilder,
        has_negations: &mut bool,
    ) {
        let dir_rel = dir
            .strip_prefix(root)
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .replace('\\', "/");

        // 1. Load .gitignore for this directory, if present.
        let gitignore_path = dir.join(".gitignore");
        if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
            Self::parse_patterns(&content, &dir_rel, git_excl, git_incl, has_negations);
        }

        // 2. Load .locignore for this directory, if present.
        let locignore_path = dir.join(".locignore");
        if let Ok(content) = std::fs::read_to_string(&locignore_path) {
            Self::parse_patterns(&content, &dir_rel, loc_excl, loc_incl, has_negations);
        }

        // 3. Recurse into subdirectories — skip heavy / hidden dirs for speed.
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(n) = file_name.to_str() else {
                continue;
            };
            // Skip .git, target, node_modules, vendor — they're either excluded
            // by default or too large to recurse into cheaply.
            if n.starts_with('.') || matches!(n, "target" | "node_modules" | "vendor") {
                continue;
            }
            Self::collect(
                &entry.path(),
                root,
                loc_excl,
                loc_incl,
                git_excl,
                git_incl,
                has_negations,
            );
        }
    }

    /// Expand `pattern` (from a `.locignore` at relative path `dir_rel`) into
    /// one or more concrete glob strings and register them on `builder`.
    fn add_pattern(builder: &mut GlobSetBuilder, pattern: &str, dir_rel: &str) {
        for expanded in Self::expand(pattern, dir_rel) {
            if let Ok(g) = Glob::new(&expanded) {
                builder.add(g);
            }
            // Silently skip malformed patterns — don't crash on user typos.
        }
    }

    /// Expand `pattern` relative to `dir_rel` into glob strings.
    ///
    /// ```text
    /// dir_rel=""      pattern="*.lock"       → ["*.lock",    "**/*.lock"]
    /// dir_rel=""      pattern="dist/"        → ["dist",      "dist/**"]
    /// dir_rel=""      pattern="/Cargo.lock"  → ["Cargo.lock"]
    /// dir_rel="src"   pattern="*.tmp"        → ["src/*.tmp", "src/**/*.tmp"]
    /// dir_rel="src"   pattern="/gen/"        → ["src/gen",   "src/gen/**"]
    /// dir_rel=""      pattern="a/**/b.rs"    → ["a/**/b.rs"]
    /// ```
    fn expand(pattern: &str, dir_rel: &str) -> Vec<String> {
        let prefix = |s: &str| -> String {
            if dir_rel.is_empty() {
                s.to_string()
            } else {
                format!("{}/{}", dir_rel, s)
            }
        };

        // Trailing `/` → directory exclude: match the dir name and everything inside.
        if let Some(dir_pat) = pattern.strip_suffix('/') {
            return vec![prefix(dir_pat), prefix(&format!("{}/**", dir_pat))];
        }

        // Leading `/` → anchored to the containing directory.
        if let Some(anchored) = pattern.strip_prefix('/') {
            return vec![prefix(anchored)];
        }

        // Pattern already contains a non-glob path separator → treat as relative.
        // e.g. "src/gen/*.rs"  or  "a/**/b.rs"
        if pattern.contains('/') {
            return vec![prefix(pattern)];
        }

        // Filename-only pattern (no `/`) → match directly in the dir AND in
        // every subdirectory below it, mirroring `.gitignore` behaviour.
        vec![prefix(pattern), prefix(&format!("**/{}", pattern))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    // ── expand() unit tests ──────────────────────────────────────────────────

    #[test]
    fn expand_filename_pattern_no_dir() {
        let e = LocIgnore::expand("*.lock", "");
        assert!(e.contains(&"*.lock".to_string()));
        assert!(e.contains(&"**/*.lock".to_string()));
    }

    #[test]
    fn expand_filename_pattern_with_dir() {
        let e = LocIgnore::expand("*.tmp", "src");
        assert!(e.contains(&"src/*.tmp".to_string()));
        assert!(e.contains(&"src/**/*.tmp".to_string()));
    }

    #[test]
    fn expand_trailing_slash_directory() {
        let e = LocIgnore::expand("dist/", "");
        assert!(e.contains(&"dist".to_string()));
        assert!(e.contains(&"dist/**".to_string()));
    }

    #[test]
    fn expand_leading_slash_anchored() {
        let e = LocIgnore::expand("/Cargo.lock", "");
        assert_eq!(e, vec!["Cargo.lock".to_string()]);
    }

    #[test]
    fn expand_slash_containing_pattern() {
        let e = LocIgnore::expand("a/**/b.rs", "");
        assert_eq!(e, vec!["a/**/b.rs".to_string()]);
    }

    // ── is_excluded() integration tests ─────────────────────────────────────

    #[test]
    fn glob_star_lock_excludes_lockfiles() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "*.lock\n");
        write(dir.path(), "Cargo.lock", "version=3");
        write(dir.path(), "main.rs", "fn main(){}");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("Cargo.lock")));
        assert!(!li.is_excluded(&dir.path().join("main.rs")));
    }

    #[test]
    fn glob_double_star_matches_nested() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "**/*.min.js\n");
        write(dir.path(), "dist/app.min.js", "");
        write(dir.path(), "src/app.js", "");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("dist/app.min.js")));
        assert!(!li.is_excluded(&dir.path().join("src/app.js")));
    }

    #[test]
    fn directory_pattern_excludes_contents() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "dist/\n");
        write(dir.path(), "dist/bundle.js", "");
        write(dir.path(), "src/main.rs", "");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("dist/bundle.js")));
        assert!(li.is_excluded(&dir.path().join("dist")));
        assert!(!li.is_excluded(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn negation_re_includes_after_broad_exclude() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "*.lock\n!Gemfile.lock\n");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("Cargo.lock")));
        assert!(li.is_excluded(&dir.path().join("yarn.lock")));
        // Negation re-includes Gemfile.lock despite the *.lock rule.
        assert!(!li.is_excluded(&dir.path().join("Gemfile.lock")));
    }

    #[test]
    fn has_negations_flag() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "*.lock\n");
        assert!(!LocIgnore::build(dir.path()).has_negations());

        write(dir.path(), ".locignore", "*.lock\n!keep.lock\n");
        assert!(LocIgnore::build(dir.path()).has_negations());
    }

    #[test]
    fn per_directory_cascade() {
        let dir = TempDir::new().unwrap();
        // Root .locignore excludes nothing special.
        write(dir.path(), ".locignore", "# root\n");
        // src/.locignore excludes *.tmp under src/.
        write(dir.path(), "src/.locignore", "*.tmp\n");
        write(dir.path(), "src/work.tmp", "");
        write(dir.path(), "src/main.rs", "");
        write(dir.path(), "root.tmp", ""); // NOT under src/ → not excluded

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("src/work.tmp")));
        assert!(!li.is_excluded(&dir.path().join("src/main.rs")));
        // root.tmp: src/.locignore only covers src/ subtree.
        // It should NOT be excluded (it's at root, not under src/).
        assert!(!li.is_excluded(&dir.path().join("root.tmp")));
    }

    #[test]
    fn comment_and_blank_lines_ignored() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "# this is a comment\n\n*.lock\n");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("Cargo.lock")));
    }

    #[test]
    fn anchored_pattern_only_matches_at_dir() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "/Cargo.lock\n");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("Cargo.lock")));
        // nested Cargo.lock should NOT be excluded by an anchored pattern
        assert!(!li.is_excluded(&dir.path().join("sub/Cargo.lock")));
    }

    #[test]
    fn path_outside_root_not_panicking() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".locignore", "*.rs\n");
        let li = LocIgnore::build(dir.path());
        // Path outside root — should not panic, returns a sane result.
        let outside = Path::new("/tmp/other/main.rs");
        let _ = li.is_excluded(outside);
    }

    #[test]
    fn gitignore_basic_exclusion() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".gitignore", "*.log\nbuild/\n");
        write(dir.path(), "app.log", "error");
        write(dir.path(), "build/output.o", "");
        write(dir.path(), "src/main.rs", "fn main(){}");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("app.log")));
        assert!(li.is_excluded(&dir.path().join("build/output.o")));
        assert!(!li.is_excluded(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn gitignore_negation_re_includes() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".gitignore", "*.log\n!important.log\n");
        write(dir.path(), "app.log", "error");
        write(dir.path(), "important.log", "must keep");

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("app.log")));
        assert!(!li.is_excluded(&dir.path().join("important.log")));
    }

    #[test]
    fn locignore_overrides_gitignore() {
        let dir = TempDir::new().unwrap();
        // .gitignore excludes all *.tmp and *.log
        write(dir.path(), ".gitignore", "*.tmp\n*.log\n");
        // .locignore re-includes special.tmp, and additionally excludes secret.rs
        write(dir.path(), ".locignore", "!special.tmp\nsecret.rs\n");

        write(dir.path(), "normal.tmp", "");
        write(dir.path(), "special.tmp", "");
        write(dir.path(), "app.log", "");
        write(dir.path(), "secret.rs", "let secret = 1;");
        write(dir.path(), "main.rs", "fn main(){}");

        let li = LocIgnore::build(dir.path());
        // normal.tmp excluded by .gitignore
        assert!(li.is_excluded(&dir.path().join("normal.tmp")));
        // special.tmp re-included by .locignore negation overriding .gitignore
        assert!(!li.is_excluded(&dir.path().join("special.tmp")));
        // app.log excluded by .gitignore
        assert!(li.is_excluded(&dir.path().join("app.log")));
        // secret.rs excluded by .locignore
        assert!(li.is_excluded(&dir.path().join("secret.rs")));
        // main.rs kept
        assert!(!li.is_excluded(&dir.path().join("main.rs")));
    }

    #[test]
    fn gitignore_subdirectory_cascade() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), ".gitignore", "*.root_ignored\n");
        write(dir.path(), "nested/.gitignore", "*.nested_ignored\n");

        write(dir.path(), "test.root_ignored", "");
        write(dir.path(), "nested/test.nested_ignored", "");
        write(dir.path(), "test.nested_ignored", ""); // at root, NOT in nested/

        let li = LocIgnore::build(dir.path());
        assert!(li.is_excluded(&dir.path().join("test.root_ignored")));
        assert!(li.is_excluded(&dir.path().join("nested/test.nested_ignored")));
        assert!(!li.is_excluded(&dir.path().join("test.nested_ignored")));
    }
}
