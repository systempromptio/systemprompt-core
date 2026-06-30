//! Cheap filesystem-state fingerprints used to key the catalogue and bundle
//! caches.
//!
//! [`hash_dir_metadata`] folds a directory tree into a hasher by relative path,
//! modification time, and size — without reading file contents. A content edit
//! changes a file's mtime and/or size, so the fingerprint shifts and the cache
//! rebuilds, but an unchanged tree costs only `readdir` + `stat` rather than
//! reading and parsing every file.

use std::path::Path;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

pub(crate) fn hash_dir_metadata(hasher: &mut Sha256, root: &Path) {
    let mut entries: Vec<(String, u64, u64)> = Vec::new();
    collect(root, root, &mut entries);
    entries.sort();
    for (rel, mtime, size) in entries {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(mtime.to_le_bytes());
        hasher.update(size.to_le_bytes());
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, u64, u64)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            collect(root, &path, out);
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| u64::try_from(d.as_nanos()).ok())
            .unwrap_or(0);
        out.push((rel, mtime, meta.len()));
    }
}
