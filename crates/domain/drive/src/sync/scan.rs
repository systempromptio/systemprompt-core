//! Walking the Drive folder and finding the files that changed locally.
//!
//! A file is compared with its index entry by size and modification time first;
//! only a file whose metadata moved is hashed, and only a different digest is a
//! change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::error::SyncError;
use super::index::{IndexEntry, SyncIndex};
use super::names::is_ignored;
use super::root::SyncRoot;
use crate::hash::hash_file;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalFile {
    pub(crate) rel: String,
    pub(crate) abs: PathBuf,
    pub(crate) mtime_unix: i64,
    pub(crate) size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalChange {
    pub(crate) file: LocalFile,
    pub(crate) sha256: String,
    pub(crate) base: Option<IndexEntry>,
}

pub(crate) fn walk(root: &SyncRoot) -> Result<Vec<LocalFile>, SyncError> {
    let mut out = Vec::new();
    if root.dir().is_dir() {
        walk_dir(root, root.dir(), &mut out)?;
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

fn walk_dir(root: &SyncRoot, dir: &Path, out: &mut Vec<LocalFile>) -> Result<(), SyncError> {
    let entries = std::fs::read_dir(dir).map_err(|error| SyncError::io("list", dir, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| SyncError::io("list", dir, error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            tracing::warn!(dir = %dir.display(), "the Drive sync skips a name that is not UTF-8");
            continue;
        };
        if is_ignored(name) {
            continue;
        }
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| SyncError::io("inspect", &path, error))?;
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

pub(crate) fn metadata(path: &Path) -> Result<(i64, u64), SyncError> {
    let meta = std::fs::metadata(path).map_err(|error| SyncError::io("inspect", path, error))?;
    let modified = meta
        .modified()
        .map_err(|error| SyncError::io("read the modification time of", path, error))?;
    let mtime_unix = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
    Ok((mtime_unix, meta.len()))
}

pub(crate) fn hash(path: &Path) -> Result<String, SyncError> {
    hash_file(path)
        .map(|(sha, _)| sha)
        .map_err(|error| SyncError::io("hash", path, error))
}

pub(crate) fn local_changes(
    root: &SyncRoot,
    index: &SyncIndex,
) -> Result<Vec<LocalChange>, SyncError> {
    let mut changes = Vec::new();
    for file in walk(root)? {
        let base = index.entries.get(&file.rel).cloned();
        if let Some(entry) = &base
            && entry.size == file.size
            && entry.mtime_unix == file.mtime_unix
        {
            continue;
        }
        let sha256 = hash(&file.abs)?;
        if base.as_ref().is_some_and(|entry| entry.sha256 == sha256) {
            continue;
        }
        changes.push(LocalChange { file, sha256, base });
    }
    Ok(changes)
}
