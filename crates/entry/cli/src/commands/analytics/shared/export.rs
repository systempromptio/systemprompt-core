use anyhow::{Context, Result};
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn export_to_csv<T: Serialize>(data: &[T], path: &Path) -> Result<()> {
    let file = File::create(path).context("Failed to create export file")?;
    let mut wtr = csv::Writer::from_writer(file);

    for record in data {
        wtr.serialize(record)?;
    }

    wtr.flush()?;
    Ok(())
}

pub fn export_single_to_csv<T: Serialize>(data: &T, path: &Path) -> Result<()> {
    let file = File::create(path).context("Failed to create export file")?;
    let mut wtr = csv::Writer::from_writer(file);
    wtr.serialize(data)?;
    wtr.flush()?;
    Ok(())
}

pub struct CsvBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl CsvBuilder {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
        }
    }

    pub fn headers(mut self, headers: Vec<&str>) -> Self {
        self.headers = headers.into_iter().map(String::from).collect();
        self
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        let mut file = File::create(path).context("Failed to create export file")?;

        writeln!(file, "{}", self.headers.join(","))?;

        for row in &self.rows {
            let escaped: Vec<String> = row
                .iter()
                .map(|cell| {
                    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.clone()
                    }
                })
                .collect();
            writeln!(file, "{}", escaped.join(","))?;
        }

        Ok(())
    }
}

impl Default for CsvBuilder {
    fn default() -> Self {
        Self::new()
    }
}
