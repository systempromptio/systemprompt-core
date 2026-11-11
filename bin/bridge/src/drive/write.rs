//! Verified writes into the Drive folder, the trash and conflict siblings.
//!
//! Every write stages beside its target and re-checks the target's path for
//! links ([`DriveRoot::confine`]) immediately before the rename.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::{ContentHash, DocumentId, DocumentRevisionId};
use tempfile::NamedTempFile;

use super::error::{DriveError, DriveResult};
use super::hash;
use super::root::DriveRoot;
use super::scan::metadata;
use super::state::DriveEntryState;

pub fn write_verified(
    root: &DriveRoot,
    path: &Path,
    bytes: &[u8],
    expected: &ContentHash,
) -> DriveResult<()> {
    hash::verify(expected, bytes)?;
    let staged = stage(root, path, bytes)?;
    land(root, path, staged, bytes)
}

/// Whether a guarded write landed or found the local file changed since it
/// was hashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardedWrite {
    Written,
    LocalChanged,
}

pub fn write_unless_changed(
    root: &DriveRoot,
    path: &Path,
    bytes: &[u8],
    expected: &ContentHash,
    hashed_local: Option<&ContentHash>,
) -> DriveResult<GuardedWrite> {
    hash::verify(expected, bytes)?;
    let staged = stage(root, path, bytes)?;
    let current = if path.exists() {
        Some(hash::hash_file(path)?.0)
    } else {
        None
    };
    if current.as_ref() != hashed_local {
        return Ok(GuardedWrite::LocalChanged);
    }
    land(root, path, staged, bytes)?;
    Ok(GuardedWrite::Written)
}

fn stage(root: &DriveRoot, path: &Path, bytes: &[u8]) -> DriveResult<NamedTempFile> {
    root.create_parent(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| DriveError::UnsafePath(path.display().to_string()))?;
    let mut staged = NamedTempFile::new_in(parent)
        .map_err(|e| DriveError::io("stage verified write", parent, e))?;
    std::io::Write::write_all(&mut staged, bytes)
        .and_then(|()| staged.as_file().sync_all())
        .map_err(|e| DriveError::io("stage verified write", path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o644))
            .map_err(|e| DriveError::io("stage verified write", path, e))?;
    }
    Ok(staged)
}

fn land(root: &DriveRoot, path: &Path, staged: NamedTempFile, bytes: &[u8]) -> DriveResult<()> {
    root.confine(path)?;
    staged
        .persist(path)
        .map_err(|e| DriveError::io("verified write", path, e.error))?;
    crate::fsutil::verify_contents(path, bytes)
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

pub fn move_file(root: &DriveRoot, from: &Path, to: &Path) -> DriveResult<()> {
    root.create_parent(to)?;
    root.confine(from)?;
    root.confine(to)?;
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
