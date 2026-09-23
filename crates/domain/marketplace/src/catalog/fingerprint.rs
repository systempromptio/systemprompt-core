//! Cheap filesystem-state fingerprints used to key the catalogue and bundle
//! caches.
//!
//! [`hash_dir_metadata`] folds a directory tree into a hasher by relative path,
//! modification time, and size — without reading file contents. A content edit
//! changes a file's mtime and/or size, so the fingerprint shifts and the cache
//! rebuilds, but an unchanged tree costs only `readdir` + `stat` rather than
//! reading and parsing every file.
//!
//! [`canonical_json`] serializes a value with every object's keys sorted.
//! Configuration types hold `HashMap`s, and `serde_json` preserves insertion
//! order, so two loads of the same files would otherwise serialize — and
//! fingerprint — differently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;
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

pub(crate) fn canonical_json<T: Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&sorted_keys(serde_json::to_value(value)?))
}

// JSON: canonical form of an arbitrary serialized config, sorted for hashing.
fn sorted_keys(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            serde_json::Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, sorted_keys(value)))
                    .collect(),
            )
        },
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sorted_keys).collect())
        },
        other => other,
    }
}
