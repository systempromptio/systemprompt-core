//! Storage-id validation and naming shared by every backend.
//!
//! An id is a path relative to the backend's root (a directory for
//! [`crate::LocalFileStorage`], a bucket prefix for [`crate::GcsFileStorage`]).
//! Absolute paths and `..` components are refused before any I/O.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path, PathBuf};

use systemprompt_traits::{FileStorageError, StorageValidation, StoredFileId};

pub(crate) fn relative_path(path: &Path) -> Result<&Path, FileStorageError> {
    if path.as_os_str().is_empty() {
        return Err(FileStorageError::Validation(StorageValidation::EmptyPath));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {},
            Component::ParentDir => {
                return Err(FileStorageError::Validation(
                    StorageValidation::ParentDirectory {
                        path: path.to_path_buf(),
                    },
                ));
            },
            Component::RootDir | Component::Prefix(_) => {
                return Err(FileStorageError::Validation(
                    StorageValidation::NotRelative {
                        path: path.to_path_buf(),
                    },
                ));
            },
        }
    }
    Ok(path)
}

pub(crate) fn id_for(path: &Path) -> StoredFileId {
    let normalised: PathBuf = path
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect();
    StoredFileId::new(normalised.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("txt" | "md") => "text/plain",
        Some("csv") => "text/csv",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
}
