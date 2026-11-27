//! Writes into the Drive folder: verified downloads, guarded replacements,
//! moves, conflict copies and the trash.
//!
//! Every write checks the content against the digest it was fetched by,
//! re-checks the target's path for links just before it lands, and goes through
//! [`systemprompt_device::secure::write_atomic`] (staged beside the target, renamed over it, read
//! back). A remote deletion moves the synced copy into `.trash/<UTC
//! time>/<path>`, kept for [`TRASH_RETENTION_DAYS`] days.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use sp_protocol::ids::{DocumentId, DocumentRevisionId};
use systemprompt_device::secure::{Access, write_atomic};

use super::error::SyncError;
use super::index::IndexEntry;
use super::names::conflict_name;
use super::root::SyncRoot;
use super::scan::{hash, metadata};
use crate::hash::sha256_hex;

pub(crate) const TRASH_RETENTION_DAYS: i64 = 30;
const STAMP: &str = "%Y%m%dT%H%M%S%.3fZ";
const SIBLING_ATTEMPTS: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Guarded {
    Written,
    LocalChanged,
}

pub(crate) fn verify(expected: &str, bytes: &[u8]) -> Result<(), SyncError> {
    let actual = sha256_hex(bytes);
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(SyncError::HashMismatch {
            expected: expected.to_owned(),
            actual,
        })
    }
}

pub(crate) fn write_verified(
    root: &SyncRoot,
    path: &Path,
    bytes: &[u8],
    expected: &str,
) -> Result<(), SyncError> {
    verify(expected, bytes)?;
    root.confine(path)?;
    write_atomic(path, bytes, Access::Shared).map_err(SyncError::Secure)
}

pub(crate) fn write_unless_changed(
    root: &SyncRoot,
    path: &Path,
    bytes: &[u8],
    expected: &str,
    hashed_local: Option<&str>,
) -> Result<Guarded, SyncError> {
    verify(expected, bytes)?;
    let current = if path.exists() {
        Some(hash(path)?)
    } else {
        None
    };
    if current.as_deref() != hashed_local {
        return Ok(Guarded::LocalChanged);
    }
    root.confine(path)?;
    write_atomic(path, bytes, Access::Shared).map_err(SyncError::Secure)?;
    Ok(Guarded::Written)
}

pub(crate) fn move_file(root: &SyncRoot, from: &Path, to: &Path) -> Result<(), SyncError> {
    root.confine(from)?;
    root.confine(to)?;
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| SyncError::io("create folder", parent, error))?;
        root.confine(to)?;
    }
    std::fs::rename(from, to).map_err(|error| SyncError::io("move", from, error))
}

pub(crate) fn free_conflict_sibling(
    root: &SyncRoot,
    rel: &str,
    host: &str,
) -> Result<String, SyncError> {
    for attempt in 1..=SIBLING_ATTEMPTS {
        let candidate = conflict_name(rel, host, attempt);
        if !root.file(&candidate)?.exists() {
            return Ok(candidate);
        }
    }
    Err(SyncError::UnsafePath {
        path: rel.to_owned(),
    })
}

pub(crate) fn synced_entry(
    path: &Path,
    document_id: Option<DocumentId>,
    revision_id: DocumentRevisionId,
    sha256: String,
) -> Result<IndexEntry, SyncError> {
    let (mtime_unix, size) = metadata(path)?;
    Ok(IndexEntry {
        document_id,
        revision_id,
        sha256,
        mtime_unix,
        size,
    })
}

pub(crate) fn move_to_trash(
    root: &SyncRoot,
    rel: &str,
    now: DateTime<Utc>,
) -> Result<Option<PathBuf>, SyncError> {
    let source = root.file(rel)?;
    if !source.exists() {
        return Ok(None);
    }
    let target = root.trash_file(&now.format(STAMP).to_string(), rel)?;
    move_file(root, &source, &target)?;
    Ok(Some(target))
}

pub(crate) fn stamp_of(name: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(name, STAMP)
        .ok()
        .map(|naive| naive.and_utc())
}

pub(crate) fn prune_trash(root: &SyncRoot, now: DateTime<Utc>) -> Result<usize, SyncError> {
    let dir = root.trash_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(SyncError::io("list the Drive trash", &dir, error)),
    };
    let cutoff = now - chrono::Duration::days(TRASH_RETENTION_DAYS);
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(|error| SyncError::io("list the Drive trash", &dir, error))?;
        let expired = entry
            .file_name()
            .to_str()
            .and_then(stamp_of)
            .is_some_and(|stamp| stamp < cutoff);
        let is_dir = entry
            .file_type()
            .map_err(|error| SyncError::io("inspect", entry.path(), error))?
            .is_dir();
        if expired && is_dir {
            std::fs::remove_dir_all(entry.path())
                .map_err(|error| SyncError::io("empty the Drive trash", entry.path(), error))?;
            removed += 1;
        }
    }
    Ok(removed)
}
