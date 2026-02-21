// Author: kelexine (https://github.com/kelexine)
// language.rs — Language-to-extension mapping and resolution

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Static map from language name → list of file extensions (with leading dot).
pub static LANGUAGE_MAP: Lazy<HashMap<&'static str, Vec<&'static str>>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("python",     vec![".py", ".pyw", ".pyi"]);
    m.insert("javascript", vec![".js", ".mjs", ".cjs"]);
    m.insert("typescript", vec![".ts", ".tsx", ".mts"]);
    m.insert("rust",       vec![".rs"]);
    m.insert("go",         vec![".go"]);
    m.insert("java",       vec![".java"]);
    m.insert("kotlin",     vec![".kt", ".kts"]);
    m.insert("swift",      vec![".swift"]);
    m.insert("c",          vec![".c", ".h"]);
    m.insert("cpp",        vec![".cpp", ".cc", ".cxx", ".hpp", ".hxx", ".h++"]);
    m.insert("csharp",     vec![".cs"]);
    m.insert("ruby",       vec![".rb", ".rake", ".gemspec"]);
    m.insert("php",        vec![".php", ".php3", ".php4", ".php5", ".phtml"]);
    m.insert("html",       vec![".html", ".htm"]);
    m.insert("css",        vec![".css", ".scss", ".sass", ".less"]);
    m.insert("shell",      vec![".sh", ".bash", ".zsh", ".fish"]);
    m.insert("sql",        vec![".sql"]);
    m.insert("markdown",   vec![".md", ".markdown", ".mdx"]);
    m.insert("json",       vec![".json", ".jsonl", ".json5"]);
    m.insert("yaml",       vec![".yml", ".yaml"]);
    m.insert("xml",        vec![".xml", ".xsl", ".xslt"]);
    m.insert("jsx",        vec![".jsx"]);
    m.insert("vue",        vec![".vue"]);
    m.insert("svelte",     vec![".svelte"]);
    m.insert("toml",       vec![".toml"]);
    m.insert("scala",      vec![".scala", ".sc"]);
    m.insert("haskell",    vec![".hs", ".lhs"]);
    m.insert("elixir",     vec![".ex", ".exs"]);
    m.insert("lua",        vec![".lua"]);
    m.insert("dart",       vec![".dart"]);
    m.insert("zig",        vec![".zig"]);
    m
});

/// Aliases: short names / common misspellings → canonical language names.
static ALIASES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("py",       "python");
    m.insert("js",       "javascript");
    m.insert("ts",       "typescript");
    m.insert("tsx",      "typescript");
    m.insert("rs",       "rust");
    m.insert("c++",      "cpp");
    m.insert("cxx",      "cpp");
    m.insert("cc",       "cpp");
    m.insert("cs",       "csharp");
    m.insert("rb",       "ruby");
    m.insert("sh",       "shell");
    m.insert("bash",     "shell");
    m.insert("zsh",      "shell");
    m.insert("md",       "markdown");
    m.insert("yml",      "yaml");
    m.insert("kt",       "kotlin");
    m.insert("hs",       "haskell");
    m
});

/// Resolve a user-provided language name or extension to a list of file extensions.
///
/// Accepts:
/// - Full language name: `"python"` → `[".py", ".pyw", ".pyi"]`
/// - Alias: `"py"` → same
/// - Raw extension: `".rs"` or `"rs"` → `[".rs"]`
pub fn resolve_extensions(input: &str) -> Vec<String> {
    let lower = input.to_lowercase();

    // Direct dot-extension supplied: ".rs" or "rs"
    if lower.starts_with('.') {
        return vec![lower];
    }

    // Check alias map first
    let canonical = ALIASES.get(lower.as_str()).copied().unwrap_or(lower.as_str());

    // Look up in language map
    if let Some(exts) = LANGUAGE_MAP.get(canonical) {
        return exts.iter().map(|e| e.to_string()).collect();
    }

    // Treat the input as a bare extension
    vec![format!(".{}", lower)]
}

/// Binary extensions — files with these extensions are skipped for line counting.
pub static BINARY_EXTENSIONS: Lazy<std::collections::HashSet<&'static str>> = Lazy::new(|| {
    [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico",
        ".pdf", ".zip", ".tar", ".gz", ".bz2", ".xz", ".rar", ".7z",
        ".exe", ".dll", ".so", ".dylib", ".bin", ".wasm",
        ".mp3", ".mp4", ".avi", ".mov", ".wav", ".flac", ".ogg",
        ".ttf", ".otf", ".woff", ".woff2", ".eot",
        ".pyc", ".pyo", ".class", ".o", ".a", ".lib",
        ".db", ".sqlite", ".sqlite3",
        ".lock",  // Cargo.lock, package-lock.json — useful but not always wanted
    ]
    .iter()
    .copied()
    .collect()
});

/// Directories excluded by default in non-git mode.
pub static EXCLUDED_DIRS: Lazy<std::collections::HashSet<&'static str>> = Lazy::new(|| {
    [
        "node_modules", ".git", "vendor", ".venv", "venv", "__pycache__",
        "dist", "build", ".next", ".nuxt", "target", "bin", "obj",
        ".gradle", ".idea", ".vscode", "coverage", ".pytest_cache",
        ".mypy_cache", ".tox", "eggs", ".eggs", ".cargo",
    ]
    .iter()
    .copied()
    .collect()
});

/// List all known language names (for help text).
#[allow(dead_code)]
pub fn all_languages() -> Vec<&'static str> {
    let mut langs: Vec<_> = LANGUAGE_MAP.keys().copied().collect();
    langs.sort_unstable();
    langs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_extensions_basic() {
        assert_eq!(resolve_extensions("rust"), vec![".rs".to_string()]);
        assert_eq!(resolve_extensions("rs"), vec![".rs".to_string()]);
        assert_eq!(resolve_extensions(".rs"), vec![".rs".to_string()]);
    }

    #[test]
    fn test_resolve_extensions_aliases() {
        assert_eq!(resolve_extensions("py"), vec![".py", ".pyw", ".pyi"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(resolve_extensions("javascript"), vec![".js", ".mjs", ".cjs"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(resolve_extensions("js"), vec![".js", ".mjs", ".cjs"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn test_resolve_extensions_case_insensitive() {
        assert_eq!(resolve_extensions("RUST"), vec![".rs".to_string()]);
        assert_eq!(resolve_extensions("Py"), vec![".py", ".pyw", ".pyi"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn test_resolve_extensions_unknown() {
        // Unknown language should be treated as a bare extension
        assert_eq!(resolve_extensions("xyzzy"), vec![".xyzzy".to_string()]);
    }

    #[test]
    fn test_all_languages() {
        let langs = all_languages();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
        assert!(langs.is_sorted());
    }
}
