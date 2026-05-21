//! Tarball extraction and remote-vs-local diffing.
//!
//! Extraction is hardened against path-traversal: symlinks, absolute
//! paths, `..` components, and entries outside the allowed top-level
//! directories are all rejected before anything touches disk.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use flate2::read::GzDecoder;
use tar::Archive;

use super::{INCLUDE_DIRS, collect_dir};
use crate::error::{SyncError, SyncResult};
use crate::files::{FileDiffStatus, SyncDiffEntry, SyncDiffResult};

pub fn extract_tarball(data: &[u8], target: &Path) -> SyncResult<usize> {
    extract_tarball_filtered(data, target, |_| true)
}

pub fn extract_tarball_selective(
    data: &[u8],
    target: &Path,
    paths_to_sync: &[String],
) -> SyncResult<usize> {
    let allowed: std::collections::HashSet<&str> =
        paths_to_sync.iter().map(String::as_str).collect();
    extract_tarball_filtered(data, target, |p| allowed.contains(p))
}

fn extract_tarball_filtered<F>(data: &[u8], target: &Path, accept: F) -> SyncResult<usize>
where
    F: Fn(&str) -> bool,
{
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut count = 0;

    let canonical_target = target.canonicalize()?;

    for entry in archive.entries()? {
        let mut entry = entry?;

        let entry_type = entry.header().entry_type();
        if !(entry_type.is_file() || entry_type.is_dir()) {
            return Err(SyncError::TarballUnsafe(format!(
                "disallowed entry type {:?} in tarball: {}",
                entry_type,
                entry.path()?.to_string_lossy()
            )));
        }

        let entry_path = entry.path()?.into_owned();
        let entry_path_str = entry_path.to_string_lossy();

        if entry_path.is_absolute()
            || entry_path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
        {
            return Err(SyncError::TarballUnsafe(format!(
                "invalid path in tarball: {entry_path_str}"
            )));
        }

        let first_component = entry_path
            .components()
            .find_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .unwrap_or("");
        if !INCLUDE_DIRS.contains(&first_component) {
            return Err(SyncError::TarballUnsafe(format!(
                "path not in allowed top-level directory: {entry_path_str}"
            )));
        }

        if !accept(&entry_path_str) {
            continue;
        }

        let dest_path = canonical_target.join(&entry_path);

        if !dest_path.starts_with(&canonical_target) {
            return Err(SyncError::TarballUnsafe(format!(
                "path escapes target directory: {entry_path_str}"
            )));
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest_path)?;
        count += 1;
    }

    Ok(count)
}

pub fn compare_tarball_with_local(data: &[u8], services_path: &Path) -> SyncResult<SyncDiffResult> {
    let temp_dir = tempfile::tempdir()?;
    extract_tarball(data, temp_dir.path())?;

    let mut remote_files: HashMap<String, (String, u64)> = HashMap::new();
    for dir in INCLUDE_DIRS {
        let dir_path = temp_dir.path().join(dir);
        if dir_path.exists() {
            let mut entries = vec![];
            collect_dir(&dir_path, temp_dir.path(), &mut entries)?;
            for entry in entries {
                remote_files.insert(entry.path, (entry.checksum, entry.size));
            }
        }
    }

    let mut local_files: HashMap<String, String> = HashMap::new();
    for dir in INCLUDE_DIRS {
        let dir_path = services_path.join(dir);
        if dir_path.exists() {
            let mut entries = vec![];
            collect_dir(&dir_path, services_path, &mut entries)?;
            for entry in entries {
                local_files.insert(entry.path, entry.checksum);
            }
        }
    }

    let mut entries = Vec::new();
    let mut added = 0;
    let mut modified = 0;
    let mut unchanged = 0;

    for (path, (remote_checksum, size)) in &remote_files {
        match local_files.get(path) {
            Some(local_checksum) if local_checksum == remote_checksum => {
                unchanged += 1;
                entries.push(SyncDiffEntry {
                    path: path.clone(),
                    status: FileDiffStatus::Unchanged,
                    size: *size,
                });
            },
            Some(_) => {
                modified += 1;
                entries.push(SyncDiffEntry {
                    path: path.clone(),
                    status: FileDiffStatus::Modified,
                    size: *size,
                });
            },
            None => {
                added += 1;
                entries.push(SyncDiffEntry {
                    path: path.clone(),
                    status: FileDiffStatus::Added,
                    size: *size,
                });
            },
        }
    }

    let mut deleted = 0;
    for path in local_files.keys() {
        if !remote_files.contains_key(path) {
            deleted += 1;
            entries.push(SyncDiffEntry {
                path: path.clone(),
                status: FileDiffStatus::Deleted,
                size: 0,
            });
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(SyncDiffResult {
        entries,
        added,
        modified,
        deleted,
        unchanged,
    })
}
