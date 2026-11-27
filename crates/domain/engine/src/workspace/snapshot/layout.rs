//! On-disk layout of one worktree's store: content objects, snapshot manifests
//! and the stat cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{SnapshotError, SnapshotId, io_at};

pub(crate) const MANIFEST_VERSION: u32 = 1;
const TEMP_NAME_ATTEMPTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileEntry {
    pub(crate) hash: String,
    pub(crate) size: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) version: u32,
    pub(crate) created_ms: u64,
    pub(crate) files: BTreeMap<String, FileEntry>,
}

impl Manifest {
    pub(crate) fn id(&self) -> SnapshotId {
        let mut hasher = Sha256::new();
        for (key, entry) in &self.files {
            hasher.update(key.as_bytes());
            hasher.update([0u8]);
            hasher.update(entry.hash.as_bytes());
            hasher.update([0u8]);
            hasher.update(entry.size.to_le_bytes());
            hasher.update([u8::from(entry.executable), b'\n']);
        }
        SnapshotId::from_digest(to_hex(hasher.finalize().as_slice()))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StatCache {
    pub(crate) files: HashMap<String, CachedStat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CachedStat {
    pub(crate) size: u64,
    pub(crate) mtime_s: i64,
    pub(crate) mtime_ns: u32,
    pub(crate) hash: String,
}

#[derive(Debug)]
pub(crate) struct Layout {
    pub(crate) root: PathBuf,
    temp_counter: AtomicU64,
}

impl Layout {
    pub(crate) const fn new(root: PathBuf) -> Self {
        Self {
            root,
            temp_counter: AtomicU64::new(0),
        }
    }

    pub(super) fn objects(&self) -> PathBuf {
        self.root.join("objects")
    }

    fn manifests(&self) -> PathBuf {
        self.root.join("manifests")
    }

    fn cache_file(&self) -> PathBuf {
        self.root.join("stat-cache.json")
    }

    pub(crate) fn ensure(&self) -> Result<(), SnapshotError> {
        for dir in [self.objects(), self.manifests()] {
            fs::create_dir_all(&dir).map_err(io_at(&dir))?;
        }
        Ok(())
    }

    fn manifest_path(&self, id: &SnapshotId) -> PathBuf {
        self.manifests().join(format!("{}.json", id.as_str()))
    }

    pub(crate) fn write_manifest(
        &self,
        id: &SnapshotId,
        manifest: &Manifest,
    ) -> Result<(), SnapshotError> {
        let path = self.manifest_path(id);
        let bytes = serde_json::to_vec(manifest).map_err(|source| SnapshotError::Manifest {
            path: path.clone(),
            source,
        })?;
        self.write_atomic(&self.manifests(), &path, &bytes)
    }

    pub(crate) fn read_manifest(&self, id: &SnapshotId) -> Result<Manifest, SnapshotError> {
        let path = self.manifest_path(id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(SnapshotError::NotFound(id.clone()));
            },
            Err(source) => return Err(SnapshotError::Io { path, source }),
        };
        serde_json::from_slice(&bytes).map_err(|source| SnapshotError::Manifest { path, source })
    }

    pub(crate) fn remove_manifest(&self, id: &SnapshotId) -> Result<(), SnapshotError> {
        let path = self.manifest_path(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(SnapshotError::Io { path, source }),
        }
    }

    pub(crate) fn manifest_ids(&self) -> Result<Vec<SnapshotId>, SnapshotError> {
        let dir = self.manifests();
        let mut ids = Vec::new();
        for entry in fs::read_dir(&dir).map_err(io_at(&dir))? {
            let entry = entry.map_err(io_at(&dir))?;
            let name = entry.file_name();
            let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            if let Ok(id) = stem.parse::<SnapshotId>() {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    pub(crate) fn load_cache(&self) -> StatCache {
        let path = self.cache_file();
        fs::read(&path).map_or_else(
            |_| StatCache::default(),
            |bytes| {
                serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                    tracing::warn!(
                        path = %path.display(),
                        %error,
                        "discarding unreadable snapshot stat cache"
                    );
                    StatCache::default()
                })
            },
        )
    }

    pub(crate) fn save_cache(&self, cache: &StatCache) -> Result<(), SnapshotError> {
        let path = self.cache_file();
        let bytes = serde_json::to_vec(cache).map_err(|source| SnapshotError::Manifest {
            path: path.clone(),
            source,
        })?;
        self.write_atomic(&self.root, &path, &bytes)
    }

    pub(super) fn write_atomic(
        &self,
        dir: &Path,
        dest: &Path,
        bytes: &[u8],
    ) -> Result<(), SnapshotError> {
        let temp = self.write_temp(dir, bytes)?;
        if let Err(source) = fs::rename(&temp, dest) {
            remove_quietly(&temp);
            return Err(SnapshotError::Io {
                path: dest.to_path_buf(),
                source,
            });
        }
        Ok(())
    }

    fn write_temp(&self, dir: &Path, bytes: &[u8]) -> Result<PathBuf, SnapshotError> {
        for _ in 0..TEMP_NAME_ATTEMPTS {
            let serial = self.temp_counter.fetch_add(1, Ordering::Relaxed);
            let temp = dir.join(format!("tmp-{}-{serial}", std::process::id()));
            let mut file = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
            {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(SnapshotError::Io { path: temp, source }),
            };
            if let Err(source) = file.write_all(bytes) {
                drop(file);
                remove_quietly(&temp);
                return Err(SnapshotError::Io { path: temp, source });
            }
            return Ok(temp);
        }
        Err(SnapshotError::Io {
            path: dir.to_path_buf(),
            source: io::Error::from(io::ErrorKind::AlreadyExists),
        })
    }
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    to_hex(Sha256::digest(bytes).as_slice())
}

pub(crate) fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

pub(super) fn remove_quietly(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        tracing::debug!(path = %path.display(), %error, "could not remove temporary file");
    }
}
