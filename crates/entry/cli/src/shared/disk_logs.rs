//! On-disk log-file discovery and tailing shared by the agent and MCP `logs`
//! commands.
//!
//! Log files follow the `<prefix>-<name>.log` convention; lookup also accepts
//! bare `<name>.log` paths and substring matches over the discovered set.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

/// Lists `.log` files in `logs_dir` whose names start with `prefix`, sorted.
pub fn list_log_files(logs_dir: &Path, prefix: &str) -> Result<Vec<String>> {
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

/// Resolves a log file for `name`: exact `<name>.log`, then
/// `<prefix><name>.log`, then the first prefixed file containing `name`.
pub fn find_log_file(logs_dir: &Path, prefix: &str, name: &str) -> Result<PathBuf> {
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
        .ok_or_else(|| {
            anyhow!(
                "Log file not found for '{}'. Available: {:?}",
                name,
                log_files
            )
        })
}

/// Reads the last `lines` lines of `log_file` that satisfy `filter`.
pub fn read_log_lines(
    log_file: &Path,
    lines: usize,
    filter: impl Fn(&str) -> bool,
) -> Result<Vec<String>> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(log_file)
        .with_context(|| format!("Failed to open log file: {}", log_file.display()))?;

    let reader = BufReader::new(file);
    let filtered: Vec<String> = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to read log lines")?
        .into_iter()
        .filter(|line| filter(line))
        .collect();

    let start = filtered.len().saturating_sub(lines);
    Ok(filtered[start..].to_vec())
}

/// Strips `prefix` and the `.log` suffix from each file name, yielding the
/// bare service/agent names offered in interactive selection.
#[must_use]
pub fn display_names(log_files: &[String], prefix: &str) -> Vec<String> {
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
