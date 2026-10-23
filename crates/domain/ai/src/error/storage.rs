//! [`StorageError`]: persisting and reading generated image files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use thiserror::Error;

use systemprompt_traits::{AiProviderError, FileStorageError, StoredFileId};

use crate::services::storage::StorageConfigError;

/// The file-store operation a [`StorageError::Operation`] failed in.
#[derive(Debug, Clone)]
pub enum StorageOperation {
    WriteImage(PathBuf),
    Delete(StoredFileId),
    Stat(StoredFileId),
}

impl std::fmt::Display for StorageOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WriteImage(path) => write!(f, "failed to write image file {}", path.display()),
            Self::Delete(id) => write!(f, "failed to delete file {id}"),
            Self::Stat(id) => write!(f, "failed to stat file {id}"),
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("{operation}: {source}")]
    Operation {
        operation: StorageOperation,
        #[source]
        source: FileStorageError,
    },

    #[error("invalid image storage configuration: {0}")]
    Config(#[source] StorageConfigError),

    #[error("image size {size} bytes exceeds the maximum allowed size {max} bytes")]
    ImageTooLarge { size: usize, max: usize },

    #[error("failed to decode base64 image: {0}")]
    ImageDecode(#[source] base64::DecodeError),

    #[error("invalid generated file id: {0}")]
    InvalidFileId(#[source] uuid::Error),

    #[error("file persistence failed: {0}")]
    FilePersistence(#[source] AiProviderError),
}
