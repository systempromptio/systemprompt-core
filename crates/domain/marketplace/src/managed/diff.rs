//! Exact file differences distinguish content, metadata and removals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FileEntry, RevisionManifest};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    pub before: Option<FileEntry>,
    pub after: Option<FileEntry>,
}

pub fn diff_files(before: &RevisionManifest, after: &RevisionManifest) -> Vec<FileChange> {
    let paths: BTreeSet<_> = before.files.keys().chain(after.files.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let old = before.files.get(path);
            let new = after.files.get(path);
            let kind = match (old, new) {
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Removed,
                (Some(old), Some(new))
                    if old.digest != new.digest
                        || old.executable != new.executable
                        || old.media_type != new.media_type
                        || old.bytes != new.bytes =>
                {
                    ChangeKind::Modified
                },
                _ => return None,
            };
            Some(FileChange {
                path: path.clone(),
                kind,
                before: old.cloned(),
                after: new.cloned(),
            })
        })
        .collect()
}
