//! Content objects: one file per body, named by its SHA-256 and verified on
//! read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::PathBuf;
use std::{fs, io};

use super::layout::{Layout, hash_bytes, is_hash, remove_quietly};
use super::{SnapshotError, io_at};

impl Layout {
    fn object_path(&self, hash: &str) -> Result<PathBuf, SnapshotError> {
        if !is_hash(hash) {
            return Err(SnapshotError::CorruptObject {
                hash: hash.to_owned(),
            });
        }
        let (prefix, rest) = hash.split_at(2);
        Ok(self.objects().join(prefix).join(rest))
    }

    pub(crate) fn has_object(&self, hash: &str) -> bool {
        self.object_path(hash).is_ok_and(|path| path.is_file())
    }

    pub(crate) fn write_object(&self, hash: &str, bytes: &[u8]) -> Result<(), SnapshotError> {
        let path = self.object_path(hash)?;
        if path.is_file() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_at(parent))?;
        }
        self.write_atomic(&self.objects(), &path, bytes)
    }

    pub(crate) fn read_object(&self, hash: &str) -> Result<Vec<u8>, SnapshotError> {
        let path = self.object_path(hash)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(SnapshotError::MissingObject {
                    hash: hash.to_owned(),
                });
            },
            Err(source) => return Err(SnapshotError::Io { path, source }),
        };
        if hash_bytes(&bytes) != hash {
            return Err(SnapshotError::CorruptObject {
                hash: hash.to_owned(),
            });
        }
        Ok(bytes)
    }

    pub(crate) fn collect_garbage(
        &self,
        keep: &HashSet<String>,
    ) -> Result<(usize, u64), SnapshotError> {
        let objects = self.objects();
        let mut removed = 0usize;
        let mut freed = 0u64;
        for entry in fs::read_dir(&objects).map_err(io_at(&objects))? {
            let entry = entry.map_err(io_at(&objects))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if !is_dir {
                if name.starts_with("tmp-") {
                    remove_quietly(&path);
                }
                continue;
            }
            for object in fs::read_dir(&path).map_err(io_at(&path))? {
                let object = object.map_err(io_at(&path))?;
                let hash = format!("{name}{}", object.file_name().to_string_lossy());
                if keep.contains(&hash) {
                    continue;
                }
                let object_path = object.path();
                let size = object.metadata().map_or(0, |m| m.len());
                match fs::remove_file(&object_path) {
                    Ok(()) => {
                        removed += 1;
                        freed += size;
                    },
                    Err(source) => {
                        return Err(SnapshotError::Io {
                            path: object_path,
                            source,
                        });
                    },
                }
            }
            if fs::remove_dir(&path).is_err() {
                tracing::trace!(path = %path.display(), "object prefix directory still in use");
            }
        }
        Ok((removed, freed))
    }
}
