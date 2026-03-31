use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Command;
use systemprompt_logging::CliService;

use super::{BackupFormat, adjust_ssl_mode, find_pg_dump};

const fn backup_extension(format: BackupFormat) -> &'static str {
    match format {
        BackupFormat::Custom => "dump",
        BackupFormat::Sql => "sql",
        BackupFormat::Directory => "dir",
    }
}

const fn pg_dump_format_flag(format: BackupFormat) -> &'static str {
    match format {
        BackupFormat::Custom => "-Fc",
        BackupFormat::Sql => "-Fp",
        BackupFormat::Directory => "-Fd",
    }
}

pub fn execute(
    profile_name: &str,
    database_url: &str,
    format: BackupFormat,
    output: Option<&str>,
) -> Result<()> {
    let pg_dump = find_pg_dump()?;

    let output_path = if let Some(p) = output {
        PathBuf::from(p)
    } else {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
        let ext = backup_extension(format);
        let dir = PathBuf::from("backups");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create {}", dir.display()))?;
        dir.join(format!("{}-{}.{}", profile_name, timestamp, ext))
    };

    CliService::section("Cloud Database Backup");
    CliService::key_value("Profile", profile_name);
    CliService::key_value("Format", &format!("{:?}", format));
    CliService::key_value("Output", &output_path.display().to_string());

    let adjusted_url = adjust_ssl_mode(database_url);
    let spinner = CliService::spinner("Running pg_dump...");

    let result = Command::new(&pg_dump)
        .arg(pg_dump_format_flag(format))
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg("-f")
        .arg(&output_path)
        .arg(&adjusted_url)
        .output()
        .context("Failed to execute pg_dump")?;

    spinner.finish_and_clear();

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("pg_dump failed:\n{}", stderr);
    }

    let size = if output_path.is_dir() {
        dir_size(&output_path)
    } else {
        std::fs::metadata(&output_path).map_or(0, |m| m.len())
    };

    CliService::success(&format!(
        "Backup complete: {} ({})",
        output_path.display(),
        format_size(size)
    ));

    Ok(())
}

fn dir_size(path: &PathBuf) -> u64 {
    std::fs::read_dir(path).map_or(0, |entries| {
        entries
            .filter_map(std::result::Result::ok)
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    })
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
