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
        ])?;
    } else {
        wtr.write_record(["Path", "Lines", "Extension"])?;
    }

    for fi in result.files.iter().filter(|f| !f.is_binary) {
        if include_functions {
            wtr.write_record([
                fi.path.to_string_lossy().as_ref(),
                &fi.lines.to_string(),
                fi.extension(),
                &fi.function_count().to_string(),
                &fi.class_count().to_string(),
                &format!("{:.2}", fi.avg_function_length()),
            ])?;
        } else {
            wtr.write_record([
                fi.path.to_string_lossy().as_ref(),
                &fi.lines.to_string(),
                fi.extension(),
            ])?;
        }
    }

    wtr.flush()?;
    println!("[SUCCESS] Exported CSV → {}", path.display());
    Ok(())
}
