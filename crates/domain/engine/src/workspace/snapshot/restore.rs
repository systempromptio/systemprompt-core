//! Rolling a worktree, or chosen paths in it, back to a snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use super::layout::{FileEntry, hash_bytes};
use super::tree::{Candidate, Hashed, scan};
use super::{SnapshotError, SnapshotId, SnapshotStore};
use crate::workspace::paths::{fold, key_matches, relative_key};

impl SnapshotStore {
    pub fn restore(&self, id: &SnapshotId) -> Result<Vec<PathBuf>, SnapshotError> {
        let _guard = self.inner.lock.lock();
        let manifest = self.inner.layout.read_manifest(id)?;
        let working = self.hash_working(false)?;
        let wanted: HashMap<String, (&str, &FileEntry)> = manifest
            .files
            .iter()
            .map(|(key, entry)| (fold(key), (key.as_str(), entry)))
            .collect();
        let present: HashMap<String, &Hashed> = working
            .iter()
            .map(|(key, hashed)| (fold(key), hashed))
            .collect();

        let mut touched = Vec::new();
        for (folded, hashed) in &present {
            if !wanted.contains_key(folded) && self.remove(&hashed.path)? {
                touched.push(hashed.path.clone());
            }
        }
        for (folded, (key, entry)) in &wanted {
            let current = present.get(folded);
            let up_to_date = current.is_some_and(|hashed| {
                hashed.entry.hash == entry.hash && hashed.entry.executable == entry.executable
            });
            if up_to_date {
                continue;
            }
            let target = match current {
                Some(hashed) => hashed.path.clone(),
                None => self.path_of(key)?,
            };
            if self.write(&target, entry)? {
                touched.push(target);
            }
        }
        touched.sort();
        tracing::info!(snapshot = %id, files = touched.len(), "restored worktree from snapshot");
        Ok(touched)
    }

    pub fn revert_files(
        &self,
        id: &SnapshotId,
        paths: &[PathBuf],
    ) -> Result<Vec<PathBuf>, SnapshotError> {
        let _guard = self.inner.lock.lock();
        let manifest = self.inner.layout.read_manifest(id)?;
        let scanned = scan(
            &self.inner.worktree,
            &self.inner.layout.root,
            &self.inner.options,
        )?;
        let requested = self.requested_keys(paths)?;
        let selected = |key: &str| requested.iter().any(|wanted| key_matches(key, wanted));

        let wanted: BTreeMap<String, (&str, &FileEntry)> = manifest
            .files
            .iter()
            .filter(|(key, _)| selected(key))
            .map(|(key, entry)| (fold(key), (key.as_str(), entry)))
            .collect();
        let mut touched = Vec::new();
        for candidate in scanned.iter().filter(|candidate| selected(&candidate.key)) {
            let folded = fold(&candidate.key);
            if !wanted.contains_key(&folded)
                && fs::File::open(&candidate.path).is_ok()
                && self.remove(&candidate.path)?
            {
                touched.push(candidate.path.clone());
            }
        }
        touched.extend(self.rewrite_wanted(&wanted, &scanned)?);
        touched.sort();
        Ok(touched)
    }

    fn requested_keys(&self, paths: &[PathBuf]) -> Result<Vec<String>, SnapshotError> {
        paths
            .iter()
            .map(|path| {
                relative_key(&self.inner.worktree, path)
                    .ok_or_else(|| SnapshotError::OutsideWorktree { path: path.clone() })
            })
            .collect()
    }

    fn rewrite_wanted(
        &self,
        wanted: &BTreeMap<String, (&str, &FileEntry)>,
        scanned: &[Candidate],
    ) -> Result<Vec<PathBuf>, SnapshotError> {
        let on_disk: HashMap<String, &Path> = scanned
            .iter()
            .map(|candidate| (fold(&candidate.key), candidate.path.as_path()))
            .collect();
        let mut touched = Vec::new();
        for (folded, (key, entry)) in wanted {
            let target = match on_disk.get(folded) {
                Some(path) => path.to_path_buf(),
                None => self.path_of(key)?,
            };
            let unchanged = fs::read(&target).is_ok_and(|bytes| hash_bytes(&bytes) == entry.hash);
            if unchanged {
                continue;
            }
            if self.write(&target, entry)? {
                touched.push(target);
            }
        }
        Ok(touched)
    }
}
