//! Unified patches and line statistics for one file, and the compact summary
//! over many.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

const CONTEXT_LINES: usize = 3;
const DIFF_TIMEOUT: Duration = Duration::from_secs(2);
const BINARY_SNIFF_BYTES: usize = 8000;

/// How a file differs between the two sides of a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

/// One changed file: its worktree-relative path, line counts and a unified
/// patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
    pub patch: String,
}

/// Added and removed line counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineStats {
    pub additions: usize,
    pub deletions: usize,
}

/// Totals over a set of file diffs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub additions: usize,
    pub deletions: usize,
    pub files: usize,
}

impl DiffSummary {
    pub fn from_diffs<'a>(diffs: impl IntoIterator<Item = &'a FileDiff>) -> Self {
        diffs.into_iter().fold(Self::default(), |acc, diff| Self {
            additions: acc.additions + diff.additions,
            deletions: acc.deletions + diff.deletions,
            files: acc.files + 1,
        })
    }

    pub const fn is_empty(&self) -> bool {
        self.files == 0
    }
}

pub fn line_stats(old: &str, new: &str) -> LineStats {
    let diff = TextDiff::configure()
        .timeout(DIFF_TIMEOUT)
        .diff_lines(old, new);
    let mut stats = LineStats::default();
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => stats.additions += 1,
            ChangeTag::Delete => stats.deletions += 1,
            ChangeTag::Equal => {},
        }
    }
    stats
}

pub fn unified_diff(path: &str, old: &str, new: &str) -> String {
    let old_name = format!("a/{path}");
    let new_name = format!("b/{path}");
    render_patch(old, new, &old_name, &new_name)
}

pub fn file_diff(path: &str, old: Option<&[u8]>, new: Option<&[u8]>) -> FileDiff {
    let status = match (old, new) {
        (None, Some(_)) => FileStatus::Added,
        (Some(_), None) => FileStatus::Deleted,
        (Some(_), Some(_)) | (None, None) => FileStatus::Modified,
    };
    let old_bytes = old.unwrap_or_default();
    let new_bytes = new.unwrap_or_default();
    let old_name = if old.is_some() {
        format!("a/{path}")
    } else {
        "/dev/null".to_owned()
    };
    let new_name = if new.is_some() {
        format!("b/{path}")
    } else {
        "/dev/null".to_owned()
    };
    let relative = crate::paths::relative_path(path);

    let texts = (text_of(old_bytes), text_of(new_bytes));
    let (Some(old_text), Some(new_text)) = texts else {
        let binary_note = if old_bytes == new_bytes {
            String::new()
        } else {
            format!("Binary files {old_name} and {new_name} differ\n")
        };
        return FileDiff {
            path: relative,
            status,
            additions: 0,
            deletions: 0,
            binary: true,
            patch: binary_note,
        };
    };

    let counts = line_stats(old_text, new_text);
    FileDiff {
        path: relative,
        status,
        additions: counts.additions,
        deletions: counts.deletions,
        binary: false,
        patch: render_patch(old_text, new_text, &old_name, &new_name),
    }
}

fn render_patch(old: &str, new: &str, old_name: &str, new_name: &str) -> String {
    let diff = TextDiff::configure()
        .timeout(DIFF_TIMEOUT)
        .diff_lines(old, new);
    diff.unified_diff()
        .context_radius(CONTEXT_LINES)
        .header(old_name, new_name)
        .to_string()
}

fn text_of(bytes: &[u8]) -> Option<&str> {
    let sniff = &bytes[..bytes.len().min(BINARY_SNIFF_BYTES)];
    if sniff.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}
