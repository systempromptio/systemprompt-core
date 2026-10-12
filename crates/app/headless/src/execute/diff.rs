//! Summarises the workspace diff a run produced (`headless_runs.diff_summary`).
//!
//! The executor snapshots every file's SHA-256 under the host's working
//! directory before the spawn and after the exit; [`workspace_diff`] compares
//! the two into a typed [`DiffSummary`]. `.git` and `node_modules` are not
//! walked.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const SKIPPED_DIRS: &[&str] = &[".git", "node_modules"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    pub kind: DiffKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub entries: Vec<DiffEntry>,
}

/// Relative path to content hash of every regular file under a directory.
pub type Snapshot = BTreeMap<String, String>;

#[must_use]
pub fn snapshot(dir: &Path) -> Snapshot {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !(entry.file_type().is_dir()
                && SKIPPED_DIRS.contains(&entry.file_name().to_string_lossy().as_ref()))
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let relative = entry.path().strip_prefix(dir).ok()?;
            let hash = hash_file(entry.path())?;
            Some((relative.to_string_lossy().replace('\\', "/"), hash))
        })
        .collect()
}

pub async fn snapshot_async(dir: PathBuf) -> Snapshot {
    match tokio::task::spawn_blocking(move || snapshot(&dir)).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(%error, "headless workspace snapshot failed");
            Snapshot::new()
        },
    }
}

fn hash_file(path: &Path) -> Option<String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            tracing::debug!(path = %path.display(), %error, "headless diff skips unreadable file");
            return None;
        },
    };
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = match file.read(&mut buf) {
            Ok(read) => read,
            Err(error) => {
                tracing::debug!(path = %path.display(), %error, "headless diff read failed");
                return None;
            },
        };
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Some(hex::encode(hasher.finalize()))
}

#[must_use]
pub fn workspace_diff(before: &Snapshot, after: &Snapshot) -> DiffSummary {
    let mut diff = DiffSummary::default();
    for (path, hash) in after {
        let kind = match before.get(path) {
            None => DiffKind::Added,
            Some(prev) if prev != hash => DiffKind::Modified,
            Some(_) => continue,
        };
        diff.push(path, kind);
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            diff.push(path, DiffKind::Deleted);
        }
    }
    diff.entries.sort_by(|a, b| a.path.cmp(&b.path));
    diff
}

impl DiffSummary {
    fn push(&mut self, path: &str, kind: DiffKind) {
        match kind {
            DiffKind::Added => self.added += 1,
            DiffKind::Modified => self.modified += 1,
            DiffKind::Deleted => self.deleted += 1,
        }
        self.entries.push(DiffEntry {
            path: path.to_owned(),
            kind,
        });
    }
}
