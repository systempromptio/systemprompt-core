//! A cheap fingerprint of a services tree on disk.
//!
//! [`tree_fingerprint`] hashes every regular file under a root by relative
//! path, size and modification time, following symlinks so a Kubernetes
//! `ConfigMap` mount (whose files are links through a swapped `..data`
//! directory) changes fingerprint when the map is updated. The value is only
//! compared within one process, so the standard hasher is enough. Unreadable
//! entries hash as their path alone, and directory links are followed only to
//! a bounded depth, so a link cycle cannot loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;

const MAX_DEPTH: usize = 16;

#[must_use]
pub fn tree_fingerprint(root: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    walk(root, root, 0, &mut hasher);
    hasher.finish()
}

fn walk(root: &Path, dir: &Path, depth: usize, hasher: &mut DefaultHasher) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let relative = path.strip_prefix(root).unwrap_or(&path);
        relative.hash(hasher);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            walk(root, &path, depth + 1, hasher);
            continue;
        }
        meta.len().hash(hasher);
        if let Some(nanos) = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        {
            nanos.as_nanos().hash(hasher);
        }
    }
}
