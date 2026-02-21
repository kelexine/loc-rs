// loc — Advanced Lines of Code Counter
//
// Author : kelexine (https://github.com/kelexine)
// Version: Dynamic (Cargo.toml)
// License: MIT
//
// A complete Rust rewrite of the Python v4 LOC counter, with improvements:
//   • True data-parallelism via Rayon (all CPU cores)
//   • Zero-copy byte-level line counting
//   • Pre-compiled regex patterns via once_cell::Lazy
//   • Richer function extraction (Rust structs/impls, Python decorators/docstrings)
//   • Filesystem mtime fallback when git is unavailable
//   • walkdir traversal (faster than os.walk)
//   • Typed errors via anyhow — no silent panics

mod cli;
mod counter;
mod display;
mod export;
mod extractors;
mod language;
mod models;

use clap::Parser;
use colored::Colorize;
use std::process;

fn main() {
    let mut args = cli::Args::parse();

    // --func-analysis implicitly enables -f
    if args.func_analysis {
        args.functions = true;
    }

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

    // Display tree + summary
    display::display_results(
        &result,
        &config.target_dir,
        args.detailed,
        args.binary,
        config.warn_size,
    );

    // Optional function analysis
    if args.func_analysis {
        display::display_function_analysis(&result, &config.target_dir);
    }

    // Optional export
    if let Some(ref output_file) = args.export {
        if let Err(e) = export::export(&result, output_file, config.extract_functions) {
            eprintln!("{} {}", "[ERROR]".red().bold(), e);
            process::exit(1);
        }
    }
}
