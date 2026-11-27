//! The content search behind the grep tool: walks the tree (or reads one file),
//! skips ignored, oversized and binary files, and keeps each match with its
//! context lines up to the match limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use globset::GlobMatcher;
use ignore::WalkBuilder;
use regex::Regex;
use tokio_util::sync::CancellationToken;

use crate::config::SearchLimits;
use crate::error::ToolError;
use crate::path::slash;
use crate::text::{clip_chars, looks_binary};

#[derive(Debug)]
pub(super) struct Include {
    pub(super) matcher: GlobMatcher,
    pub(super) whole_path: bool,
}

#[derive(Debug)]
pub(super) struct Query {
    pub(super) regex: Regex,
    pub(super) include: Option<Include>,
    pub(super) context: usize,
    pub(super) limits: SearchLimits,
}

#[derive(Debug)]
pub(super) struct FileHits {
    pub(super) path: PathBuf,
    pub(super) modified: SystemTime,
    pub(super) rows: BTreeMap<usize, bool>,
    pub(super) lines: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct Found {
    pub(super) files: Vec<FileHits>,
    pub(super) matches: usize,
    pub(super) capped: bool,
}

fn wanted(path: &Path, root: &Path, include: Option<&Include>) -> bool {
    let Some(include) = include else { return true };
    if include.whole_path {
        path.strip_prefix(root)
            .is_ok_and(|relative| include.matcher.is_match(slash(relative)))
    } else {
        path.file_name()
            .is_some_and(|name| include.matcher.is_match(name))
    }
}

fn candidates(root: &Path) -> Box<dyn Iterator<Item = PathBuf>> {
    if root.is_file() {
        return Box::new(std::iter::once(root.to_path_buf()));
    }
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    Box::new(
        walker
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
            .map(ignore::DirEntry::into_path),
    )
}

pub(super) fn search(
    root: &Path,
    query: &Query,
    cancel: &CancellationToken,
) -> Result<Found, ToolError> {
    let mut found = Found::default();
    let base = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    for path in candidates(root) {
        if cancel.is_cancelled() {
            return Err(ToolError::Aborted);
        }
        if found.matches >= query.limits.grep_matches {
            found.capped = true;
            break;
        }
        if !wanted(&path, base, query.include.as_ref()) {
            continue;
        }
        let remaining = query.limits.grep_matches - found.matches;
        if let Some((hits, capped)) = scan_file(path, query, remaining) {
            found.matches += hits.rows.values().filter(|is_match| **is_match).count();
            found.capped |= capped;
            found.files.push(hits);
        }
    }
    Ok(found)
}

fn scan_file(path: PathBuf, query: &Query, room: usize) -> Option<(FileHits, bool)> {
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > query.limits.grep_max_file_bytes {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    if looks_binary(&bytes[..bytes.len().min(8192)]) {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let mut hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| query.regex.is_match(line))
        .map(|(at, _)| at)
        .collect();
    if hits.is_empty() {
        return None;
    }
    let capped = hits.len() > room;
    hits.truncate(room);
    let mut rows: BTreeMap<usize, bool> = BTreeMap::new();
    for &hit in &hits {
        let start = hit.saturating_sub(query.context);
        let end = (hit + query.context).min(lines.len().saturating_sub(1));
        for index in start..=end {
            rows.entry(index).or_insert(false);
        }
        rows.insert(hit, true);
    }
    let kept: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if !rows.contains_key(&index) {
                return String::new();
            }
            let (shown, cut) = clip_chars(line.trim_end(), query.limits.grep_line_chars);
            if cut {
                format!("{shown}…")
            } else {
                shown.to_owned()
            }
        })
        .collect();
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    Some((
        FileHits {
            path,
            modified,
            rows,
            lines: kept,
        },
        capped,
    ))
}
