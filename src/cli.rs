// Author: kelexine (https://github.com/kelexine)
// cli.rs — CLI argument parsing via clap derive

use clap::Parser;

/// LOC v5 — Advanced Lines of Code counter
///
/// A fast, feature-rich LOC tool with function extraction, git integration,
/// parallel processing, and multi-format export.
///
/// Author: kelexine (https://github.com/kelexine)
#[derive(Parser, Debug)]
#[command(
    name = "loc",
    version = "5.0.0",
    author = "kelexine <https://github.com/kelexine>",
    about = "Advanced LOC counter — functions, git dates, parallel scan, JSON/CSV export",
    after_help = "\
EXAMPLES:
  loc                          Count LOC in current directory
  loc src/                     Scan a specific directory
  loc -d                       Show per-extension breakdown
  loc -f                       Extract and list functions/methods
  loc -f --func-analysis       Full function complexity report
  loc -t rust python           Only scan Rust and Python files
  loc -e results.json          Export to JSON
  loc -e stats.csv -f          Export CSV with function data
  loc --warn-size 500          Warn about files > 500 lines
  loc --git-dates              Use git log for last-modified dates
  loc src/ -d -t rust -f -e out.json

SUPPORTED LANGUAGES:
  python, javascript, typescript, rust, go, java, kotlin, swift,
  c, cpp, csharp, ruby, php, html, css, shell, sql, markdown,
  json, yaml, xml, jsx, vue, svelte, toml, scala, haskell, elixir, lua, dart, zig

FUNCTION EXTRACTION:
  Rust, Python, JavaScript/TypeScript, Go, C/C++, Java/Kotlin/C#

IMPROVEMENTS OVER v4 (Python):
  • Rayon data-parallelism — uses all CPU cores
  • Zero-copy line counting via raw byte scan
  • Pre-compiled regex (once_cell) — no per-file regex compilation
  • Richer Rust extraction: structs, impl blocks, pub/async detection
  • Python decorator and docstring extraction
  • Filesystem mtime fallback when not in a git repo
  • walkdir-based traversal (faster than os.walk)"
)]
pub struct Args {
    /// Target directory to scan (default: current directory)
    #[arg(default_value = ".")]
    pub directory: String,

    /// Show per-extension breakdown table
    #[arg(short = 'd', long = "detailed")]
    pub detailed: bool,

    /// Include binary files in tree view
    #[arg(short = 'b', long = "binary")]
    pub binary: bool,

    /// Extract functions, methods, and classes from source files
    #[arg(short = 'f', long = "functions")]
    pub functions: bool,

    /// Show detailed function analysis report (auto-enables -f)
    #[arg(long = "func-analysis")]
    pub func_analysis: bool,

    /// Filter by language(s) — e.g. -t rust python typescript
    #[arg(short = 't', long = "type", value_name = "LANG", num_args = 1..)]
    pub file_types: Vec<String>,

    /// Export results to file (.json, .jsonl, or .csv)
    #[arg(short = 'e', long = "export", value_name = "FILE")]
    pub export: Option<String>,

    /// Emit a warning for files that exceed this line count
    #[arg(long = "warn-size", value_name = "LINES")]
    pub warn_size: Option<usize>,

    /// Use `git log` for last-modified dates (more accurate, slightly slower)
    #[arg(long = "git-dates")]
    pub git_dates: bool,

    /// Disable parallel file processing
    #[arg(long = "no-parallel")]
    pub no_parallel: bool,
}
