// Author: kelexine (https://github.com/kelexine)
// agent/mod.rs — Agent detection and output-mode resolution.
//
// Reads the process environment via `agent_harnesses::detect()` and resolves
// the correct [`OutputMode`] for the current invocation.  All hint output
// goes to stderr so it never pollutes the agent's parsing pipeline.

pub mod harnesses;
use harnesses::{DetectionResult, detect};
use crate::cli::OutputFormat;

// ─── Output mode ─────────────────────────────────────────────────────────────

/// The rendering mode for this invocation.
///
/// Priority of resolution (highest → lowest):
/// 1. `-q` / `--quiet`   → [`OutputMode::Quiet`]
/// 2. `--json`            → [`OutputMode::Json`]   (legacy compat, kept as-is)
/// 3. `--format <mode>`  → the named mode
/// 4. Env-var detection  → [`OutputMode::Agent`] when a known harness is found
/// 5. Default            → [`OutputMode::Human`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Rich colored terminal output for humans: ANSI, padded tables, truncation.
    Human,
    /// Machine-readable TSV — no ANSI, nothing truncated, token-efficient.
    /// `--tree` renders a flat file list instead of ASCII art.
    Agent,
    /// JSON summary to stdout (existing `--json` behaviour, unchanged).
    Json,
    /// One matched path per line to stdout; useful for piping into other tools.
    Quiet,
}

/// Resolve the output mode and optionally the detected agent name.
///
/// Returns `(mode, detected_agent)` where `detected_agent` is `Some` when the
/// mode was auto-detected from env vars (used in the startup hint).
pub fn resolve_output_mode(
    format: Option<OutputFormat>,
    json_flag: bool,
    quiet_flag: bool,
) -> (OutputMode, Option<String>) {
    // Explicit flags are checked first — they always override auto-detection.
    if quiet_flag {
        return (OutputMode::Quiet, None);
    }
    if json_flag {
        return (OutputMode::Json, None);
    }
    if let Some(fmt) = format {
        return (
            match fmt {
                OutputFormat::Human => OutputMode::Human,
                OutputFormat::Agent => OutputMode::Agent,
                OutputFormat::Json  => OutputMode::Json,
                OutputFormat::Quiet => OutputMode::Quiet,
            },
            None,
        );
    }

    // Auto-detect from process environment.
    match detect() {
        DetectionResult::Known(key) => (OutputMode::Agent, Some(key.id().to_string())),
        DetectionResult::Unknown(v) => (OutputMode::Agent, Some(v)),
        DetectionResult::None       => (OutputMode::Human, None),
    }
}

// ─── Hint helpers ─────────────────────────────────────────────────────────────

/// Emit a single hint line to stderr.
///
/// Always stderr — never pollutes stdout data streams that agents parse.
#[inline]
pub fn hint(msg: &str) {
    eprintln!("Hint: {}", msg);
}

/// Emit contextual next-step hints based on which flags were used.
///
/// Called at the end of every run so both humans and agents always know
/// what to try next.  All output goes to stderr.
pub fn print_hints(
    mode: OutputMode,
    used_detailed: bool,
    used_tree: bool,
    used_functions: bool,
    used_func_analysis: bool,
    used_export: bool,
    detected_agent: Option<&str>,
) {
    match mode {
        OutputMode::Agent => {
            if let Some(agent) = detected_agent {
                eprintln!("# Agent mode auto-detected: {}", agent);
            }
            if !used_detailed {
                hint("Use -d for language breakdown table");
            }
            if !used_tree {
                hint("Use --tree for a flat TSV file list");
            }
            if !used_functions {
                hint("Use -f to embed function counts, --func-analysis for full complexity report");
            }
            if !used_export {
                hint("Use -e out.tsv to save results to file");
            }
            hint("Use --format human to switch to colored terminal output");
        }
        OutputMode::Human => {
            if !used_detailed && !used_tree && !used_functions {
                hint(
                    "Use -d for language breakdown, --tree for directory tree, \
                     -f to extract functions",
                );
            } else if !used_detailed {
                hint("Use -d for a per-language breakdown");
            } else if !used_functions && !used_func_analysis {
                hint("Use -f to extract functions, --func-analysis for complexity report");
            }
            if !used_export {
                hint("Use -e out.json / out.csv / out.html to export");
            }
            hint("Use --format agent for machine-readable TSV (auto-detected when run inside a coding agent)");
        }
        OutputMode::Json => {
            if !used_export {
                hint("Use -e out.json to persist the same data to a file");
            }
            hint("Use --format agent for TSV output (lighter on tokens)");
        }
        OutputMode::Quiet => {
            hint("Use -d for language breakdown, --json for full JSON summary");
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;

    #[test]
    fn quiet_flag_wins_over_json() {
        let (mode, agent) = resolve_output_mode(None, true, true);
        assert_eq!(mode, OutputMode::Quiet);
        assert!(agent.is_none());
    }

    #[test]
    fn json_flag_wins_over_format() {
        let (mode, _) = resolve_output_mode(Some(OutputFormat::Human), true, false);
        assert_eq!(mode, OutputMode::Json);
    }

    #[test]
    fn explicit_format_agent() {
        let (mode, _) = resolve_output_mode(Some(OutputFormat::Agent), false, false);
        assert_eq!(mode, OutputMode::Agent);
    }

    #[test]
    fn explicit_format_quiet() {
        let (mode, _) = resolve_output_mode(Some(OutputFormat::Quiet), false, false);
        assert_eq!(mode, OutputMode::Quiet);
    }

    #[test]
    fn auto_detect_known_agent_returns_agent_mode() {
        unsafe { std::env::set_var("CRUSH", "1") };
        let (mode, name) = resolve_output_mode(None, false, false);
        unsafe { std::env::remove_var("CRUSH") };
        assert_eq!(mode, OutputMode::Agent);
        assert_eq!(name.as_deref(), Some("crush"));
    }

    #[test]
    fn auto_detect_unknown_agent_returns_agent_mode() {
        unsafe { std::env::set_var("AI_AGENT", "my-custom-tool") };
        let (mode, name) = resolve_output_mode(None, false, false);
        unsafe { std::env::remove_var("AI_AGENT") };
        assert_eq!(mode, OutputMode::Agent);
        assert_eq!(name.as_deref(), Some("my-custom-tool"));
    }

    #[test]
    fn explicit_flag_beats_env_detection() {
        // Even if an agent env-var is set, --format human must win.
        unsafe { std::env::set_var("CRUSH", "1") };
        let (mode, _) = resolve_output_mode(Some(OutputFormat::Human), false, false);
        unsafe { std::env::remove_var("CRUSH") };
        assert_eq!(mode, OutputMode::Human);
    }
}
