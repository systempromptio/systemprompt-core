//! Log files read beneath a logs root, for the `logs` commands of the
//! capabilities that write them (`admin agents logs`, `plugins mcp logs`).
//!
//! [`confined_logs_dir`] resolves an operator-named `--logs-dir` against the
//! profile's logs root (a relative path resolves against it) and refuses one
//! that resolves outside it. Log files follow the `<prefix><name>.log`
//! convention; lookup also accepts a bare `<name>.log` path and a substring
//! match over the discovered set. The name is accepted only as a single path
//! segment, and every entry is resolved through
//! [`entries_within`](super::entries_within) /
//! [`locate_within`](super::locate_within), so neither a name nor a symbolic
//! link leads a read out of the logs directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::{PathConfinementError, confine_within, entries_within, locate_within, single_segment};

/// Why a log file could not be found or read.
#[derive(Debug, thiserror::Error)]
pub enum LogFileError {
    #[error(transparent)]
    Confinement(#[from] PathConfinementError),
    #[error("--logs-dir must name a directory inside {}", root.display())]
    OutsideLogsRoot {
        root: PathBuf,
        #[source]
        source: PathConfinementError,
    },
    #[error("log file not found for '{name}'; available: {available:?}")]
    NotFound {
        name: String,
        available: Vec<String>,
    },
    #[error("{} cannot be read", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn confined_logs_dir(
    logs_root: &Path,
    requested: Option<&str>,
) -> Result<PathBuf, LogFileError> {
    let Some(dir) = requested else {
        return Ok(logs_root.to_path_buf());
    };
    confine_within(logs_root, &logs_root.join(dir)).map_err(|source| {
        LogFileError::OutsideLogsRoot {
            root: logs_root.to_path_buf(),
            source,
        }
    })
}

pub fn list_log_files(logs_dir: &Path, prefix: &str) -> Result<Vec<String>, LogFileError> {
    let mut files = entries_within(logs_dir, logs_dir)?
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .filter_map(|entry| {
            entry
                .name
                .to_str()
                .filter(|name| {
                    name.starts_with(prefix)
                        && Path::new(name)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                })
                .map(String::from)
        })
        .collect::<Vec<_>>();

    files.sort();
    Ok(files)
}

pub fn find_log_file(
    logs_dir: &Path,
    prefix: &str,
    kind: &'static str,
    name: &str,
) -> Result<PathBuf, LogFileError> {
    let name = single_segment(kind, name)?;
    for candidate in [format!("{name}.log"), format!("{prefix}{name}.log")] {
        if let Some(found) = locate_within(logs_dir, &logs_dir.join(candidate))? {
            return Ok(found);
        }
    }

    let log_files = list_log_files(logs_dir, prefix)?;
    let found = log_files
        .iter()
        .find(|file| file.contains(name))
        .map(|file| logs_dir.join(file));
    found.ok_or_else(|| LogFileError::NotFound {
        name: name.to_owned(),
        available: log_files,
    })
}

pub fn read_log_lines(
    log_file: &Path,
    lines: usize,
    filter: impl Fn(&str) -> bool,
) -> Result<Vec<String>, LogFileError> {
    let read = |source| LogFileError::Read {
        path: log_file.to_path_buf(),
        source,
    };
    let file = std::fs::File::open(log_file).map_err(read)?;
    let filtered: Vec<String> = BufReader::new(file)
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(read)?
        .into_iter()
        .filter(|line| filter(line))
        .collect();

    let start = filtered.len().saturating_sub(lines);
    Ok(filtered[start..].to_vec())
}

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
