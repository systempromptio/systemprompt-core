//! Verified writes into the Drive folder, the trash and conflict siblings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use chrono::Utc;
use systemprompt_identifiers::{ContentHash, DocumentId, DocumentRevisionId};

use super::error::{DriveError, DriveResult};
use super::hash;
use super::root::DriveRoot;
use super::scan::metadata;
use super::state::DriveEntryState;

pub fn write_verified(path: &Path, bytes: &[u8], expected: &ContentHash) -> DriveResult<()> {
    hash::verify(expected, bytes)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| DriveError::io("create folder", parent, e))?;
    }
    crate::fsutil::atomic_write_0644(path, bytes)
        .map_err(|e| DriveError::io("verified write", path, e))
}

pub fn synced_entry(
    path: &Path,
    document_id: Option<DocumentId>,
    revision_id: DocumentRevisionId,
    sha256: ContentHash,
) -> DriveResult<DriveEntryState> {
    let (mtime_unix, size) = metadata(path)?;
    Ok(DriveEntryState {
        document_id,
        revision_id,
        sha256,
        mtime_unix,
        size,
    })
}

pub fn move_to_trash(root: &DriveRoot, rel: &str) -> DriveResult<Option<PathBuf>> {
    let source = root.file(rel)?;
    if !source.exists() {
        return Ok(None);
    }
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string();
    let target = root.trash_dir().join(stamp).join(rel);
    move_file(&source, &target)?;
    Ok(Some(target))
}

pub fn move_file(from: &Path, to: &Path) -> DriveResult<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| DriveError::io("create folder", parent, e))?;
    }
    std::fs::rename(from, to).map_err(|e| DriveError::io("move", from, e))
}

#[must_use]
pub fn conflict_sibling_name(rel: &str, host_name: &str, attempt: u32) -> String {
    let (dir, file) = rel.rsplit_once('/').map_or(("", rel), |(d, f)| (d, f));
    let (stem, ext) = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, Some(ext)),
        _ => (file, None),
    };
    let counter = if attempt > 1 {
        format!(" {attempt}")
    } else {
        String::new()
    };
    let name = ext.map_or_else(
        || format!("{stem} (conflict from {host_name}){counter}"),
        |ext| format!("{stem} (conflict from {host_name}){counter}.{ext}"),
    );
    if dir.is_empty() {
        name
    } else {
        format!("{dir}/{name}")
    }
}

pub fn free_conflict_sibling(root: &DriveRoot, rel: &str, host_name: &str) -> DriveResult<String> {
    let mut attempt = 1;
    loop {
        let candidate = conflict_sibling_name(rel, host_name, attempt);
        if !root.file(&candidate)?.exists() {
            return Ok(candidate);
        }
        attempt += 1;
    }
}
