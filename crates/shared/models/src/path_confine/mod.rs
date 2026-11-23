//! Confinement of a path read beneath a root directory.
//!
//! [`confine_within`] resolves a candidate path and its root through the
//! filesystem (symbolic links and `..` included) and returns the resolved
//! path only when it lies inside the resolved root. [`single_segment`]
//! accepts a name only when it is one plain path segment: not empty, not `.`
//! or `..`, and free of separators and NUL.
//!
//! Every other read beneath a root goes through the same rule:
//! [`locate_within`] confines a path that may be absent and
//! [`entries_within`] enumerates a directory. An enumeration refuses, with
//! [`PathConfinementError::Escapes`] (or [`PathConfinementError::Unresolved`]
//! for a dangling link), when any entry resolves outside the root: an escaping
//! link is never followed and never silently skipped. [`logs`] reads log
//! files beneath a logs root by the same rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod logs;

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Why a path was not accepted beneath its root.
#[derive(Debug, thiserror::Error)]
pub enum PathConfinementError {
    #[error("root {} cannot be resolved", root.display())]
    RootUnresolved {
        root: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path {} cannot be resolved", path.display())]
    Unresolved {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path {} resolves outside {}", path.display(), root.display())]
    Escapes { path: PathBuf, root: PathBuf },
    #[error("{} cannot be read", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{kind} '{value}' is not a single path segment")]
    NotASegment { kind: &'static str, value: String },
}

pub fn confine_within(root: &Path, candidate: &Path) -> Result<PathBuf, PathConfinementError> {
    let canonical_root =
        root.canonicalize()
            .map_err(|source| PathConfinementError::RootUnresolved {
                root: root.to_path_buf(),
                source,
            })?;
    let canonical =
        candidate
            .canonicalize()
            .map_err(|source| PathConfinementError::Unresolved {
                path: candidate.to_path_buf(),
                source,
            })?;
    if canonical.starts_with(&canonical_root) {
        Ok(canonical)
    } else {
        Err(PathConfinementError::Escapes {
            path: candidate.to_path_buf(),
            root: root.to_path_buf(),
        })
    }
}

pub fn locate_within(
    root: &Path,
    candidate: &Path,
) -> Result<Option<PathBuf>, PathConfinementError> {
    match candidate.symlink_metadata() {
        Ok(_) => confine_within(root, candidate).map(Some),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(PathConfinementError::Unresolved {
            path: candidate.to_path_buf(),
            source,
        }),
    }
}

/// One entry of a directory enumerated beneath a root, resolved inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfinedEntry {
    pub name: OsString,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub fn entries_within(root: &Path, dir: &Path) -> Result<Vec<ConfinedEntry>, PathConfinementError> {
    let resolved = confine_within(root, dir)?;
    let read = std::fs::read_dir(&resolved).map_err(|source| PathConfinementError::Read {
        path: resolved.clone(),
        source,
    })?;
    let mut entries = Vec::new();
    for entry in read {
        let entry = entry.map_err(|source| PathConfinementError::Read {
            path: resolved.clone(),
            source,
        })?;
        let path = confine_within(root, &entry.path())?;
        entries.push(ConfinedEntry {
            name: entry.file_name(),
            is_dir: path.is_dir(),
            path,
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

pub fn single_segment<'a>(
    kind: &'static str,
    value: &'a str,
) -> Result<&'a str, PathConfinementError> {
    let not_a_segment = || PathConfinementError::NotASegment {
        kind,
        value: value.to_owned(),
    };
    if value.contains(['/', '\\', '\0']) {
        return Err(not_a_segment());
    }
    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(value),
        _ => Err(not_a_segment()),
    }
}
