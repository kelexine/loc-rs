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
/// Human mode emits no hints — `--help` covers feature discovery.
/// All other modes write to stderr so the stdout data stream is never
/// polluted.
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
        // ── Human — no hints; --help covers discovery ─────────────────────
        OutputMode::Human => {}

        // ── Agent (TSV) ───────────────────────────────────────────────────
        OutputMode::Agent => {
            // Banner: identifies which harness triggered auto-detection.
            if let Some(agent) = detected_agent {
                eprintln!("# Agent-Detected: {agent}");
            }

            // Suggest flags that would add more data to the current output.
            if !used_detailed {
                hint("Use -d to include a per-language breakdown (code / comment / blank)");
            }
            if !used_tree {
                hint("Use --tree to include a flat TSV file list with per-file metrics");
            }

            // Escalate function hints: neither → combined hint; -f only → suggest --func-analysis.
            // Note: --func-analysis auto-enables -f, so they are not independent flags.
            if !used_functions && !used_func_analysis {
                hint("Use --func-analysis for function counts + full cyclomatic-complexity report \
                      (or just -f for counts only; --func-analysis implies -f)");
            } else if used_functions && !used_func_analysis {
                hint("Use --func-analysis for a full cyclomatic-complexity report (implies -f, already active)");
            }

            if !used_export {
                hint("Use -e <file>.tsv to write these results to disk");
            }
            hint("Use --format human to switch to Coloured terminal output");
        }

        // ── JSON ──────────────────────────────────────────────────────────
        OutputMode::Json => {
            if !used_export {
                hint("Use -e <file>.json to persist these results to disk");
            }
            hint("Use --format agent for compact TSV output (lower token cost)");
        }

        // ── Quiet (one path per line) ─────────────────────────────────────
        OutputMode::Quiet => {
            hint("Use -d for a per-language summary or --json for the full structured report");
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
