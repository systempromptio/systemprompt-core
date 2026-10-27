//! The Drive folder and the paths inside it.
//!
//! A path the gateway names is untrusted: it is accepted only as a relative,
//! `/`-separated path of plain segments, so a change feed entry can never
//! address a file outside the folder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use super::error::{DriveError, DriveResult};
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
        let rel = checked_relative(rel)?;
        Ok(self.dir.join(rel))
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
        out.push(segment);
    }
    if out.starts_with(TRASH_DIR) || out.as_os_str() == DRIVE_STATE_FILE {
        return Err(unsafe_path());
    }
    Ok(out)
}
