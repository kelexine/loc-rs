// Author: kelexine (https://github.com/kelexine)
// display.rs — Colored terminal output, tree view, and analysis reports

use colored::*;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use crate::models::{Breakdown, FileInfo, ScanResult};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_num(n: usize) -> String {
    // Thousands-separator formatting
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn fmt_percent(part: usize, total: usize) -> String {
    if total == 0 {
        return "  0.00%".to_string();
    }
    format!("{:>7.2}%", part as f64 / total as f64 * 100.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree structure
// ─────────────────────────────────────────────────────────────────────────────

enum TreeNode<'a> {
    File(&'a FileInfo),
    Dir(BTreeMap<String, TreeNode<'a>>),
}

fn insert_into_tree<'a>(
    tree: &mut BTreeMap<String, TreeNode<'a>>,
    parts: &[&str],
    info: &'a FileInfo,
) {
    if parts.is_empty() {
        return;
    }

    let mut current_level = tree;
    let (dirs, file_name) = parts.split_at(parts.len() - 1);

    // Traverse or create directories iteratively
    for &dir in dirs {
        let node = current_level
            .entry(dir.to_string())
            .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));

        // Advance the mutable reference deeper into the tree
        match node {
            TreeNode::Dir(children) => {
                current_level = children;
            }
            TreeNode::File(_) => {
                return;
            }
        }
    }

    // Insert the actual file at the final destination
    if let Some(&name) = file_name.first() {
        current_level.insert(name.to_string(), TreeNode::File(info));
    }
}

fn build_tree<'a>(files: &'a [FileInfo], root: &Path) -> BTreeMap<String, TreeNode<'a>> {
    let mut tree: BTreeMap<String, TreeNode> = BTreeMap::new();
    for fi in files {
        let parts: Vec<&str> = if let Ok(rel) = fi.path.strip_prefix(root) {
            rel.iter().filter_map(|c| c.to_str()).collect()
        } else {
            fi.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| vec![n])
                .unwrap_or_default()
        };
        if !parts.is_empty() {
            insert_into_tree(&mut tree, &parts, fi);
        }
    }
    tree
}

fn print_tree_node(
    name: &str,
    node: &TreeNode,
    prefix: &str,
    is_last: bool,
    show_binary: bool,
    warn_size: Option<usize>,
) -> usize {
    let connector = if is_last { "└── " } else { "├── " };
    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
    let mut total = 0;

    match node {
        TreeNode::File(fi) => {
            if fi.is_binary && !show_binary {
                return 0;
            }

            let name_colored = if fi.is_binary {
                name.yellow().to_string()
            } else if fi.is_lockfile {
                name.bright_black().to_string()
            } else if fi.lines == 0 {
                name.cyan().to_string()
            } else {
                name.green().to_string()
            };

            let binary_tag = if fi.is_binary {
                format!(" {}", "[binary]".yellow())
            } else {
                String::new()
            };

            let lockfile_tag = if fi.is_lockfile {
                format!(" {}", "[lockfile]".bright_black())
            } else {
                String::new()
            };

            let warn_tag = if warn_size.map(|w| fi.lines > w).unwrap_or(false) {
                format!(" {}", "⚠ LARGE".red().bold())
            } else {
                String::new()
            };

            let func_tag = if fi.function_count() > 0 {
                format!(" {}", format!("[{} fn]", fi.function_count()).magenta())
            } else {
                String::new()
            };

            let date_tag = fi
                .last_modified
                .map(|d| format!(" {}", format!("[{}]", d.format("%Y-%m-%d")).dimmed()))
                .unwrap_or_default();

            // Lockfiles show no line count — their lines are not tracked.
            let lines_tag = if fi.is_binary || fi.is_lockfile {
                String::new()
            } else {
                format!(" {}", format!("({})", fmt_num(fi.lines)).bright_black())
            };

            println!(
                "{}{}{}{}{}{}{}{}{}",
                prefix,
                connector,
                name_colored,
                lines_tag,
                lockfile_tag,
                func_tag,
                date_tag,
                binary_tag,
                warn_tag
            );
            // Lockfiles do not contribute to the running directory total.
            if !fi.is_lockfile {
                total += fi.lines;
            }
        }
        TreeNode::Dir(children) => {
            println!("{}{}{}", prefix, connector, name.blue().bold());
            let count = children.len();
            for (i, (child_name, child_node)) in children.iter().enumerate() {
                let last = i == count - 1;
                total += print_tree_node(
                    child_name,
                    child_node,
                    &child_prefix,
                    last,
                    show_binary,
                    warn_size,
                );
            }
        }
    }

    total
}

// ─────────────────────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// Box & Table Formatting Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn visible_width(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

fn print_sep(widths: &[usize], left: &str, mid: &str, right: &str) {
    let parts: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    println!(
        "  {}{}{}",
        left.bright_black(),
        parts.join(&mid.bright_black().to_string()),
        right.bright_black()
    );
}

fn print_row(cols: &[(&str, usize, bool)]) {
    print!("  {}", "│".bright_black());
    for (text, width, right) in cols {
        let v_len = visible_width(text);
        let pad = width.saturating_sub(v_len);
        if *right {
            print!(" {}{} {}", " ".repeat(pad), text, "│".bright_black());
        } else {
            print!(" {}{} {}", text, " ".repeat(pad), "│".bright_black());
        }
    }
    println!();
}

fn print_full_width_row(text: &str, total_inner_width: usize, centered: bool) {
    let v_len = visible_width(text);
    let total_pad = total_inner_width.saturating_sub(v_len);
    if centered {
        let left_pad = total_pad / 2;
        let right_pad = total_pad.saturating_sub(left_pad);
        println!(
            "  {} {}{}{} {}",
            "│".bright_black(),
            " ".repeat(left_pad),
            text,
            " ".repeat(right_pad),
            "│".bright_black()
        );
    } else {
        println!(
            "  {} {}{} {}",
            "│".bright_black(),
            text,
            " ".repeat(total_pad),
            "│".bright_black()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public display functions
// ─────────────────────────────────────────────────────────────────────────────

/// Render summary output, optional tree view, and structured per-language breakdown.
pub fn display_results(
    result: &ScanResult,
    root: &Path,
    show_details: bool,
    show_binary: bool,
    show_tree: bool,
    warn_size: Option<usize>,
    show_functions: bool,
) {
    let total_lines: usize = result.files.iter().map(|f| f.lines).sum();
    let text_files = result.text_file_count();
    let bin_files = result.binary_file_count();
    let lockfile_count = result.lockfile_count();
    let total_fns = result.total_functions();
    let total_cls = result.total_classes();

    if show_tree {
        println!();
        println!("{}", "Project Structure:".bold());
        println!();
        let tree = build_tree(&result.files, root);
        let count = tree.len();
        for (i, (name, node)) in tree.iter().enumerate() {
            let last = i == count - 1;
            print_tree_node(name, node, "", last, show_binary, warn_size);
        }
        println!("{}", "━".repeat(90).bright_black());
    }

    println!();

    // ── Summary Box ─────────────────────────────────────────────────────────
    let sum_widths = [30, 25, 13];
    let top_title = "LOC-RS ANALYSIS SUMMARY".cyan().bold().to_string();
    println!("  {}", format!("┌{}┐", "─".repeat(76)).bright_black());
    print_full_width_row(&top_title, 74, true);
    print_sep(&sum_widths, "├", "┬", "┤");

    let h_metric = "Metric".bold().to_string();
    let h_count = "Count".bold().to_string();
    let h_share = "Share".bold().to_string();
    print_row(&[
        (&h_metric, 30, false),
        (&h_count, 25, true),
        (&h_share, 13, true),
    ]);
    print_sep(&sum_widths, "├", "┼", "┤");

    let code = result.total_code();
    let comment = result.total_comment();
    let blank = result.total_blank();

    let row_code = "Code".green().to_string();
    let val_code = fmt_num(code).green().to_string();
    let share_code = fmt_percent(code, total_lines).bright_black().to_string();
    print_row(&[
        (&row_code, 30, false),
        (&val_code, 25, true),
        (&share_code, 13, true),
    ]);

    let row_comment = "Comments".magenta().to_string();
    let val_comment = fmt_num(comment).magenta().to_string();
    let share_comment = fmt_percent(comment, total_lines).bright_black().to_string();
    print_row(&[
        (&row_comment, 30, false),
        (&val_comment, 25, true),
        (&share_comment, 13, true),
    ]);

    let row_blank = "Blank".dimmed().to_string();
    let val_blank = fmt_num(blank).dimmed().to_string();
    let share_blank = fmt_percent(blank, total_lines).bright_black().to_string();
    print_row(&[
        (&row_blank, 30, false),
        (&val_blank, 25, true),
        (&share_blank, 13, true),
    ]);

    print_sep(&sum_widths, "├", "┼", "┤");

    let row_total = "Total Lines".bold().to_string();
    let val_total = fmt_num(total_lines).bold().to_string();
    let share_total = "100.00%".bold().to_string();
    print_row(&[
        (&row_total, 30, false),
        (&val_total, 25, true),
        (&share_total, 13, true),
    ]);

    println!("  {}", format!("├{}┤", "─".repeat(76)).bright_black());

    let mut file_parts = vec![format!("{} text", fmt_num(text_files).blue())];
    if bin_files > 0 {
        file_parts.push(format!("{} binary", fmt_num(bin_files).yellow()));
    }
    if lockfile_count > 0 {
        file_parts.push(format!(
            "{} Lockfiles",
            fmt_num(lockfile_count).bright_black()
        ));
    }
    let file_summary = format!("Files: {}", file_parts.join("  │  "));
    print_full_width_row(&file_summary, 74, false);

    if show_functions && (total_fns > 0 || total_cls > 0) {
        let fn_summary = format!(
            "Functions: {}  │  Classes/Structs: {}",
            fmt_num(total_fns).magenta(),
            fmt_num(total_cls).magenta()
        );
        print_full_width_row(&fn_summary, 74, false);
    }

    println!("  {}", format!("└{}┘", "─".repeat(76)).bright_black());

    if let Some(ws) = warn_size {
        let large_files = result.files.iter().filter(|f| f.lines > ws).count();
        if large_files > 0 {
            println!(
                "  {} {}",
                "⚠ ".yellow().bold(),
                format!(
                    "{} files exceed the threshold of {} lines",
                    large_files,
                    fmt_num(ws)
                )
                .yellow()
            );
        }
    }

    // ── Per-Language Breakdown Table ─────────────────────────────────────────
    if show_details {
        println!();
        display_breakdown(&result.breakdown, total_lines, text_files, show_functions);
    } else {
        println!();
    }
}

fn display_breakdown(
    breakdown: &Breakdown,
    total_lines: usize,
    total_files: usize,
    has_functions: bool,
) {
    let mut sorted: Vec<_> = breakdown.iter().collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.1.lines));

    if has_functions {
        let widths = [22, 7, 10, 10, 9, 10, 8];
        print_sep(&widths, "┌", "┬", "┐");

        let h_lang = "Language".bold().to_string();
        let h_files = "Files".bold().to_string();
        let h_code = "Code".bold().to_string();
        let h_comm = "Comments".bold().to_string();
        let h_blank = "Blank".bold().to_string();
        let h_fns = "Functions".bold().to_string();
        let h_share = "Share".bold().to_string();

        print_row(&[
            (&h_lang, 22, false),
            (&h_files, 7, true),
            (&h_code, 10, true),
            (&h_comm, 10, true),
            (&h_blank, 9, true),
            (&h_fns, 10, true),
            (&h_share, 8, true),
        ]);
        print_sep(&widths, "├", "┼", "┤");

        let mut tot_code = 0;
        let mut tot_comm = 0;
        let mut tot_blank = 0;
        let mut tot_fns = 0;

        for (ext, stats) in &sorted {
            tot_code += stats.code;
            tot_comm += stats.comment;
            tot_blank += stats.blank;
            tot_fns += stats.functions;

            let ext_colored = match ext.as_str() {
                "Rust" | "rs" => ext.green(),
                "Python" | "py" => ext.yellow(),
                "JavaScript" | "TypeScript" | "TSX" | "JSX" | "js" | "ts" | "jsx" | "tsx" => {
                    ext.cyan()
                }
                "Go" | "go" => ext.blue(),
                "C" | "C Header" | "C++" | "C++ Header" | "c" | "cpp" | "h" | "hpp" => ext.red(),
                "Shell" | "sh" | "bash" | "zsh" => ext.magenta(),
                "Makefile" | "Kconfig" | "Dockerfile" | "CMake" | "Meson" | "Bazel"
                | "Justfile" | "Config" => ext.yellow(),
                "Device Tree" | "Device Tree Include" => ext.blue().bold(),
                "HTML" | "Vue" | "Svelte" | "CSS" | "SCSS" | "Sass" | "Less" => ext.blue(),
                "Markdown" => ext.white().bold(),
                "JSON" | "YAML" | "TOML" => ext.cyan(),
                "Assembly" => ext.green().bold(),
                "Unknown" => ext.dimmed(),
                _ => ext.white(),
            };
            let code_s = fmt_num(stats.code).bold().to_string();
            let comm_s = fmt_num(stats.comment).magenta().to_string();
            let blank_s = fmt_num(stats.blank).dimmed().to_string();
            let files_s = fmt_num(stats.files);
            let fns_s = fmt_num(stats.functions);
            let share_s = fmt_percent(stats.lines, total_lines)
                .bright_black()
                .to_string();

            print_row(&[
                (&ext_colored.to_string(), 22, false),
                (&files_s, 7, true),
                (&code_s, 10, true),
                (&comm_s, 10, true),
                (&blank_s, 9, true),
                (&fns_s, 10, true),
                (&share_s, 8, true),
            ]);
        }

        print_sep(&widths, "├", "┼", "┤");
        let t_label = "Total".bold().to_string();
        let t_files = fmt_num(total_files).bold().to_string();
        let t_code = fmt_num(tot_code).bold().to_string();
        let t_comm = fmt_num(tot_comm).magenta().to_string();
        let t_blank = fmt_num(tot_blank).dimmed().to_string();
        let t_fns = fmt_num(tot_fns).to_string();
        let t_share = "100.00%".bold().to_string();

        print_row(&[
            (&t_label, 22, false),
            (&t_files, 7, true),
            (&t_code, 10, true),
            (&t_comm, 10, true),
            (&t_blank, 9, true),
            (&t_fns, 10, true),
            (&t_share, 8, true),
        ]);
        print_sep(&widths, "└", "┴", "┘");
    } else {
        let widths = [24, 8, 12, 11, 11, 8];
        print_sep(&widths, "┌", "┬", "┐");

        let h_lang = "Language".bold().to_string();
        let h_files = "Files".bold().to_string();
        let h_code = "Code".bold().to_string();
        let h_comm = "Comments".bold().to_string();
        let h_blank = "Blank".bold().to_string();
        let h_share = "Share".bold().to_string();

        print_row(&[
            (&h_lang, 24, false),
            (&h_files, 8, true),
            (&h_code, 12, true),
            (&h_comm, 11, true),
            (&h_blank, 11, true),
            (&h_share, 8, true),
        ]);
        print_sep(&widths, "├", "┼", "┤");

        let mut tot_code = 0;
        let mut tot_comm = 0;
        let mut tot_blank = 0;

        for (ext, stats) in &sorted {
            tot_code += stats.code;
            tot_comm += stats.comment;
            tot_blank += stats.blank;

            let ext_colored = match ext.as_str() {
                "Rust" | "rs" => ext.green(),
                "Python" | "py" => ext.yellow(),
                "JavaScript" | "TypeScript" | "TSX" | "JSX" | "js" | "ts" | "jsx" | "tsx" => {
                    ext.cyan()
                }
                "Go" | "go" => ext.blue(),
                "C" | "C Header" | "C++" | "C++ Header" | "c" | "cpp" | "h" | "hpp" => ext.red(),
                "Shell" | "sh" | "bash" | "zsh" => ext.magenta(),
                "Makefile" | "Kconfig" | "Dockerfile" | "CMake" | "Meson" | "Bazel"
                | "Justfile" | "Config" => ext.yellow(),
                "Device Tree" | "Device Tree Include" => ext.blue().bold(),
                "HTML" | "Vue" | "Svelte" | "CSS" | "SCSS" | "Sass" | "Less" => ext.blue(),
                "Markdown" => ext.white().bold(),
                "JSON" | "YAML" | "TOML" => ext.cyan(),
                "Assembly" => ext.green().bold(),
                "Unknown" => ext.dimmed(),
                _ => ext.white(),
            };
            let code_s = fmt_num(stats.code).bold().to_string();
            let comm_s = fmt_num(stats.comment).magenta().to_string();
            let blank_s = fmt_num(stats.blank).dimmed().to_string();
            let files_s = fmt_num(stats.files);
            let share_s = fmt_percent(stats.lines, total_lines)
                .bright_black()
                .to_string();

            print_row(&[
                (&ext_colored.to_string(), 24, false),
                (&files_s, 8, true),
                (&code_s, 12, true),
                (&comm_s, 11, true),
                (&blank_s, 11, true),
                (&share_s, 8, true),
            ]);
        }

        print_sep(&widths, "├", "┼", "┤");
        let t_label = "Total".bold().to_string();
        let t_files = fmt_num(total_files).bold().to_string();
        let t_code = fmt_num(tot_code).bold().to_string();
        let t_comm = fmt_num(tot_comm).magenta().to_string();
        let t_blank = fmt_num(tot_blank).dimmed().to_string();
        let t_share = "100.00%".bold().to_string();

        print_row(&[
            (&t_label, 24, false),
            (&t_files, 8, true),
            (&t_code, 12, true),
            (&t_comm, 11, true),
            (&t_blank, 11, true),
            (&t_share, 8, true),
        ]);
        print_sep(&widths, "└", "┴", "┘");
    }
    println!();
}

/// Render the function-analysis report for extracted functions and classes.
pub fn display_function_analysis(result: &ScanResult, root: &Path) {
    let files_with_fns: Vec<_> = result
        .files
        .iter()
        .filter(|f| f.function_count() > 0)
        .collect();

    if files_with_fns.is_empty() {
        println!(
            "{}",
            "[WARN] No functions found in analyzed files.".yellow()
        );
        return;
    }

    println!("\n{}", "[INFO] Function Analysis Report".blue().bold());
    println!("{}", "=".repeat(90));
    println!();

    display_overall_stats(result, &files_with_fns);
    display_largest_functions(&files_with_fns, root);
    display_complex_functions(&files_with_fns, root);
    display_top_files(&files_with_fns, root);

    println!("{}", "=".repeat(90));
    println!();
}

fn display_overall_stats(result: &ScanResult, files_with_fns: &[&FileInfo]) {
    let total_fns = result.total_functions();
    let total_cls = result.total_classes();
    let non_class_fns: Vec<_> = files_with_fns
        .iter()
        .flat_map(|f| f.functions.iter().filter(|fn_| !fn_.is_class))
        .collect();
    let avg_len = if non_class_fns.is_empty() {
        0.0
    } else {
        non_class_fns.iter().map(|f| f.line_count()).sum::<usize>() as f64
            / non_class_fns.len() as f64
    };

    println!("{}", "Overall Statistics:".bold());
    println!("  Total Functions/Methods : {}", fmt_num(total_fns));
    println!("  Total Classes/Structs   : {}", fmt_num(total_cls));
    println!("  Average Function Length : {:.1} lines\n", avg_len);
}

fn display_largest_functions(files_with_fns: &[&FileInfo], root: &Path) {
    let mut all_fns: Vec<(&Path, &crate::models::FunctionInfo)> = files_with_fns
        .iter()
        .flat_map(|fi| {
            fi.functions
                .iter()
                .filter(|f| !f.is_class)
                .map(move |f| (fi.path.as_path(), f))
        })
        .collect();
    all_fns.sort_by_key(|a| std::cmp::Reverse(a.1.line_count()));

    if all_fns.is_empty() {
        return;
    }

    println!("{}", "Top 10 Largest Functions:".bold());
    println!(
        "{:<42} {:<32} {:>8} {:>12}",
        "Function", "File", "Lines", "Complexity"
    );
    println!("{}", "-".repeat(96));

    for (path, func) in all_fns.iter().take(10) {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());

        let complexity_str = if func.complexity > 10 {
            format!("{:>12}", func.complexity).red().to_string()
        } else if func.complexity > 5 {
            format!("{:>12}", func.complexity).yellow().to_string()
        } else {
            format!("{:>12}", func.complexity).green().to_string()
        };

        println!(
            "{:<42} {:<32} {:>8} {}",
            truncate(&func.name, 40),
            truncate(&rel, 30),
            fmt_num(func.line_count()),
            complexity_str
        );
    }
    println!();
}

fn display_complex_functions(files_with_fns: &[&FileInfo], root: &Path) {
    let mut complex_fns: Vec<_> = files_with_fns
        .iter()
        .flat_map(|fi| {
            fi.functions
                .iter()
                .filter(|f| !f.is_class && f.complexity > 10)
                .map(move |f| (fi.path.as_path(), f))
        })
        .collect();

    if complex_fns.is_empty() {
        return;
    }

    complex_fns.sort_by_key(|a| std::cmp::Reverse(a.1.complexity));
    println!("{}", "High Complexity Functions (>10):".bold());
    println!("{:<42} {:<32} {:>12}", "Function", "File", "Complexity");
    println!("{}", "-".repeat(86));

    for (path, func) in complex_fns.iter().take(15) {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());

        println!(
            "{:<42} {:<32} {}",
            truncate(&func.name, 40),
            truncate(&rel, 30),
            format!("{:>12}", func.complexity).red()
        );
    }
    println!();
}

fn display_top_files(files_with_fns: &[&FileInfo], root: &Path) {
    let mut sorted_files = files_with_fns.to_vec();
    sorted_files.sort_by_key(|b| std::cmp::Reverse(b.function_count()));

    println!("{}", "Top 10 Files by Function Count:\n".bold());

    for fi in sorted_files.iter().take(10) {
        let rel = fi
            .path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| fi.path.display().to_string());

        println!("{}", rel.cyan());
        println!(
            "  Functions: {}, Classes: {}, Avg length: {:.1} lines",
            fi.function_count(),
            fi.class_count(),
            fi.avg_function_length()
        );

        for func in fi.functions.iter().take(5) {
            let kind = match (func.is_class, func.is_async, func.is_method) {
                (true, _, _) => "class ",
                (_, true, _) => "async fn",
                (_, _, true) => "method  ",
                _ => "fn      ",
            };

            let params: Vec<_> = func.parameters.iter().take(3).cloned().collect();
            let ellipsis = if func.parameters.len() > 3 {
                ", ..."
            } else {
                ""
            };
            let complexity_note = if func.complexity > 5 {
                format!(" {}", format!("[cc={}]", func.complexity).yellow())
            } else {
                String::new()
            };

            println!(
                "    {} {}({}{}) — {} lines{}",
                kind.green(),
                func.name,
                params.join(", "),
                ellipsis,
                func.line_count(),
                complexity_note,
            );
        }

        if fi.functions.len() > 5 {
            println!(
                "    {} and {} more ...",
                "~".dimmed(),
                fi.functions.len() - 5
            );
        }
        println!();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility
// ─────────────────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("...{}", &s[s.len().saturating_sub(max - 3)..])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Agent-mode display (TSV, no ANSI)
// ─────────────────────────────────────────────────────────────────────────────

/// Render scan results as TSV to stdout for agent consumption.
///
/// Delegates every section to [`crate::export::tsv`] so stdout output and
/// `-e out.tsv` file export can never drift out of schema — same writer
/// functions, same column layout, byte-identical given the same flags.
///
/// Layout:
/// - `# SUMMARY` section — always written
/// - `# BREAKDOWN` section — one row per extension (if `show_details`)
/// - `# FILES` section — one row per text file (if `show_tree`)
/// - `# FUNCTION_STATS` / `# LARGEST_FUNCTIONS` / `# HIGH_COMPLEXITY` /
///   `# TOP_FILES` — if `show_func_analysis`
///
/// All output is raw TSV — no ANSI codes, no truncation, nothing padded.
/// Hints go to stderr via [`crate::agent::print_hints`]. Write errors to
/// stdout are ignored (mirroring the previous `.ok()` policy) since a
/// broken stdout pipe isn't something agent-mode output can meaningfully
/// recover from.
pub fn display_agent_tsv(
    result: &ScanResult,
    root: &Path,
    show_details: bool,
    show_tree: bool,
    show_functions: bool,
    show_func_analysis: bool,
    warn_size: Option<usize>,
) {
    use crate::export::tsv;

    let stdout = std::io::stdout();
    let mut w = std::io::BufWriter::new(stdout.lock());

    let _ = tsv::write_summary_section(&mut w, result, root, warn_size);

    if show_details {
        let _ = writeln!(w);
        let _ = tsv::write_breakdown_section(&mut w, result, show_functions);
    }

    if show_tree {
        let _ = writeln!(w);
        let _ = tsv::write_files_section(&mut w, result, root, show_functions);
    }

    if show_func_analysis {
        let _ = writeln!(w);
        let _ = tsv::write_function_analysis_sections(&mut w, result, root);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Quiet mode display
// ─────────────────────────────────────────────────────────────────────────────

/// Print one matched path per line to stdout.
///
/// Binary and lockfiles are included — callers can pipe into `grep`, `xargs`,
/// or other tools and filter themselves.  Paths are relative to `root`.
pub fn display_quiet(result: &ScanResult, root: &Path) {
    for fi in &result.files {
        let rel = fi
            .path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| fi.path.display().to_string());
        println!("{}", rel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FileInfo;
    use std::path::PathBuf;

    #[test]
    fn test_iterative_tree_building_deep() {
        let mut tree = BTreeMap::new();
        // Create a deep path: a/b/c/.../z/file.rs (26 levels)
        let mut path_parts = Vec::new();
        for i in 0..26 {
            path_parts.push(Box::leak(format!("{}", (b'a' + i) as char).into_boxed_str()) as &str);
        }
        let info = FileInfo::new(
            PathBuf::from("a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/file.rs"),
            10,
            10,
            0,
            0,
            false,
            None,
        );

        insert_into_tree(&mut tree, &path_parts, &info);

        // Verify the depth
        let mut node = &tree["a"];
        for i in 1..26 {
            match node {
                TreeNode::Dir(children) => {
                    node = &children[path_parts[i]];
                }
                _ => panic!("Expected directory at depth {}", i),
            }
        }

        match node {
            TreeNode::File(fi) => assert_eq!(fi.lines, 10),
            _ => panic!("Expected file at the leaf"),
        }
    }
}
