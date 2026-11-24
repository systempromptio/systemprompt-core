//! Confinement of a path read beneath a root directory.
//!
//! [`confine_within`] resolves a candidate path and its root through the
//! filesystem (symbolic links and `..` included) and returns the resolved
//! path only when it lies inside the resolved root. [`single_segment`]
//! accepts a name only when it is one path segment, by the same rule the
//! typed identifiers of the services tree apply in `try_new`.
//!
//! Every other read beneath a root goes through the same rule:
//! [`locate_within`] confines a path that may be absent, [`read_within`]
//! reads a confined file, [`open_within`] returns a confined file already
//! open (the handle is checked to be the confined file after the open), and
//! [`entries_within`] enumerates a directory.
//! An enumeration refuses, with [`PathConfinementError::Escapes`] (or
//! [`PathConfinementError::Unresolved`] for a dangling link), when any entry
//! resolves outside the root: an escaping link is never followed and never
//! silently skipped. [`logs`] reads log files beneath a logs root by the
//! same rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod logs;

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use systemprompt_identifiers::error::IdValidationError;

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
    #[error("{kind} is not a single path segment")]
    NotASegment {
        kind: &'static str,
        #[source]
        source: IdValidationError,
    },
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

pub fn open_within(
    root: &Path,
    candidate: &Path,
) -> Result<(PathBuf, std::fs::File), PathConfinementError> {
    let path = confine_within(root, candidate)?;
    let file = std::fs::File::open(&path).map_err(|source| PathConfinementError::Read {
        path: path.clone(),
        source,
    })?;
    let reconfined = confine_within(root, candidate)?;
    if reconfined != path || !same_file(&file, &path)? {
        return Err(PathConfinementError::Escapes {
            path: candidate.to_path_buf(),
            root: root.to_path_buf(),
        });
    }
    Ok((path, file))
}

#[cfg(unix)]
fn same_file(file: &std::fs::File, path: &Path) -> Result<bool, PathConfinementError> {
    use std::os::unix::fs::MetadataExt;
    let read = |source| PathConfinementError::Read {
        path: path.to_path_buf(),
        source,
    };
    let held = file.metadata().map_err(read)?;
    let named = path.symlink_metadata().map_err(read)?;
    Ok(held.dev() == named.dev() && held.ino() == named.ino())
}

#[cfg(not(unix))]
fn same_file(file: &std::fs::File, path: &Path) -> Result<bool, PathConfinementError> {
    let read = |source| PathConfinementError::Read {
        path: path.to_path_buf(),
        source,
    };
    let held = file.metadata().map_err(read)?;
    let named = path.symlink_metadata().map_err(read)?;
    Ok(held.len() == named.len() && !named.file_type().is_symlink())
}

pub fn read_within(root: &Path, candidate: &Path) -> Result<String, PathConfinementError> {
    let path = confine_within(root, candidate)?;
    std::fs::read_to_string(&path).map_err(|source| PathConfinementError::Read { path, source })
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
    systemprompt_identifiers::macros::validate_path_segment(kind, value)
        .map_err(|source| PathConfinementError::NotASegment { kind, source })?;
    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(value),
        _ => Err(PathConfinementError::NotASegment {
            kind,
            source: IdValidationError::invalid(kind, "must be one plain path component"),
        }),
    }
}
