//! On-disk MCP log-file discovery and tailing for `plugins mcp logs`.
//!
//! Log files follow the `<prefix><name>.log` convention; lookup also accepts
//! a bare `<name>.log` path and a substring match over the discovered set.
//! The server name is accepted only as a single path segment, so a lookup
//! never leaves the logs directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use systemprompt_models::path_confine::single_segment;

pub(super) fn list_log_files(logs_dir: &Path, prefix: &str) -> Result<Vec<String>> {
    let mut files = std::fs::read_dir(logs_dir)
        .context("Failed to read logs directory")?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            path.file_name()
                .and_then(|n| n.to_str())
                .filter(|name| {
                    name.starts_with(prefix)
                        && path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                })
                .map(String::from)
        })
        .collect::<Vec<_>>();

    files.sort();
    Ok(files)
}

pub(super) fn find_log_file(logs_dir: &Path, prefix: &str, name: &str) -> Result<PathBuf> {
    let name = single_segment("MCP server name", name)?;
    let exact_path = logs_dir.join(format!("{name}.log"));
    if exact_path.exists() {
        return Ok(exact_path);
    }

    let prefixed_path = logs_dir.join(format!("{prefix}{name}.log"));
    if prefixed_path.exists() {
        return Ok(prefixed_path);
    }

    let log_files = list_log_files(logs_dir, prefix)?;
    log_files
        .iter()
        .find(|file| file.contains(name))
        .map(|file| logs_dir.join(file))
        .ok_or_else(|| anyhow!("Log file not found for '{name}'. Available: {log_files:?}"))
}

pub(super) fn read_log_lines(
    log_file: &Path,
    lines: usize,
    filter: impl Fn(&str) -> bool,
) -> Result<Vec<String>> {
    let file = std::fs::File::open(log_file)
        .with_context(|| format!("Failed to open log file: {}", log_file.display()))?;

    let filtered: Vec<String> = BufReader::new(file)
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to read log lines")?
        .into_iter()
        .filter(|line| filter(line))
        .collect();

    let start = filtered.len().saturating_sub(lines);
    Ok(filtered[start..].to_vec())
}

pub(super) fn display_names(log_files: &[String], prefix: &str) -> Vec<String> {
    log_files
        .iter()
        .map(|f| {
            f.strip_prefix(prefix)
                .unwrap_or(f)
                .strip_suffix(".log")
                .unwrap_or(f)
                .to_owned()
        })
        .collect()
}
