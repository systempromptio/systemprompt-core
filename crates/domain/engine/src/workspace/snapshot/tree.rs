//! Walking a worktree the way snapshots see it: honouring ignore files,
//! skipping symlinks, VCS metadata and the store itself, and hashing contents
//! with an mtime+size cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, io};

use ignore::{DirEntry, WalkBuilder};

use super::layout::{CachedStat, FileEntry, Layout, StatCache, hash_bytes};
use super::{SnapshotError, SnapshotOptions};
use crate::workspace::paths::rel_key_under;

const VCS_DIRS: &[&str] = &[".git", ".hg", ".svn", ".jj"];
const RACY_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) key: String,
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) mtime: Option<(i64, u32)>,
    pub(crate) executable: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Hashed {
    pub(crate) path: PathBuf,
    pub(crate) entry: FileEntry,
}

struct TreeHasher<'a> {
    layout: &'a Layout,
    options: &'a SnapshotOptions,
    previous: StatCache,
    store_objects: bool,
}

pub(crate) fn scan(
    root: &Path,
    store_dir: &Path,
    options: &SnapshotOptions,
) -> Result<Vec<Candidate>, SnapshotError> {
    if !root.is_dir() {
        return Err(SnapshotError::Io {
            path: root.to_path_buf(),
            source: io::Error::new(io::ErrorKind::NotFound, "worktree is not a directory"),
        });
    }
    let mut out = Vec::new();
    for result in walker(root, store_dir).build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                tracing::debug!(%error, "skipping unreadable path while scanning worktree");
                continue;
            },
        };
        let Some(candidate) = candidate(root, &entry, options) else {
            continue;
        };
        if out.len() >= options.max_files {
            return Err(SnapshotError::TooManyFiles {
                limit: options.max_files,
            });
        }
        out.push(candidate);
    }
    Ok(out)
}

fn walker(root: &Path, store_dir: &Path) -> WalkBuilder {
    let store_dir = store_dir.to_path_buf();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .parents(true)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            let name = entry.file_name();
            let is_vcs = VCS_DIRS.iter().any(|dir| name.eq_ignore_ascii_case(dir));
            !is_vcs && !entry.path().starts_with(&store_dir)
        });
    builder
}

fn candidate(root: &Path, entry: &DirEntry, options: &SnapshotOptions) -> Option<Candidate> {
    if !entry.file_type().is_some_and(|t| t.is_file()) {
        return None;
    }
    let key = rel_key_under(root, entry.path())?;
    let metadata = match entry.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::debug!(
                path = %entry.path().display(),
                %error,
                "skipping file without metadata"
            );
            return None;
        },
    };
    if metadata.len() > options.max_file_size {
        return None;
    }
    Some(Candidate {
        key,
        path: entry.path().to_path_buf(),
        size: metadata.len(),
        mtime: metadata.modified().ok().map(time_parts),
        executable: is_executable(&metadata),
    })
}

pub(crate) fn hash_tree(
    root: &Path,
    layout: &Layout,
    options: &SnapshotOptions,
    store_objects: bool,
) -> Result<BTreeMap<String, Hashed>, SnapshotError> {
    let started = SystemTime::now();
    let racy_after = time_parts(started.checked_sub(RACY_WINDOW).unwrap_or(UNIX_EPOCH));
    let candidates = scan(root, &layout.root, options)?;
    let hasher = TreeHasher {
        layout,
        options,
        previous: layout.load_cache(),
        store_objects,
    };
    let mut fresh = StatCache::default();
    let mut out = BTreeMap::new();
    for candidate in candidates {
        let Some((hash, size)) = hasher.content_hash(&candidate)? else {
            continue;
        };
        if let Some(stat) = settled_stat(&candidate, &hash, size, racy_after) {
            fresh.files.insert(candidate.key.clone(), stat);
        }
        let entry = FileEntry {
            hash,
            size,
            executable: candidate.executable,
        };
        out.insert(
            candidate.key,
            Hashed {
                path: candidate.path,
                entry,
            },
        );
    }
    if let Err(error) = layout.save_cache(&fresh) {
        tracing::warn!(%error, "could not save snapshot stat cache");
    }
    Ok(out)
}

impl TreeHasher<'_> {
    fn content_hash(&self, candidate: &Candidate) -> Result<Option<(String, u64)>, SnapshotError> {
        if let Some(hash) = self.cached_hash(candidate) {
            return Ok(Some((hash, candidate.size)));
        }
        let bytes = match fs::read(&candidate.path) {
            Ok(bytes) => bytes,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
            {
                tracing::debug!(
                    path = %candidate.path.display(),
                    %error,
                    "file vanished or is unreadable"
                );
                return Ok(None);
            },
            Err(source) => {
                return Err(SnapshotError::Io {
                    path: candidate.path.clone(),
                    source,
                });
            },
        };
        let size = bytes.len() as u64;
        if size > self.options.max_file_size {
            return Ok(None);
        }
        let hash = hash_bytes(&bytes);
        if self.store_objects {
            self.layout.write_object(&hash, &bytes)?;
        }
        Ok(Some((hash, size)))
    }

    fn cached_hash(&self, candidate: &Candidate) -> Option<String> {
        let mtime = candidate.mtime?;
        let stat = self.previous.files.get(&candidate.key)?;
        let unchanged = stat.size == candidate.size && (stat.mtime_s, stat.mtime_ns) == mtime;
        let usable = unchanged && (!self.store_objects || self.layout.has_object(&stat.hash));
        usable.then(|| stat.hash.clone())
    }
}

fn settled_stat(
    candidate: &Candidate,
    hash: &str,
    size: u64,
    racy_after: (i64, u32),
) -> Option<CachedStat> {
    let (secs, nanos) = candidate.mtime?;
    let settled = (secs, nanos) < racy_after && size == candidate.size;
    settled.then(|| CachedStat {
        size,
        mtime_s: secs,
        mtime_ns: nanos,
        hash: hash.to_owned(),
    })
}

pub(crate) fn time_parts(time: SystemTime) -> (i64, u32) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => (
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            since.subsec_nanos(),
        ),
        Err(before) => {
            let before = before.duration();
            (
                i64::try_from(before.as_secs()).map_or(i64::MIN, |secs| -secs),
                before.subsec_nanos(),
            )
        },
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
const fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}
