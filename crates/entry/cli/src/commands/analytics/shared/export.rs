use anyhow::{Context, Result};
use serde::Serialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use systemprompt_models::AppPaths;

pub fn resolve_export_path(user_path: &Path) -> Result<PathBuf> {
    if user_path.is_absolute()
        || user_path
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty())
    {
        return Ok(user_path.to_path_buf());
    }

    let exports_dir = AppPaths::get()
        .context("AppPaths not initialized - use an absolute path for export")?
        .storage()
        .exports()
        .to_path_buf();

    Ok(exports_dir.join(user_path))
}

pub fn ensure_export_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).context("Failed to create export directory")?;
        }
    }
    Ok(())
}

pub fn export_to_csv<T: Serialize>(data: &[T], path: &Path) -> Result<()> {
    ensure_export_dir(path)?;
    let file = File::create(path).context("Failed to create export file")?;
    let mut writer = std::io::BufWriter::new(file);

    if data.is_empty() {
        return Ok(());
    }

    let json_value = serde_json::to_value(&data[0])?;
    if let serde_json::Value::Object(obj) = json_value {
        let headers: Vec<&str> = obj.keys().map(String::as_str).collect();
        writeln!(writer, "{}", headers.join(","))?;
    }

    for record in data {
        let json = serde_json::to_value(record)?;
        if let serde_json::Value::Object(obj) = json {
            let values: Vec<String> = obj
                .values()
                .map(|v| match v {
                    serde_json::Value::String(s) => escape_csv_field(s),
                    serde_json::Value::Null => String::new(),
                    _ => v.to_string(),
                })
                .collect();
            writeln!(writer, "{}", values.join(","))?;
        }
    }

    writer.flush()?;
    Ok(())
}

pub fn export_single_to_csv<T: Serialize>(data: &T, path: &Path) -> Result<()> {
    ensure_export_dir(path)?;
    let file = File::create(path).context("Failed to create export file")?;
    let mut writer = std::io::BufWriter::new(file);

    let json = serde_json::to_value(data)?;
    if let serde_json::Value::Object(obj) = json {
        let headers: Vec<&str> = obj.keys().map(String::as_str).collect();
        writeln!(writer, "{}", headers.join(","))?;

        let values: Vec<String> = obj
            .values()
            .map(|v| match v {
                serde_json::Value::String(s) => escape_csv_field(s),
                serde_json::Value::Null => String::new(),
                _ => v.to_string(),
            })
            .collect();
        writeln!(writer, "{}", values.join(","))?;
    }

    writer.flush()?;
    Ok(())
}

fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[derive(Debug)]
pub struct CsvBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl CsvBuilder {
    pub const fn new() -> Self {
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
        ensure_export_dir(path)?;
        let mut file = File::create(path).context("Failed to create export file")?;

        writeln!(file, "{}", self.headers.join(","))?;

        for row in &self.rows {
            let escaped: Vec<String> = row.iter().map(|cell| escape_csv_field(cell)).collect();
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
