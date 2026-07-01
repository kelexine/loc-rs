// Author: kelexine (https://github.com/kelexine)
// export/html.rs — Standalone HTML visual dashboard export
//
// Full data is always injected regardless of which CLI flags were passed;
// the dashboard hides/shows UI sections dynamically based on
// `function_extraction_enabled` / `function_analysis_enabled` /
// `warn_size` flags baked into the metadata. A section is either fully
// present with real data, or absent — never rendered half-populated
// because a flag happened to be off.

use super::json::file_to_value;
use crate::models::{FunctionInfo, ScanResult};
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Build the function-analysis payload for the dashboard: overall stats
/// plus the largest-functions / high-complexity / top-files tables.
/// Mirrors [`crate::export::tsv::write_function_analysis_sections`] but as
/// a JSON value with root-relative paths. Returns `None` when no file has
/// any extracted functions, so the dashboard can omit the section
/// entirely instead of rendering empty tables.
fn build_function_analysis_json(result: &ScanResult, root: &Path) -> Option<serde_json::Value> {
    let files_with_fns: Vec<_> = result
        .files
        .iter()
        .filter(|f| f.function_count() > 0)
        .collect();

    if files_with_fns.is_empty() {
        return None;
    }

    let rel = |p: &Path| -> String {
        p.strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| p.display().to_string())
    };

    let non_class_fns: Vec<_> = files_with_fns
        .iter()
        .flat_map(|f| f.functions.iter().filter(|fn_| !fn_.is_class))
        .collect();
    let avg_len = if non_class_fns.is_empty() {
        0.0_f64
    } else {
        non_class_fns.iter().map(|f| f.line_count()).sum::<usize>() as f64
            / non_class_fns.len() as f64
    };

    let mut all_fns: Vec<(&Path, &FunctionInfo)> = files_with_fns
        .iter()
        .flat_map(|fi| {
            fi.functions
                .iter()
                .filter(|f| !f.is_class)
                .map(move |f| (fi.path.as_path(), f))
        })
        .collect();
    all_fns.sort_by_key(|(_, func)| std::cmp::Reverse(func.line_count()));

    let largest_functions: Vec<_> = all_fns
        .iter()
        .take(10)
        .map(|(path, func)| {
            json!({
                "name": func.name,
                "file": rel(path),
                "lines": func.line_count(),
                "complexity": func.complexity,
                "parameters": func.parameters,
            })
        })
        .collect();

    let mut complex: Vec<_> = files_with_fns
        .iter()
        .flat_map(|fi| {
            fi.functions
                .iter()
                .filter(|f| !f.is_class && f.complexity > 10)
                .map(move |f| (fi.path.as_path(), f))
        })
        .collect();
    complex.sort_by_key(|(_, func)| std::cmp::Reverse(func.complexity));

    let high_complexity: Vec<_> = complex
        .iter()
        .take(15)
        .map(|(path, func)| {
            json!({
                "name": func.name,
                "file": rel(path),
                "complexity": func.complexity,
            })
        })
        .collect();

    let mut sorted_files = files_with_fns.clone();
    sorted_files.sort_by_key(|f| std::cmp::Reverse(f.function_count()));

    let top_files: Vec<_> = sorted_files
        .iter()
        .take(10)
        .map(|fi| {
            json!({
                "file": rel(&fi.path),
                "functions": fi.function_count(),
                "classes": fi.class_count(),
                "avg_fn_length": (fi.avg_function_length() * 100.0).round() / 100.0,
            })
        })
        .collect();

    Some(json!({
        "total_functions": result.total_functions(),
        "total_classes": result.total_classes(),
        "avg_function_length": (avg_len * 100.0).round() / 100.0,
        "largest_functions": largest_functions,
        "high_complexity": high_complexity,
        "top_files": top_files,
    }))
}

/// Export a standalone, self-contained HTML dashboard to `path`.
///
/// Unlike JSON/TSV export (which exclude binary/lockfile files from the
/// `files` array), the HTML report includes every scanned file so the
/// in-browser table can toggle visibility client-side without needing to
/// re-run the scan. Paths are always relative to `root`.
pub fn export_html(
    result: &ScanResult,
    path: &Path,
    root: &Path,
    extract_functions: bool,
    func_analysis: bool,
    warn_size: Option<usize>,
) -> Result<()> {
    let function_analysis = if func_analysis {
        build_function_analysis_json(result, root)
    } else {
        None
    };

    let all_files: Vec<_> = result
        .files
        .iter()
        .map(|f| file_to_value(f, extract_functions, Some(root)))
        .collect();

    let data = json!({
        "metadata": {
            "total_lines": result.total_lines(),
            "total_code": result.total_code(),
            "total_comment": result.total_comment(),
            "total_blank": result.total_blank(),
            "total_files": result.text_file_count(),
            "binary_files": result.binary_file_count(),
            "lockfiles": result.lockfile_count(),
            "total_functions": result.total_functions(),
            "total_classes": result.total_classes(),
            "timestamp": Utc::now().to_rfc3339(),
            "function_extraction_enabled": extract_functions,
            "function_analysis_enabled": func_analysis && function_analysis.is_some(),
            "warn_size": warn_size,
            "scan_dir": root.display().to_string(),
            "generator": concat!("loc v", env!("CARGO_PKG_VERSION"), " by kelexine (https://github.com/kelexine)"),
        },
        "breakdown": result.breakdown,
        "files": all_files,
        "function_analysis": function_analysis,
    });

    let json_data = serde_json::to_string(&data)?;
    let html_content = render_html(&json_data);

    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut writer = BufWriter::new(f);
    writer.write_all(html_content.as_bytes())?;

    println!("[SUCCESS] Exported HTML Visual Report → {}", path.display());
    Ok(())
}

/// Render the full HTML document, injecting `json_data` as the sole data
/// source. Every dynamic bit — chart data, table rows, section
/// visibility — is computed client-side in JS from this one payload, so
/// the Rust side never string-builds table rows or SVG by hand.
fn render_html(json_data: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>loc-rs | Visual Report</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
<style>
:root {{
    --bg: #0f172a;
    --panel: #141b2d;
    --card-bg: #1e293b;
    --card-bg-hover: #243044;
    --text-primary: #f8fafc;
    --text-secondary: #94a3b8;
    --text-dim: #64748b;
    --accent: #38bdf8;
    --accent-dim: #38bdf822;
    --border: #2c3a52;
    --success: #10b981;
    --warning: #f59e0b;
    --danger: #ef4444;
    --sidebar-w: 200px;
}}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, sans-serif;
    background-color: var(--bg);
    color: var(--text-primary);
    line-height: 1.5;
    display: flex;
    min-height: 100vh;
}}

/* ── Sidebar ─────────────────────────────────────────────────────── */
.sidebar {{
    width: var(--sidebar-w);
    background: var(--panel);
    border-right: 1px solid var(--border);
    padding: 1.5rem 1rem;
    position: fixed;
    top: 0; left: 0; bottom: 0;
    display: flex;
    flex-direction: column;
}}
.brand {{ font-size: 1.05rem; font-weight: 800; color: var(--accent); margin-bottom: 0.25rem; }}
.brand-sub {{ font-size: 0.7rem; color: var(--text-dim); margin-bottom: 1.75rem; word-break: break-all; }}
.nav-link {{
    display: block;
    padding: 0.55rem 0.75rem;
    margin-bottom: 0.25rem;
    border-radius: 0.5rem;
    color: var(--text-secondary);
    text-decoration: none;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
}}
.nav-link:hover {{ background: var(--card-bg); color: var(--text-primary); }}
.nav-link.active {{ background: var(--accent-dim); color: var(--accent); }}
.sidebar-footer {{ margin-top: auto; font-size: 0.7rem; color: var(--text-dim); }}
.sidebar-footer a {{ color: var(--accent); text-decoration: none; }}

/* ── Main content ────────────────────────────────────────────────── */
.main {{ margin-left: var(--sidebar-w); flex: 1; padding: 2rem; max-width: 1300px; }}
.view {{ display: none; }}
.view.active {{ display: block; }}

header.page-header {{
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    margin-bottom: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border);
}}
h1 {{ font-size: 1.35rem; font-weight: 700; }}
.timestamp, .scandir {{ font-size: 0.8rem; color: var(--text-secondary); }}

.stats-grid {{
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 1rem;
    margin-bottom: 2rem;
}}
.stat-card {{
    background: var(--card-bg);
    padding: 1.25rem;
    border-radius: 0.75rem;
    border: 1px solid var(--border);
    text-align: center;
}}
.stat-value {{ font-size: 1.75rem; font-weight: 800; display: block; }}
.stat-label {{ font-size: 0.75rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em; margin-top: 0.25rem; display: block; }}

.charts-row {{
    display: grid;
    grid-template-columns: 1fr 1.4fr;
    gap: 1.5rem;
    margin-bottom: 2rem;
}}
.chart-container {{
    background: var(--card-bg);
    padding: 1.25rem;
    border-radius: 0.75rem;
    border: 1px solid var(--border);
    min-height: 340px;
}}

.panel {{
    background: var(--card-bg);
    padding: 1rem;
    border-radius: 0.75rem;
    border: 1px solid var(--border);
    margin-bottom: 1.5rem;
    overflow-x: auto;
}}
.panel h2 {{ font-size: 1rem; margin: 0.25rem 0.5rem 0.75rem; color: var(--text-primary); }}

table {{ width: 100%; border-collapse: collapse; }}
th {{
    text-align: left;
    color: var(--text-secondary);
    font-size: 0.8rem;
    border-bottom: 1px solid var(--border);
    padding: 0.65rem 0.85rem;
    white-space: nowrap;
    cursor: pointer;
    user-select: none;
}}
th:hover {{ color: var(--text-primary); }}
th.sorted-asc::after {{ content: ' \2191'; color: var(--accent); }}
th.sorted-desc::after {{ content: ' \2193'; color: var(--accent); }}
td {{ padding: 0.6rem 0.85rem; border-bottom: 1px solid var(--border); font-size: 0.875rem; white-space: nowrap; }}
tr:last-child td {{ border-bottom: none; }}
tr.large-file td:first-child {{ border-left: 2px solid var(--warning); }}

.badge {{ display: inline-block; padding: 0.1rem 0.5rem; border-radius: 9999px; font-size: 0.7rem; font-weight: 600; }}
.badge-neutral {{ background: #33415522; color: var(--text-secondary); }}
.badge-low {{ background: #10b98122; color: var(--success); }}
.badge-med {{ background: #f59e0b22; color: var(--warning); }}
.badge-high {{ background: #ef444422; color: var(--danger); }}

.toolbar {{ display: flex; gap: 0.75rem; margin-bottom: 1rem; flex-wrap: wrap; align-items: center; }}
input[type="text"] {{
    flex: 1;
    min-width: 200px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: white;
    padding: 0.5rem 1rem;
    border-radius: 0.5rem;
    outline: none;
    font-size: 0.875rem;
}}
input[type="text"]:focus {{ border-color: var(--accent); }}
.result-count {{ font-size: 0.8rem; color: var(--text-dim); white-space: nowrap; }}

.empty-state {{ padding: 2.5rem; text-align: center; color: var(--text-dim); font-size: 0.9rem; }}
</style>
</head>
<body>

<nav class="sidebar">
    <div class="brand">loc-rs</div>
    <div class="brand-sub" id="scanDirLabel"></div>
    <a class="nav-link active" data-view="overview">Overview</a>
    <a class="nav-link" data-view="files">Files</a>
    <a class="nav-link" data-view="functions" id="functionsNavLink" style="display:none;">Functions</a>
    <div class="sidebar-footer">
        Generated by <a href="https://github.com/kelexine/loc-rs" target="_blank">loc-rs</a><br>
        by <a href="https://github.com/kelexine" target="_blank">kelexine</a>
    </div>
</nav>

<div class="main">

    <!-- ── Overview ──────────────────────────────────────────────── -->
    <section class="view active" id="view-overview">
        <header class="page-header">
            <h1>Overview</h1>
            <div id="timestamp" class="timestamp"></div>
        </header>
        <div class="stats-grid" id="statsGrid"></div>
        <div class="charts-row">
            <div class="chart-container"><canvas id="languageChart"></canvas></div>
            <div class="chart-container"><canvas id="compositionChart"></canvas></div>
        </div>
    </section>

    <!-- ── Files ─────────────────────────────────────────────────── -->
    <section class="view" id="view-files">
        <header class="page-header">
            <h1>Files</h1>
            <div class="scandir" id="scanDirFull"></div>
        </header>
        <div class="panel">
            <div class="toolbar">
                <input type="text" id="fileSearch" placeholder="Search by path or extension...">
                <span class="result-count" id="fileResultCount"></span>
            </div>
            <div style="overflow-x:auto;">
                <table id="filesTable">
                    <thead><tr id="filesTableHead"></tr></thead>
                    <tbody id="fileTableBody"></tbody>
                </table>
            </div>
        </div>
    </section>

    <!-- ── Functions ─────────────────────────────────────────────── -->
    <section class="view" id="view-functions">
        <header class="page-header">
            <h1>Function Analysis</h1>
        </header>
        <div class="stats-grid" id="fnStatsGrid"></div>
        <div class="panel">
            <h2>Largest Functions</h2>
            <table><thead><tr>
                <th>Function</th><th>File</th><th>Lines</th><th>Complexity</th><th>Parameters</th>
            </tr></thead><tbody id="largestFnBody"></tbody></table>
        </div>
        <div class="panel">
            <h2>High Complexity (cc &gt; 10)</h2>
            <table><thead><tr><th>Function</th><th>File</th><th>Complexity</th></tr></thead>
            <tbody id="highComplexityBody"></tbody></table>
        </div>
        <div class="panel">
            <h2>Top Files by Function Count</h2>
            <table><thead><tr>
                <th>File</th><th>Functions</th><th>Classes</th><th>Avg Fn Length</th>
            </tr></thead><tbody id="topFilesBody"></tbody></table>
        </div>
    </section>

</div>

<script>
const reportData = {data};
const meta = reportData.metadata;
const fnEnabled = !!meta.function_extraction_enabled;
const faEnabled = !!meta.function_analysis_enabled;
const warnSize = meta.warn_size;

// ── Navigation ─────────────────────────────────────────────────────
document.querySelectorAll('.nav-link').forEach(link => {{
    link.addEventListener('click', () => {{
        document.querySelectorAll('.nav-link').forEach(l => l.classList.remove('active'));
        document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
        link.classList.add('active');
        document.getElementById('view-' + link.dataset.view).classList.add('active');
    }});
}});

if (faEnabled) {{
    document.getElementById('functionsNavLink').style.display = 'block';
}}

// ── Header / scan dir ────────────────────────────────────────────────
document.getElementById('timestamp').textContent = 'Generated: ' + new Date(meta.timestamp).toLocaleString();
document.getElementById('scanDirLabel').textContent = meta.scan_dir;
document.getElementById('scanDirLabel').title = meta.scan_dir;
document.getElementById('scanDirFull').textContent = 'Scan root: ' + meta.scan_dir;

// ── Overview stat cards (dynamic based on enabled flags) ─────────────
function statCard(value, label) {{
    return `<div class="stat-card"><span class="stat-value">${{value.toLocaleString()}}</span><span class="stat-label">${{label}}</span></div>`;
}}
let statsHtml =
    statCard(meta.total_lines, 'Total Lines') +
    statCard(meta.total_code, 'Code') +
    statCard(meta.total_comment, 'Comments') +
    statCard(meta.total_blank, 'Blank') +
    statCard(meta.total_files, 'Text Files') +
    statCard(meta.binary_files, 'Binary Files') +
    statCard(meta.lockfiles, 'Lockfiles');
if (fnEnabled) {{
    statsHtml += statCard(meta.total_functions, 'Functions');
    statsHtml += statCard(meta.total_classes, 'Classes');
}}
document.getElementById('statsGrid').innerHTML = statsHtml;

// ── Language chart (doughnut) ─────────────────────────────────────
const breakdown = reportData.breakdown;
const langLabels = Object.keys(breakdown).sort((a, b) => breakdown[b].lines - breakdown[a].lines);
const langValues = langLabels.map(l => breakdown[l].lines);
const palette = ['#38bdf8', '#818cf8', '#c084fc', '#f472b6', '#fb7185', '#fb923c', '#fbbf24', '#a3e635', '#4ade80', '#2dd4bf'];

new Chart(document.getElementById('languageChart'), {{
    type: 'doughnut',
    data: {{
        labels: langLabels,
        datasets: [{{ data: langValues, backgroundColor: palette, borderWidth: 0 }}]
    }},
    options: {{
        maintainAspectRatio: false,
        plugins: {{
            legend: {{ position: 'bottom', labels: {{ color: '#94a3b8', boxWidth: 12 }} }},
            title: {{ display: true, text: 'Lines by Language', color: '#f8fafc', font: {{ size: 14 }} }}
        }}
    }}
}});

// ── Composition chart (stacked bar: code/comment/blank, top 8 exts) ─
const topExts = langLabels.slice(0, 8);
new Chart(document.getElementById('compositionChart'), {{
    type: 'bar',
    data: {{
        labels: topExts,
        datasets: [
            {{ label: 'Code', data: topExts.map(e => breakdown[e].code), backgroundColor: '#38bdf8' }},
            {{ label: 'Comment', data: topExts.map(e => breakdown[e].comment), backgroundColor: '#fbbf24' }},
            {{ label: 'Blank', data: topExts.map(e => breakdown[e].blank), backgroundColor: '#334155' }},
        ]
    }},
    options: {{
        maintainAspectRatio: false,
        scales: {{
            x: {{ stacked: true, ticks: {{ color: '#94a3b8' }}, grid: {{ color: '#2c3a52' }} }},
            y: {{ stacked: true, ticks: {{ color: '#94a3b8' }}, grid: {{ color: '#2c3a52' }} }}
        }},
        plugins: {{
            legend: {{ position: 'bottom', labels: {{ color: '#94a3b8', boxWidth: 12 }} }},
            title: {{ display: true, text: 'Composition by Extension (top 8)', color: '#f8fafc', font: {{ size: 14 }} }}
        }}
    }}
}});

// ── Files table (dynamic columns, sortable, searchable) ──────────────
const files = reportData.files;

const baseCols = [
    {{ key: 'path', label: 'Path' }},
    {{ key: 'lines', label: 'Lines' }},
    {{ key: 'code', label: 'Code' }},
    {{ key: 'comment', label: 'Comment' }},
    {{ key: 'blank', label: 'Blank' }},
    {{ key: 'extension', label: 'Ext' }},
    {{ key: 'is_binary', label: 'Binary' }},
    {{ key: 'is_lockfile', label: 'Lockfile' }},
];
const fnCols = [
    {{ key: 'function_count', label: 'Functions' }},
    {{ key: 'class_count', label: 'Classes' }},
    {{ key: 'avg_function_length', label: 'Avg Fn Len' }},
    {{ key: 'max_complexity', label: 'Max Complexity' }},
];
const tailCols = [{{ key: 'last_modified', label: 'Last Modified' }}];
const columns = fnEnabled ? baseCols.concat(fnCols, tailCols) : baseCols.concat(tailCols);

let sortKey = 'lines';
let sortDir = 'desc';

function maxComplexity(f) {{
    if (!f.functions || f.functions.length === 0) return 0;
    return Math.max(...f.functions.map(fn => fn.complexity));
}}
files.forEach(f => {{ f.max_complexity = maxComplexity(f); }});

function renderFilesHead() {{
    const head = document.getElementById('filesTableHead');
    head.innerHTML = columns.map(c => {{
        let cls = '';
        if (c.key === sortKey) cls = sortDir === 'asc' ? 'sorted-asc' : 'sorted-desc';
        return `<th data-key="${{c.key}}" class="${{cls}}">${{c.label}}</th>`;
    }}).join('');
    head.querySelectorAll('th').forEach(th => {{
        th.addEventListener('click', () => {{
            const key = th.dataset.key;
            if (sortKey === key) {{ sortDir = sortDir === 'asc' ? 'desc' : 'asc'; }}
            else {{ sortKey = key; sortDir = 'desc'; }}
            renderFilesHead();
            renderFilesBody(document.getElementById('fileSearch').value);
        }});
    }});
}}

function complexityBadge(v) {{
    if (v === 0) return '<span class="badge badge-neutral">-</span>';
    const cls = v > 15 ? 'badge-high' : (v > 7 ? 'badge-med' : 'badge-low');
    return `<span class="badge ${{cls}}">${{v}}</span>`;
}}
function boolBadge(v) {{
    return v ? '<span class="badge badge-med">yes</span>' : '<span class="badge badge-neutral">no</span>';
}}

function renderFilesBody(filter) {{
    const q = (filter || '').toLowerCase();
    let rows = files.filter(f =>
        f.path.toLowerCase().includes(q) || (f.extension || '').toLowerCase().includes(q)
    );
    rows.sort((a, b) => {{
        let av = a[sortKey], bv = b[sortKey];
        if (typeof av === 'string') {{ av = av.toLowerCase(); bv = (bv || '').toLowerCase(); }}
        if (av < bv) return sortDir === 'asc' ? -1 : 1;
        if (av > bv) return sortDir === 'asc' ? 1 : -1;
        return 0;
    }});
    document.getElementById('fileResultCount').textContent = rows.length.toLocaleString() + ' / ' + files.length.toLocaleString() + ' files';

    const body = document.getElementById('fileTableBody');
    if (rows.length === 0) {{
        body.innerHTML = `<tr><td colspan="${{columns.length}}"><div class="empty-state">No files match your search.</div></td></tr>`;
        return;
    }}
    body.innerHTML = rows.slice(0, 200).map(f => {{
        const isLarge = warnSize != null && f.lines > warnSize;
        let cells = `
            <td title="${{f.path}}">${{f.path}}</td>
            <td>${{f.lines.toLocaleString()}}</td>
            <td>${{f.code.toLocaleString()}}</td>
            <td>${{f.comment.toLocaleString()}}</td>
            <td>${{f.blank.toLocaleString()}}</td>
            <td>${{f.extension || '-'}}</td>
            <td>${{boolBadge(f.is_binary)}}</td>
            <td>${{boolBadge(f.is_lockfile)}}</td>
        `;
        if (fnEnabled) {{
            cells += `
                <td>${{(f.function_count || 0).toLocaleString()}}</td>
                <td>${{(f.class_count || 0).toLocaleString()}}</td>
                <td>${{(f.avg_function_length || 0).toFixed(2)}}</td>
                <td>${{complexityBadge(f.max_complexity)}}</td>
            `;
        }}
        cells += `<td>${{f.last_modified ? new Date(f.last_modified).toLocaleDateString() : '-'}}</td>`;
        return `<tr class="${{isLarge ? 'large-file' : ''}}">${{cells}}</tr>`;
    }}).join('');
}}

renderFilesHead();
renderFilesBody('');
document.getElementById('fileSearch').addEventListener('input', e => renderFilesBody(e.target.value));

// ── Function analysis view ────────────────────────────────────────
if (faEnabled && reportData.function_analysis) {{
    const fa = reportData.function_analysis;
    document.getElementById('fnStatsGrid').innerHTML =
        statCard(fa.total_functions, 'Total Functions') +
        statCard(fa.total_classes, 'Total Classes') +
        statCard(Math.round(fa.avg_function_length * 100) / 100, 'Avg Fn Length');

    document.getElementById('largestFnBody').innerHTML = fa.largest_functions.map(f => `
        <tr>
            <td>${{f.name}}</td>
            <td title="${{f.file}}">${{f.file}}</td>
            <td>${{f.lines}}</td>
            <td>${{complexityBadge(f.complexity)}}</td>
            <td>${{(f.parameters || []).join(', ') || '-'}}</td>
        </tr>
    `).join('') || `<tr><td colspan="5"><div class="empty-state">No data.</div></td></tr>`;

    document.getElementById('highComplexityBody').innerHTML = fa.high_complexity.map(f => `
        <tr>
            <td>${{f.name}}</td>
            <td title="${{f.file}}">${{f.file}}</td>
            <td>${{complexityBadge(f.complexity)}}</td>
        </tr>
    `).join('') || `<tr><td colspan="3"><div class="empty-state">No functions exceed complexity 10.</div></td></tr>`;

    document.getElementById('topFilesBody').innerHTML = fa.top_files.map(f => `
        <tr>
            <td title="${{f.file}}">${{f.file}}</td>
            <td>${{f.functions}}</td>
            <td>${{f.classes}}</td>
            <td>${{f.avg_fn_length.toFixed(2)}}</td>
        </tr>
    `).join('') || `<tr><td colspan="4"><div class="empty-state">No data.</div></td></tr>`;
}}
</script>
</body>
</html>
"#,
        data = json_data
    )
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ExtensionStats, FileInfo, FunctionInfo};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_result_with_functions() -> (ScanResult, PathBuf) {
        let root = PathBuf::from("/repo");
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "rs".to_string(),
            ExtensionStats { lines: 100, code: 80, comment: 10, blank: 10, files: 1, functions: 1 },
        );
        let file = FileInfo::new(PathBuf::from("/repo/src/main.rs"), 100, 80, 10, 10, false, None)
            .with_functions(vec![FunctionInfo {
                name: "main".to_string(),
                line_start: 1,
                line_end: 20,
                parameters: vec![],
                is_async: false,
                is_method: false,
                is_class: false,
                docstring: None,
                decorators: vec![],
                complexity: 15,
            }]);
        (ScanResult { files: vec![file], breakdown }, root)
    }

    fn make_plain_result() -> (ScanResult, PathBuf) {
        let root = PathBuf::from("/repo");
        let mut breakdown = HashMap::new();
        breakdown.insert(
            "rs".to_string(),
            ExtensionStats { lines: 50, code: 40, comment: 5, blank: 5, files: 1, functions: 0 },
        );
        (
            ScanResult {
                files: vec![FileInfo::new(PathBuf::from("/repo/src/lib.rs"), 50, 40, 5, 5, false, None)],
                breakdown,
            },
            root,
        )
    }

    #[test]
    fn build_function_analysis_json_none_when_no_functions() {
        let (result, root) = make_plain_result();
        assert!(build_function_analysis_json(&result, &root).is_none());
    }

    #[test]
    fn build_function_analysis_json_some_with_relative_paths() {
        let (result, root) = make_result_with_functions();
        let fa = build_function_analysis_json(&result, &root).unwrap();
        assert_eq!(fa["total_functions"], 1);
        assert_eq!(fa["largest_functions"][0]["file"], "src/main.rs");
        assert_eq!(fa["high_complexity"][0]["complexity"], 15);
        assert_eq!(fa["top_files"][0]["file"], "src/main.rs");
    }

    #[test]
    fn export_html_creates_file_and_embeds_data() {
        let (result, root) = make_result_with_functions();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.html");
        export_html(&result, &path, &root, true, true, Some(50)).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<!DOCTYPE html>"));
        assert!(contents.contains("\"src/main.rs\""), "expected relative path embedded");
        assert!(contents.contains("\"function_analysis_enabled\":true"));
        assert!(!contents.contains("/repo/src/main.rs"), "absolute path leaked into report");
    }

    #[test]
    fn export_html_omits_function_analysis_when_disabled() {
        let (result, root) = make_result_with_functions();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.html");
        export_html(&result, &path, &root, true, false, None).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\"function_analysis_enabled\":false"));
        assert!(contents.contains("\"function_analysis\":null"));
    }

    #[test]
    fn export_html_includes_binary_and_lockfiles_in_data() {
        let (mut result, root) = make_plain_result();
        result.files.push(
            FileInfo::new(PathBuf::from("/repo/bin/tool"), 0, 0, 0, 0, true, None)
        );
        result.files.push(
            FileInfo::new(PathBuf::from("/repo/Cargo.lock"), 500, 0, 0, 0, false, None)
                .mark_as_lockfile()
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.html");
        export_html(&result, &path, &root, false, false, None).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\"is_binary\":true"));
        assert!(contents.contains("\"is_lockfile\":true"));
    }
}
