// Author: kelexine (https://github.com/kelexine)
// export/html.rs — HTML visual report export logic

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use crate::models::ScanResult;
use super::json::file_to_value;

pub fn export_html(result: &ScanResult, path: &Path, extract_functions: bool) -> Result<()> {
    let text_files: Vec<_> = result.files.iter().filter(|f| !f.is_binary).collect();
    
    // Prepare the data to inject into JS
    let data = json!({
        "metadata": {
            "total_lines": result.total_lines(),
            "total_files": result.text_file_count(),
            "total_functions": result.total_functions(),
            "total_classes": result.total_classes(),
            "timestamp": Utc::now().to_rfc3339(),
            "function_extraction_enabled": extract_functions,
            "generator": concat!("loc v", env!("CARGO_PKG_VERSION")),
        },
        "breakdown": result.breakdown,
        "files": text_files.iter().map(|f| file_to_value(f, extract_functions)).collect::<Vec<_>>(),
    });

    let json_data = serde_json::to_string(&data)?;
    let html_content = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>loc-rs | Visual Report</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        :root {{
            --bg: #0f172a;
            --card-bg: #1e293b;
            --text-primary: #f8fafc;
            --text-secondary: #94a3b8;
            --accent: #38bdf8;
            --border: #334155;
            --success: #10b981;
            --warning: #f59e0b;
        }}

        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: 'Inter', -apple-system, sans-serif;
            background-color: var(--bg);
            color: var(--text-primary);
            line-height: 1.5;
            padding: 2rem;
        }}

        .container {{ max-width: 1200px; margin: 0 auto; }}
        
        header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 1px solid var(--border);
        }}

        h1 {{ font-size: 1.5rem; font-weight: 700; color: var(--accent); }}
        .timestamp {{ font-size: 0.875rem; color: var(--text-secondary); }}

        /* Stats Grid */
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
            margin-bottom: 2rem;
        }}

        .stat-card {{
            background: var(--card-bg);
            padding: 1.5rem;
            border-radius: 0.75rem;
            border: 1px solid var(--border);
            text-align: center;
        }}

        .stat-value {{ font-size: 2rem; font-weight: 800; display: block; }}
        .stat-label {{ font-size: 0.875rem; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.05em; }}

        /* Charts Section */
        .charts-row {{
            display: grid;
            grid-template-columns: 1fr 1.5fr;
            gap: 2rem;
            margin-bottom: 2rem;
        }}

        .chart-container {{
            background: var(--card-bg);
            padding: 1.5rem;
            border-radius: 0.75rem;
            border: 1px solid var(--border);
            min-height: 350px;
        }}

        /* Table */
        .table-container {{
            background: var(--card-bg);
            padding: 1rem;
            border-radius: 0.75rem;
            border: 1px solid var(--border);
            overflow-x: auto;
        }}

        table {{ width: 100%; border-collapse: collapse; }}
        th {{ text-align: left; color: var(--text-secondary); font-size: 0.875rem; border-bottom: 1px solid var(--border); padding: 0.75rem 1rem; }}
        td {{ padding: 0.75rem 1rem; border-bottom: 1px solid var(--border); font-size: 0.9375rem; }}
        tr:last-child td {{ border-bottom: none; }}
        
        .complexity-badge {{
            display: inline-block;
            padding: 0.125rem 0.5rem;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 600;
        }}
        .low-complexity {{ background: #10b98122; color: #10b981; }}
        .med-complexity {{ background: #f59e0b22; color: #f59e0b; }}
        .high-complexity {{ background: #ef444422; color: #ef4444; }}

        .search-container {{ margin-bottom: 1rem; }}
        input[type="text"] {{
            width: 100%;
            background: var(--bg);
            border: 1px solid var(--border);
            color: white;
            padding: 0.5rem 1rem;
            border-radius: 0.5rem;
            outline: none;
        }}
        input[type="text"]:focus {{ border-color: var(--accent); }}

    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>loc-rs Dashboard</h1>
            <div id="timestamp" class="timestamp"></div>
        </header>

        <div class="stats-grid">
            <div class="stat-card">
                <span id="totalLines" class="stat-value">-</span>
                <span class="stat-label">Total Lines</span>
            </div>
            <div class="stat-card">
                <span id="totalFiles" class="stat-value">-</span>
                <span class="stat-label">Text Files</span>
            </div>
            <div class="stat-card">
                <span id="totalFunctions" class="stat-value">-</span>
                <span class="stat-label">Functions</span>
            </div>
            <div class="stat-card">
                <span id="totalClasses" class="stat-value">-</span>
                <span class="stat-label">Classes</span>
            </div>
        </div>

        <div class="charts-row">
            <div class="chart-container">
                <canvas id="languageChart"></canvas>
            </div>
            <div class="chart-container">
                <div class="search-container">
                    <input type="text" id="fileSearch" placeholder="Search files...">
                </div>
                <div class="table-container">
                    <table>
                        <thead>
                            <tr>
                                <th>Path</th>
                                <th>Lines</th>
                                <th id="complexityHeader">Max Complexity</th>
                            </tr>
                        </thead>
                        <tbody id="fileTableBody"></tbody>
                    </table>
                </div>
            </div>
        </div>
    </div>

    <script>
        const reportData = {data};

        // Render Header & Stats
        document.getElementById('timestamp').textContent = 'Generated: ' + new Date(reportData.metadata.timestamp).toLocaleString();
        document.getElementById('totalLines').textContent = reportData.metadata.total_lines.toLocaleString();
        document.getElementById('totalFiles').textContent = reportData.metadata.total_files.toLocaleString();
        document.getElementById('totalFunctions').textContent = reportData.metadata.total_functions.toLocaleString();
        document.getElementById('totalClasses').textContent = reportData.metadata.total_classes.toLocaleString();

        // Language Chart
        const breakdown = reportData.breakdown;
        const labels = Object.keys(breakdown).sort((a,b) => breakdown[b].lines - breakdown[a].lines);
        const values = labels.map(l => breakdown[l].lines);
        
        new Chart(document.getElementById('languageChart'), {{
            type: 'doughnut',
            data: {{
                labels: labels,
                datasets: [{{
                    data: values,
                    backgroundColor: [
                        '#38bdf8', '#818cf8', '#c084fc', '#f472b6', '#fb7185',
                        '#fb923c', '#fbbf24', '#a3e635', '#4ade80', '#2dd4bf'
                    ],
                    borderWidth: 0
                }}]
            }},
            options: {{
                maintainAspectRatio: false,
                plugins: {{
                    legend: {{ position: 'bottom', labels: {{ color: '#94a3b8' }} }},
                    title: {{ display: true, text: 'Lines by Language', color: '#f8fafc', font: {{ size: 16 }} }}
                }}
            }}
        }});

        // File Table
        const tableBody = document.getElementById('fileTableBody');
        const files = reportData.files;

        function getMaxComplexity(f) {{
            if (!f.functions || f.functions.length === 0) return 0;
            return Math.max(...f.functions.map(fn => fn.complexity));
        }}

        function renderTable(filter = '') {{
            tableBody.innerHTML = '';
            files
                .filter(f => f.path.toLowerCase().includes(filter.toLowerCase()))
                .sort((a, b) => b.lines - a.lines)
                .slice(0, 50)
                .forEach(f => {{
                    const maxComp = getMaxComplexity(f);
                    const compClass = maxComp > 15 ? 'high-complexity' : (maxComp > 7 ? 'med-complexity' : 'low-complexity');
                    
                    const row = document.createElement('tr');
                    row.innerHTML = `
                        <td>${{f.path}}</td>
                        <td>${{f.lines.toLocaleString()}}</td>
                        <td><span class="complexity-badge ${{compClass}}">${{maxComp > 0 ? maxComp : '-'}}</span></td>
                    `;
                    tableBody.appendChild(row);
                }});
        }}

        document.getElementById('fileSearch').addEventListener('input', (e) => renderTable(e.target.value));
        renderTable();
    </script>
</body>
</html>
"#, data = json_data);

    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut writer = BufWriter::new(f);
    writer.write_all(html_content.as_bytes())?;

    eprintln!("[SUCCESS] Exported HTML Visual Report → {}", path.display());
    Ok(())
}
