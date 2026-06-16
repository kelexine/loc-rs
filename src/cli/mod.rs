// Author: kelexine (https://github.com/kelexine)
// cli/mod.rs — CLI argument parsing via clap derive

use clap::{Parser, ValueEnum};

/// Output format selector.
///
/// Passed via `--format <MODE>`.  Auto-detection from agent env-vars takes
/// effect when this flag is absent and `--json` / `-q` are not set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Rich colored terminal output (default for interactive sessions).
    Human,
    /// TSV output — no ANSI, nothing truncated, token-efficient.
    /// `--tree` renders a flat file-list TSV instead of ASCII art.
    Agent,
    /// JSON summary to stdout (same as `--json`).
    Json,
    /// One matched path per line (same as `-q`).
    Quiet,
}

/// LOC — Advanced Lines of Code counter
///
/// A fast, feature-rich LOC tool with function extraction, git integration,
/// parallel processing, and multi-format export.  When run inside a coding
/// agent (`CLAUDECODE`, `AI_AGENT`, `CODEX_SANDBOX`, … are set), output
/// switches to agent mode automatically — no flag needed.
///
/// Author: kelexine (<https://github.com/kelexine>)
#[derive(Parser, Debug)]
#[command(
    name = "loc",
    version = env!("CARGO_PKG_VERSION"),
    author = "kelexine <https://github.com/kelexine>",
    about = "Advanced LOC counter — functions, git dates, parallel scan, multi-format output",
    after_help = "\
EXAMPLES:
  loc                          Count LOC in current directory
  loc src/                     Scan a specific directory
  loc -d                       Show per-language breakdown table
  loc -f                       Extract and list functions/methods
  loc -f --func-analysis       Full function complexity report
  loc -t rust python           Only scan Rust and Python files
  loc -e results.json          Export to JSON
  loc -e stats.csv -f          Export CSV with function data
  loc -e report.html           Export interactive HTML dashboard
  loc --warn-size 500          Warn about files > 500 lines
  loc --git-dates              Use git log for last-modified dates
  loc --tree                   Show directory tree (flat TSV in agent mode)
  loc --format agent           Force machine-readable TSV output
  loc --format agent -d -f     Full TSV output with breakdown + functions
  loc -q                       One path per line (pipe-friendly)
  loc -q | head -20            First 20 matched paths
  loc src/ -d -t rust -f -e out.json

OUTPUT MODES:
  human   Colored terminal output, truncated tables, hints at end [default]
  agent   TSV, no ANSI, nothing truncated — auto-detected from env vars
  json    JSON summary to stdout (same as --json)
  quiet   One matched path per line

AGENT AUTO-DETECTION:
  loc detects coding agents from environment variables and switches to agent
  mode automatically: CLAUDECODE, CLAUDE_CODE (Claude Code), CODEX_SANDBOX
  (Codex), CURSOR_TRACE_ID (Cursor), AI_AGENT (any agent), and more.
  Override with --format human to force terminal output.

SUPPORTED LANGUAGES:
  python, javascript, typescript, rust, go, java, kotlin, swift,
  c, cpp, csharp, ruby, php, html, css, shell, sql, markdown,
  json, yaml, xml, jsx, vue, svelte, toml, scala, haskell, elixir, lua, zig, nim

FUNCTION EXTRACTION:
  Rust, Python, JavaScript/TypeScript, Go, C/C++, Java/Kotlin/C#, PHP, Swift, Ruby"
)]
pub struct Args {
    /// Target directory to scan (default: current directory)
    #[arg(default_value = ".")]
    pub directory: String,

    /// Show per-language breakdown table
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

    /// Export results to file (.json, .jsonl, .csv, .tsv, or .html)
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

    /// Include hidden files and directories (skipped by default)
    #[arg(short = 'H', long = "include-hidden")]
    pub include_hidden: bool,

    /// Show recursive directory tree (flat TSV file list in agent mode)
    #[arg(long = "tree")]
    pub tree: bool,

    /// Print scan summary as JSON to stdout — equivalent to --format json
    #[arg(long = "json")]
    pub json: bool,

    /// Output format: human (default), agent (TSV), json, or quiet
    ///
    /// Auto-detected from env vars when omitted — coding agents get TSV
    /// output automatically without passing this flag.
    #[arg(long = "format", value_name = "MODE")]
    pub format: Option<OutputFormat>,

    /// Print one matched path per line — equivalent to --format quiet
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}
