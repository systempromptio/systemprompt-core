//! The Drive folder and the paths inside it.
//!
//! A path the gateway names is untrusted: it is accepted only as a relative,
//! `/`-separated path of plain segments, so a change feed entry can never
//! address a file outside the folder. A well-formed path is placed only when
//! its segments are portable ([`super::names`]) and no existing component
//! below the folder is a link, so a write can never follow one out of it.
//!
//! A path checked by [`DriveRoot::file`] can still have a component replaced
//! by a link before the write lands (a download sits between the two), so
//! every write creates its missing folders one at a time without following
//! links ([`DriveRoot::create_parent`]) and re-checks the whole path
//! ([`DriveRoot::confine`]) after staging, immediately before the rename.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use super::error::{DriveError, DriveResult};
use super::names::{PathRefusal, portable_segment};
use crate::config::paths::{DRIVE_STATE_FILE, drive_dir};

pub const TRASH_DIR: &str = ".trash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveRoot {
    dir: PathBuf,
}

impl DriveRoot {
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    #[must_use]
    pub fn resolve() -> Option<Self> {
        drive_dir().map(Self::new)
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.dir.join(DRIVE_STATE_FILE)
    }

    #[must_use]
    pub fn trash_dir(&self) -> PathBuf {
        self.dir.join(TRASH_DIR)
    }

    pub fn file(&self, rel: &str) -> DriveResult<PathBuf> {
        let checked = checked_relative(rel)?;
        let path = self.dir.join(checked);
        self.confine(&path)?;
        Ok(path)
    }

    pub fn confine(&self, path: &Path) -> DriveResult<()> {
        let mut at = self.dir.clone();
        for segment in self.segments(path)? {
            at.push(segment);
            match std::fs::symlink_metadata(&at) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(self.refused(path, PathRefusal::Symlink));
                },
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(DriveError::io("inspect drive path", &at, e)),
            }
        }
        Ok(())
    }

    pub fn create_parent(&self, path: &Path) -> DriveResult<()> {
        let mut segments = self.segments(path)?;
        segments.pop();
        let mut at = self.dir.clone();
        for segment in segments {
            at.push(segment);
            if let Err(e) = std::fs::create_dir(&at)
                && e.kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(DriveError::io("create folder", &at, e));
            }
            let meta = std::fs::symlink_metadata(&at)
                .map_err(|e| DriveError::io("inspect drive path", &at, e))?;
            if meta.file_type().is_symlink() {
                return Err(self.refused(path, PathRefusal::Symlink));
            }
            if !meta.is_dir() {
                return Err(DriveError::io(
                    "create folder",
                    &at,
                    std::io::ErrorKind::NotADirectory.into(),
                ));
            }
        }
        Ok(())
    }

    fn segments<'p>(&self, path: &'p Path) -> DriveResult<Vec<&'p std::ffi::OsStr>> {
        let unsafe_path = || DriveError::UnsafePath(path.display().to_string());
        let Ok(inside) = path.strip_prefix(&self.dir) else {
            return Err(unsafe_path());
        };
        inside
            .components()
            .map(|component| match component {
                Component::Normal(part) => Ok(part),
                _ => Err(unsafe_path()),
            })
            .collect()
    }

    fn refused(&self, path: &Path, refusal: PathRefusal) -> DriveError {
        let rel = self
            .relative(path)
            .unwrap_or_else(|| path.display().to_string());
        unplaceable(&rel, refusal)
    }

    #[must_use]
    pub fn relative(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.dir).ok()?;
        let mut parts = Vec::new();
        for component in rel.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
                _ => return None,
            }
        }
        (!parts.is_empty()).then(|| parts.join("/"))
    }
}

fn checked_relative(rel: &str) -> DriveResult<PathBuf> {
    let unsafe_path = || DriveError::UnsafePath(rel.to_owned());
    if rel.is_empty() || rel.contains('\\') || rel.starts_with('/') {
        return Err(unsafe_path());
    }
    let mut out = PathBuf::new();
    for segment in rel.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains(':') {
            return Err(unsafe_path());
        }
        portable_segment(segment).map_err(|refusal| unplaceable(rel, refusal))?;
        out.push(segment);
    }
    if out.starts_with(TRASH_DIR) || out.as_os_str() == DRIVE_STATE_FILE {
        return Err(unsafe_path());
    }
    Ok(out)
}

fn unplaceable(rel: &str, refusal: PathRefusal) -> DriveError {
    DriveError::Unplaceable {
        path: rel.to_owned(),
        refusal,
    }
}
