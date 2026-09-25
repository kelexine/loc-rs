#![allow(missing_docs)]

// loc — Advanced Lines of Code Counter
//
// Author : kelexine (https://github.com/kelexine)
// Version: See Cargo.toml
// License: MIT
//
// Features:
//   • True data-parallelism via Rayon (all CPU cores)
//   • Zero-copy byte-level line counting
//   • Pre-compiled regex patterns via once_cell::Lazy
//   • Richer function extraction (Rust structs/impls, Python decorators/docstrings)
//   • Filesystem mtime fallback when git is unavailable
//   • walkdir traversal (faster than os.walk)
//   • Typed errors via anyhow — no silent panics
//   • Agent-aware output: auto-detects coding agents and switches to TSV
//   • --format human|agent|json|quiet  +  legacy --json  +  -q

mod agent;
mod cli;
mod config;
mod counter;
mod display;
mod export;
mod extractors;
mod language;
mod locignore;
mod models;

use agent::OutputMode;
use clap::Parser;
use colored::Colorize;
use std::process;

fn main() {
    let mut args = cli::Args::parse();

    // --func-analysis implicitly enables -f
    if args.func_analysis {
        args.functions = true;
    }

    // ── Resolve output mode ───────────────────────────────────────────────────
    let (mode, detected_agent) = agent::resolve_output_mode(args.format, args.json, args.quiet);

    let config = match counter::ScanConfig::from_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {}", "[ERROR]".red().bold(), e);
            process::exit(1);
        }
    };

    let result = match counter::run_scan(&config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} {}", "[ERROR]".red().bold(), e);
            process::exit(1);
        }
    };

    // ── Dispatch by mode ──────────────────────────────────────────────────────
    match mode {
        // ── JSON (legacy --json or --format json) ─────────────────────────────
        OutputMode::Json => {
            let mut warnings = Vec::new();
            let mut hints = Vec::new();

            if args.func_analysis {
                warnings.push(
                    "--func-analysis is not supported with JSON output; \
                     use -f to embed function data in the JSON"
                        .to_string(),
                );
            }

            if let Some(ws) = config.warn_size {
                let large_files = result.files.iter().filter(|f| f.lines > ws).count();
                if large_files > 0 {
                    warnings.push(format!(
                        "{} files exceed the threshold of {} lines",
                        large_files, ws
                    ));
                }
            }

            if args.export.is_none() {
                hints.push("Use -e <file>.json to persist these results to disk".to_string());
            }
            hints.push("Use --format agent for compact TSV output (lower token cost)".to_string());

            if let Some(agent) = detected_agent.as_deref() {
                hints.push(format!("Detected agent environment: {agent}"));
            }

            if let Err(e) =
                export::json::print_json_stats(&result, config.extract_functions, &warnings, &hints)
            {
                eprintln!("{} {}", "[ERROR]".red().bold(), e);
                process::exit(1);
            }
        }

        // ── Agent (TSV, no ANSI) ──────────────────────────────────────────────
        OutputMode::Agent => {
            display::display_agent_tsv(
                &result,
                &config.target_dir,
                args.detailed,
                args.tree,
                config.extract_functions,
                args.func_analysis,
                config.warn_size,
            );
            agent::print_hints(
                mode,
                args.detailed,
                args.tree,
                args.functions,
                args.func_analysis,
                args.export.is_some(),
                detected_agent.as_deref(),
            );
        }

        // ── Quiet (one path per line) ─────────────────────────────────────────
        OutputMode::Quiet => {
            display::display_quiet(&result, &config.target_dir);
        }

        // ── Human (coloured terminal) ─────────────────────────────────────────
        OutputMode::Human => {
            display::display_results(
                &result,
                &config.target_dir,
                args.detailed,
                args.binary,
                args.tree,
                config.warn_size,
                config.extract_functions,
            );
            if args.func_analysis {
                display::display_function_analysis(&result, &config.target_dir);
            }
        }
    }

    // ── Export (always honoured regardless of mode) ───────────────────────────
    if let Some(ref output_file) = args.export
        && let Err(e) = export::export(
            &result,
            output_file,
            &config.target_dir,
            config.extract_functions,
            args.func_analysis,
            config.warn_size,
        )
    {
        eprintln!("{} {}", "[ERROR]".red().bold(), e);
        process::exit(1);
    }
}
