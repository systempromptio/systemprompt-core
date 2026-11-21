//! Confinement of a path read beneath a root directory.
//!
//! [`confine_within`] resolves a candidate path and its root through the
//! filesystem (symbolic links and `..` included) and returns the resolved
//! path only when it lies inside the resolved root. [`single_segment`]
//! accepts a name only when it is one path segment, by the same rule the
//! typed identifiers of the services tree apply in `try_new`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
