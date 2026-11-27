//! The snapshot store of one worktree: tracking, diffing and reading snapshots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

use super::changes::{Change, Side, entries, pair, working_entries};
use super::fsops::{Written, remove_file, write_file};
use super::layout::{FileEntry, Layout, MANIFEST_VERSION, Manifest, hash_bytes};
use super::tree::{Hashed, hash_tree};
use super::{DiffTarget, SnapshotError, SnapshotId, SnapshotInfo, SnapshotOptions, io_at};
use crate::diff::{FileDiff, file_diff};
use crate::paths::{fold, path_for_key, relative_key};

const STORE_KEY_CHARS: usize = 16;

/// The snapshot store of one worktree. Cheap to clone; operations on one store
/// are serialized.
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    pub(super) inner: Arc<StoreInner>,
}

#[derive(Debug)]
pub(super) struct StoreInner {
    pub(super) worktree: PathBuf,
    pub(super) layout: Layout,
    pub(super) options: SnapshotOptions,
    pub(super) lock: Mutex<()>,
}

impl SnapshotStore {
    pub fn open(
        worktree: impl Into<PathBuf>,
        data_dir: &Path,
        options: SnapshotOptions,
    ) -> Result<Self, SnapshotError> {
        let worktree = worktree.into();
        let identity = fs::canonicalize(&worktree).unwrap_or_else(|_| worktree.clone());
        let digest = hash_bytes(fold(&identity.to_string_lossy()).as_bytes());
        let short = digest.get(..STORE_KEY_CHARS).unwrap_or(digest.as_str());
        let layout = Layout::new(data_dir.join("snapshots").join(short));
        layout.ensure()?;
        Ok(Self {
            inner: Arc::new(StoreInner {
                worktree,
                layout,
                options,
                lock: Mutex::new(()),
            }),
        })
    }

    pub fn worktree(&self) -> &Path {
        &self.inner.worktree
    }

    pub fn store_dir(&self) -> &Path {
        &self.inner.layout.root
    }

    pub fn track(&self) -> Result<SnapshotId, SnapshotError> {
        let _guard = self.inner.lock.lock();
        let tree = self.hash_working(true)?;
        let manifest = Manifest {
            version: MANIFEST_VERSION,
            created_ms: now_ms(),
            files: tree
                .into_iter()
                .map(|(key, hashed)| (key, hashed.entry))
                .collect(),
        };
        let id = manifest.id();
        self.inner.layout.write_manifest(&id, &manifest)?;
        tracing::debug!(snapshot = %id, files = manifest.files.len(), "tracked worktree snapshot");
        Ok(id)
    }

    pub fn diff(&self, from: &SnapshotId, to: &DiffTarget) -> Result<Vec<FileDiff>, SnapshotError> {
        let _guard = self.inner.lock.lock();
        let old = self.inner.layout.read_manifest(from)?;
        match to {
            DiffTarget::Snapshot(id) => {
                let new = self.inner.layout.read_manifest(id)?;
                pair(entries(&old.files), entries(&new.files))
                    .iter()
                    .map(|change| self.stored_diff(change))
                    .collect()
            },
            DiffTarget::Working => {
                let working = self.hash_working(false)?;
                pair(entries(&old.files), working_entries(&working))
                    .iter()
                    .map(|change| self.working_diff(change, &working))
                    .collect()
            },
        }
    }

    pub fn changed_files(&self, from: &SnapshotId) -> Result<Vec<PathBuf>, SnapshotError> {
        let _guard = self.inner.lock.lock();
        let old = self.inner.layout.read_manifest(from)?;
        let working = self.hash_working(false)?;
        let changes = pair(entries(&old.files), working_entries(&working));
        Ok(changes
            .iter()
            .filter_map(|change| path_for_key(&self.inner.worktree, change.key()))
            .collect())
    }

    pub fn file_at(&self, id: &SnapshotId, path: &Path) -> Result<Option<Vec<u8>>, SnapshotError> {
        let manifest = self.inner.layout.read_manifest(id)?;
        let key = relative_key(&self.inner.worktree, path).ok_or_else(|| {
            SnapshotError::OutsideWorktree {
                path: path.to_path_buf(),
            }
        })?;
        let folded = fold(&key);
        let entry = manifest
            .files
            .iter()
            .find(|(candidate, _)| fold(candidate) == folded)
            .map(|(_, entry)| entry);
        entry.map_or_else(
            || Ok(None),
            |entry| self.inner.layout.read_object(&entry.hash).map(Some),
        )
    }

    pub fn exists(&self, id: &SnapshotId) -> bool {
        self.inner.layout.read_manifest(id).is_ok()
    }

    pub fn list(&self) -> Result<Vec<SnapshotInfo>, SnapshotError> {
        let mut infos = Vec::new();
        for id in self.inner.layout.manifest_ids()? {
            match self.inner.layout.read_manifest(&id) {
                Ok(manifest) => infos.push(SnapshotInfo {
                    created: UNIX_EPOCH + Duration::from_millis(manifest.created_ms),
                    files: manifest.files.len(),
                    id,
                }),
                Err(error) => {
                    tracing::warn!(snapshot = %id, %error, "skipping unreadable snapshot");
                },
            }
        }
        infos.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.id.cmp(&b.id)));
        Ok(infos)
    }

    pub(super) fn hash_working(
        &self,
        store_objects: bool,
    ) -> Result<BTreeMap<String, Hashed>, SnapshotError> {
        hash_tree(
            &self.inner.worktree,
            &self.inner.layout,
            &self.inner.options,
            store_objects,
        )
    }

    pub(super) fn path_of(&self, key: &str) -> Result<PathBuf, SnapshotError> {
        path_for_key(&self.inner.worktree, key).ok_or_else(|| SnapshotError::OutsideWorktree {
            path: PathBuf::from(key),
        })
    }

    pub(super) fn write(&self, target: &Path, entry: &FileEntry) -> Result<bool, SnapshotError> {
        let bytes = self.inner.layout.read_object(&entry.hash)?;
        match write_file(target, &bytes, entry.executable).map_err(io_at(target))? {
            Written::Yes => Ok(true),
            Written::SkippedSymlink => {
                tracing::warn!(path = %target.display(), "not restoring a file through a symlink");
                Ok(false)
            },
        }
    }

    pub(super) fn remove(&self, path: &Path) -> Result<bool, SnapshotError> {
        remove_file(&self.inner.worktree, path).map_err(io_at(path))
    }

    fn stored_diff(&self, change: &Change<'_>) -> Result<FileDiff, SnapshotError> {
        let old_bytes = self.object_for(change.old)?;
        let new_bytes = self.object_for(change.new)?;
        Ok(file_diff(
            change.key(),
            old_bytes.as_deref(),
            new_bytes.as_deref(),
        ))
    }

    fn working_diff(
        &self,
        change: &Change<'_>,
        working: &BTreeMap<String, Hashed>,
    ) -> Result<FileDiff, SnapshotError> {
        let old_bytes = self.object_for(change.old)?;
        let new_bytes = match change.new.and_then(|(key, _)| working.get(key)) {
            Some(hashed) => Some(fs::read(&hashed.path).map_err(io_at(&hashed.path))?),
            None => None,
        };
        Ok(file_diff(
            change.key(),
            old_bytes.as_deref(),
            new_bytes.as_deref(),
        ))
    }

    fn object_for(&self, side: Option<Side<'_>>) -> Result<Option<Vec<u8>>, SnapshotError> {
        side.map(|(_, entry)| self.inner.layout.read_object(&entry.hash))
            .transpose()
    }
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}
