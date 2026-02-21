// Author: kelexine (https://github.com/kelexine)
// display.rs — Colored terminal output, tree view, and analysis reports

use std::collections::BTreeMap;
use std::path::Path;
use colored::*;

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
    if total == 0 { return "  0.00%".to_string(); }
    format!("{:>7.2}%", part as f64 / total as f64 * 100.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree structure
// ─────────────────────────────────────────────────────────────────────────────

enum TreeNode<'a> {
    File(&'a FileInfo),
    Dir(BTreeMap<String, TreeNode<'a>>),
}

fn insert_into_tree<'a>(tree: &mut BTreeMap<String, TreeNode<'a>>, parts: &[&str], info: &'a FileInfo) {
    if parts.is_empty() { return; }

    if parts.len() == 1 {
        tree.insert(parts[0].to_string(), TreeNode::File(info));
    } else {
        let dir = tree.entry(parts[0].to_string())
            .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));
        if let TreeNode::Dir(children) = dir {
            insert_into_tree(children, &parts[1..], info);
        }
    }
}

fn build_tree<'a>(files: &'a [FileInfo], root: &Path) -> BTreeMap<String, TreeNode<'a>> {
    let mut tree: BTreeMap<String, TreeNode> = BTreeMap::new();
    for fi in files {
        if let Ok(rel) = fi.path.strip_prefix(root) {
            let parts: Vec<&str> = rel.iter().filter_map(|c| c.to_str()).collect();
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
            if fi.is_binary && !show_binary { return 0; }

            let name_colored = if fi.is_binary {
                name.yellow().to_string()
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

            let date_tag = fi.last_modified
                .map(|d| format!(" {}", format!("[{}]", d.format("%Y-%m-%d")).dimmed()))
                .unwrap_or_default();

            let lines_tag = if fi.is_binary {
                String::new()
            } else {
                format!(" {}", format!("({})", fmt_num(fi.lines)).bright_black())
            };

            println!("{}{}{}{}{}{}{}{}", prefix, connector, name_colored, lines_tag, func_tag, date_tag, binary_tag, warn_tag);
            total += fi.lines;
        }
        TreeNode::Dir(children) => {
            println!("{}{}{}", prefix, connector, name.blue().bold());
            let count = children.len();
            for (i, (child_name, child_node)) in children.iter().enumerate() {
                let last = i == count - 1;
                total += print_tree_node(child_name, child_node, &child_prefix, last, show_binary, warn_size);
            }
        }
    }

    total
}

// ─────────────────────────────────────────────────────────────────────────────
// Public display functions
// ─────────────────────────────────────────────────────────────────────────────

pub fn display_results(
    result: &ScanResult,
    root: &Path,
    show_details: bool,
    show_binary: bool,
    warn_size: Option<usize>,
) {
    println!();
    println!("{}", "Directory Structure:".bold());
    println!();

    let tree = build_tree(&result.files, root);
    let count = tree.len();
    let mut total_lines = 0;
    for (i, (name, node)) in tree.iter().enumerate() {
        let is_last = i == count - 1;
        total_lines += print_tree_node(name, &node, "", is_last, show_binary, warn_size);
    }

    println!();
    println!("{}", "=".repeat(70));
    println!("{} {}", "[SUCCESS]".green().bold(), format!("Total Lines of Code: {}", fmt_num(total_lines)).bold());
    println!("{} {}", "[INFO]   ".blue(), format!("Text Files: {}", fmt_num(result.text_file_count())));

    let bin_count = result.binary_file_count();
    if bin_count > 0 {
        println!("{} {}", "[INFO]   ".blue(), format!("Binary Files Skipped: {}", fmt_num(bin_count)));
    }

    if result.total_functions() > 0 {
        println!("{} {}", "[INFO]   ".blue(), format!("Functions/Methods: {}", fmt_num(result.total_functions())));
        println!("{} {}", "[INFO]   ".blue(), format!("Classes/Structs:   {}", fmt_num(result.total_classes())));
    }

    if let Some(ws) = warn_size {
        let large: Vec<_> = result.files.iter().filter(|f| f.lines > ws).collect();
        if !large.is_empty() {
            println!("{} {} files exceed {} lines", "[WARN]   ".yellow(), large.len(), ws);
        }
    }

    println!("{}", "=".repeat(70));
    println!();

    if show_details {
        display_breakdown(&result.breakdown, total_lines, result.total_functions() > 0);
    }
}

fn display_breakdown(breakdown: &Breakdown, total_lines: usize, has_functions: bool) {
    println!();
    println!("{}", "[INFO] Breakdown by extension:".blue());
    println!();

    let mut sorted: Vec<_> = breakdown.iter().collect();
    sorted.sort_by(|a, b| b.1.lines.cmp(&a.1.lines));

    if has_functions {
        println!("{:<20} {:>14} {:>10} {:>12} {:>10}", "Extension", "Lines", "Files", "Functions", "Share");
        println!("{}", "-".repeat(68));
    } else {
        println!("{:<20} {:>14} {:>10} {:>10}", "Extension", "Lines", "Files", "Share");
        println!("{}", "-".repeat(56));
    }

    for (ext, stats) in &sorted {
        if has_functions {
            println!(
                "{:<20} {:>14} {:>10} {:>12} {:>10}",
                ext,
                fmt_num(stats.lines),
                fmt_num(stats.files),
                fmt_num(stats.functions),
                fmt_percent(stats.lines, total_lines),
            );
        } else {
            println!(
                "{:<20} {:>14} {:>10} {:>10}",
                ext,
                fmt_num(stats.lines),
                fmt_num(stats.files),
                fmt_percent(stats.lines, total_lines),
            );
        }
    }
    println!();
}

pub fn display_function_analysis(result: &ScanResult, root: &Path) {
    let files_with_fns: Vec<_> = result.files.iter().filter(|f| f.function_count() > 0).collect();

    if files_with_fns.is_empty() {
        println!("{}", "[WARN] No functions found in analyzed files.".yellow());
        return;
    }

    println!();
    println!("{}", "[INFO] Function Analysis Report".blue().bold());
    println!("{}", "=".repeat(90));
    println!();

    let total_fns = result.total_functions();
    let total_cls = result.total_classes();
    let avg_len = if total_fns > 0 {
        files_with_fns.iter()
            .flat_map(|f| f.functions.iter().filter(|fn_| !fn_.is_class))
            .map(|f| f.line_count())
            .sum::<usize>() as f64 / total_fns as f64
    } else { 0.0 };

    println!("{}", "Overall Statistics:".bold());
    println!("  Total Functions/Methods : {}", fmt_num(total_fns));
    println!("  Total Classes/Structs   : {}", fmt_num(total_cls));
    println!("  Average Function Length : {:.1} lines", avg_len);
    println!();

    // Top 10 largest functions
    let mut all_fns: Vec<(&Path, &crate::models::FunctionInfo)> = files_with_fns
        .iter()
        .flat_map(|fi| fi.functions.iter().filter(|f| !f.is_class).map(move |f| (fi.path.as_path(), f)))
        .collect();
    all_fns.sort_by(|a, b| b.1.line_count().cmp(&a.1.line_count()));

    if !all_fns.is_empty() {
        println!("{}", "Top 10 Largest Functions:".bold());
        println!("{:<42} {:<32} {:>8} {:>12}", "Function", "File", "Lines", "Complexity");
        println!("{}", "-".repeat(96));
        for (path, func) in all_fns.iter().take(10) {
            let rel = path.strip_prefix(root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            let fname = truncate(&func.name, 40);
            let file = truncate(&rel, 30);
            let complexity_str = if func.complexity > 10 {
                format!("{:>12}", func.complexity).red().to_string()
            } else if func.complexity > 5 {
                format!("{:>12}", func.complexity).yellow().to_string()
            } else {
                format!("{:>12}", func.complexity).green().to_string()
            };
            println!("{:<42} {:<32} {:>8} {}", fname, file, fmt_num(func.line_count()), complexity_str);
        }
        println!();
    }

    // High-complexity functions
    let mut complex_fns: Vec<_> = files_with_fns.iter()
        .flat_map(|fi| fi.functions.iter().filter(|f| !f.is_class && f.complexity > 10).map(move |f| (fi.path.as_path(), f)))
        .collect();

    if !complex_fns.is_empty() {
        complex_fns.sort_by(|a, b| b.1.complexity.cmp(&a.1.complexity));
        println!("{}", "High Complexity Functions (>10):".bold());
        println!("{:<42} {:<32} {:>12}", "Function", "File", "Complexity");
        println!("{}", "-".repeat(86));
        for (path, func) in complex_fns.iter().take(15) {
            let rel = path.strip_prefix(root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            let fname = truncate(&func.name, 40);
            let file = truncate(&rel, 30);
            println!("{:<42} {:<32} {}", fname, file, format!("{:>12}", func.complexity).red());
        }
        println!();
    }

    // Top 10 files by function count
    let mut sorted_files = files_with_fns.clone();
    sorted_files.sort_by(|a, b| b.function_count().cmp(&a.function_count()));

    println!("{}", "Top 10 Files by Function Count:".bold());
    println!();

    for fi in sorted_files.iter().take(10) {
        let rel = fi.path.strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| fi.path.display().to_string());

        println!("{}", rel.cyan());
        println!("  Functions: {}, Classes: {}, Avg length: {:.1} lines",
            fi.function_count(), fi.class_count(), fi.avg_function_length());

        for func in fi.functions.iter().take(5) {
            let kind = match (func.is_class, func.is_async, func.is_method) {
                (true, _, _) => "class ",
                (_, true, _) => "async fn",
                (_, _, true) => "method  ",
                _ => "fn      ",
            };
            let params: String = func.parameters.iter().take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let ellipsis = if func.parameters.len() > 3 { ", ..." } else { "" };
            let complexity_note = if func.complexity > 5 {
                format!(" {}", format!("[cc={}]", func.complexity).yellow())
            } else { String::new() };

            println!("    {} {}({}{}) — {} lines{}",
                kind.green(),
                func.name,
                params,
                ellipsis,
                func.line_count(),
                complexity_note,
            );
        }
        if fi.functions.len() > 5 {
            println!("    {} and {} more ...", "~".dimmed(), fi.functions.len() - 5);
        }
        println!();
    }

    println!("{}", "=".repeat(90));
    println!();
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
