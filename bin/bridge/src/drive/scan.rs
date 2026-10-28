//! Walking the Drive folder and finding the files that changed locally.
//!
//! A file is compared with its state entry by size and mtime first; only a
//! file whose metadata moved is hashed, and only a differing digest is a
//! change. Editor lock files, partial downloads, temp files and every hidden
//! name (the state file, `.trash`, staged writes) are never synced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_identifiers::ContentHash;

use super::error::{DriveError, DriveResult};
use super::hash::hash_file;
use super::root::DriveRoot;
use super::state::{DriveEntryState, DriveState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub rel: String,
    pub abs: PathBuf,
    pub mtime_unix: i64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalChange {
    pub file: LocalFile,
    pub sha256: ContentHash,
    pub base: Option<DriveEntryState>,
}

#[must_use]
pub fn is_ignored(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with('.')
        || lower.starts_with("~$")
        || lower.ends_with('~')
        || Path::new(&lower)
            .extension()
            .is_some_and(|ext| ext == "tmp" || ext == "crdownload")
}

pub fn walk(root: &DriveRoot) -> DriveResult<Vec<LocalFile>> {
    let mut out = Vec::new();
    if root.dir().is_dir() {
        walk_dir(root, root.dir(), &mut out)?;
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

fn walk_dir(root: &DriveRoot, dir: &Path, out: &mut Vec<LocalFile>) -> DriveResult<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| DriveError::io("list", dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| DriveError::io("list", dir, e))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            tracing::warn!(dir = %dir.display(), "drive skips a file whose name is not UTF-8");
            continue;
        };
        if is_ignored(name) {
            continue;
        }
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| DriveError::io("stat", &path, e))?;
        if file_type.is_dir() {
            walk_dir(root, &path, out)?;
        } else if file_type.is_file()
            && let Some(rel) = root.relative(&path)
        {
            let (mtime_unix, size) = metadata(&path)?;
            out.push(LocalFile {
                rel,
                abs: path,
                mtime_unix,
                size,
            });
        }
    }
    Ok(())
}

pub fn metadata(path: &Path) -> DriveResult<(i64, u64)> {
    let meta = std::fs::metadata(path).map_err(|e| DriveError::io("stat", path, e))?;
    let mtime = meta
        .modified()
        .map_err(|e| DriveError::io("stat mtime", path, e))?;
    let mtime_unix = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Ok((mtime_unix, meta.len()))
}

pub fn local_changes(root: &DriveRoot, state: &DriveState) -> DriveResult<Vec<LocalChange>> {
    let mut changes = Vec::new();
    for file in walk(root)? {
        let base = state.entries.get(&file.rel).cloned();
        if let Some(entry) = &base
            && entry.size == file.size
            && entry.mtime_unix == file.mtime_unix
        {
            continue;
        }
        let (sha256, _) = hash_file(&file.abs)?;
        if base.as_ref().is_some_and(|entry| entry.sha256 == sha256) {
            continue;
        }
        changes.push(LocalChange { file, sha256, base });
    }
    Ok(changes)
}
