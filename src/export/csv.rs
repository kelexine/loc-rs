// Author: kelexine (https://github.com/kelexine)
// export/csv.rs — CSV export logic

use crate::models::ScanResult;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub fn export_csv(result: &ScanResult, path: &Path, include_functions: bool) -> Result<()> {
    let f = File::create(path).with_context(|| format!("Cannot create {}", path.display()))?;
    let mut wtr = csv::Writer::from_writer(BufWriter::new(f));

    // Header
    if include_functions {
        wtr.write_record([
            "Path",
            "Lines",
            "Extension",
            "Functions",
            "Classes",
            "Avg Fn Length",
            "Last Modified",
        ])?;
    } else {
        wtr.write_record(["Path", "Lines", "Extension", "Last Modified"])?;
    }

    for fi in result.files.iter().filter(|f| !f.is_binary) {
        let last_mod = fi
            .last_modified
            .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default();

        if include_functions {
            wtr.write_record([
                fi.path.to_string_lossy().as_ref(),
                &fi.lines.to_string(),
                fi.extension(),
                &fi.function_count().to_string(),
                &fi.class_count().to_string(),
                &format!("{:.2}", fi.avg_function_length()),
                &last_mod,
            ])?;
        } else {
            wtr.write_record([
                fi.path.to_string_lossy().as_ref(),
                &fi.lines.to_string(),
                fi.extension(),
                &last_mod,
            ])?;
        }
    }

    wtr.flush()?;
    println!("[SUCCESS] Exported CSV → {}", path.display());
    Ok(())
}
