//! The Drive folder and the paths inside it.
//!
//! A path the instance names is untrusted: it is placed only as a relative,
//! `/`-separated path of portable segments ([`super::names`]), resolved with
//! [`systemprompt_device::secure::contained_path`] so no existing component below the folder is a
//! link or reparse point. A download sits between that check and the write, so
//! every write and move checks the whole path again ([`SyncRoot::confine`])
//! just before it lands.
//!
//! The folder is the caller's choice; [`default_drive_root`] names
//! `<Documents>/<product> Drive` and [`account_root`] one subfolder per
//! instance under it. A folder records the instance it mirrors in its index,
//! and a folder holding another instance's index is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use systemprompt_device::secure::{SecureError, contained_path};
use url::Url;

use super::error::SyncError;
use super::index::SyncIndex;
use super::names::{PathRefusal, portable_segment, safe_label};

pub(crate) const INDEX_FILE: &str = ".drive-state.json";
pub(crate) const TRASH_DIR: &str = ".trash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SyncRoot {
    dir: PathBuf,
}

impl SyncRoot {
    #[must_use]
    pub(crate) const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    #[must_use]
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub(crate) fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    #[must_use]
    pub(crate) fn trash_dir(&self) -> PathBuf {
        self.dir.join(TRASH_DIR)
    }

    pub(crate) fn file(&self, rel: &str) -> Result<PathBuf, SyncError> {
        let unsafe_path = || SyncError::UnsafePath {
            path: rel.to_owned(),
        };
        if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
            return Err(unsafe_path());
        }
        for segment in rel.split('/') {
            portable_segment(segment).map_err(|refusal| match refusal {
                PathRefusal::NotRelative => unsafe_path(),
                refusal => SyncError::Unplaceable {
                    path: rel.to_owned(),
                    refusal,
                },
            })?;
        }
        let first = rel.split('/').next().unwrap_or(rel);
        if first.eq_ignore_ascii_case(TRASH_DIR) || rel.eq_ignore_ascii_case(INDEX_FILE) {
            return Err(SyncError::Unplaceable {
                path: rel.to_owned(),
                refusal: PathRefusal::Hidden,
            });
        }
        contained_path(&self.dir, rel).map_err(|error| contained_error(rel, error))
    }

    pub(crate) fn trash_file(&self, stamp: &str, rel: &str) -> Result<PathBuf, SyncError> {
        let nested = format!("{stamp}/{rel}");
        contained_path(&self.trash_dir(), &nested).map_err(|error| contained_error(&nested, error))
    }

    pub(crate) fn confine(&self, path: &Path) -> Result<(), SyncError> {
        let unsafe_path = || SyncError::UnsafePath {
            path: path.display().to_string(),
        };
        let inside = path
            .strip_prefix(&self.dir)
            .map_err(|_outside| unsafe_path())?;
        let mut at = self.dir.clone();
        for component in inside.components() {
            let Component::Normal(part) = component else {
                return Err(unsafe_path());
            };
            at.push(part);
            match std::fs::symlink_metadata(&at) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(SyncError::Unplaceable {
                        path: self
                            .relative(path)
                            .unwrap_or_else(|| path.display().to_string()),
                        refusal: PathRefusal::Link,
                    });
                },
                Ok(_) => {},
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => return Err(SyncError::io("inspect", &at, error)),
            }
        }
        Ok(())
    }

    #[must_use]
    pub(crate) fn relative(&self, path: &Path) -> Option<String> {
        let inside = path.strip_prefix(&self.dir).ok()?;
        let mut parts = Vec::new();
        for component in inside.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
                _ => return None,
            }
        }
        (!parts.is_empty()).then(|| parts.join("/"))
    }
}

fn contained_error(rel: &str, error: SecureError) -> SyncError {
    match error {
        SecureError::Link { .. } => SyncError::Unplaceable {
            path: rel.to_owned(),
            refusal: PathRefusal::Link,
        },
        SecureError::Escapes { .. } => SyncError::UnsafePath {
            path: rel.to_owned(),
        },
        other => SyncError::Secure(other),
    }
}

#[must_use]
pub fn default_drive_root(documents: &Path, product: &str) -> PathBuf {
    documents.join(safe_label(&format!("{product} Drive"), "Drive"))
}

#[must_use]
pub fn account_root(base: &Path, instance: &Url) -> PathBuf {
    let host = instance.host_str().unwrap_or("instance");
    let label = instance
        .port()
        .map_or_else(|| host.to_owned(), |port| format!("{host}-{port}"));
    base.join(safe_label(&label, "instance"))
}

pub(crate) fn prepare_root(dir: &Path, instance: &str) -> Result<SyncRoot, SyncError> {
    let invalid = || SyncError::InvalidRoot {
        root: dir.to_path_buf(),
    };
    if !dir.is_absolute() {
        return Err(invalid());
    }
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return Err(invalid()),
        Ok(_) => {},
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(dir)
                .map_err(|error| SyncError::io("create the Drive folder", dir, error))?;
        },
        Err(error) => return Err(SyncError::io("inspect", dir, error)),
    }
    let root = SyncRoot::new(dir.to_path_buf());
    let path = root.index_path();
    let mut index = SyncIndex::load(&path)?;
    match index.instance.as_deref() {
        Some(known) if known != instance => {
            return Err(SyncError::RootTaken {
                root: dir.to_path_buf(),
            });
        },
        Some(_) => {},
        None => {
            index.instance = Some(instance.to_owned());
            index.save(&path)?;
        },
    }
    Ok(root)
}
