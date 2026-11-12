//! [`FileDeleteService`]: remove a file's stored object and its row.
//!
//! The object goes first and the row second, so a failed removal leaves the
//! row behind as the handle for a retry. A row whose path is absolute names
//! a file ingested from disk that the [`FileStorage`] backend never held;
//! only its row is removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};
use thiserror::Error;

use crate::error::FilesError;
use crate::models::File;
use crate::repository::FileRepository;

/// What became of the stored object behind a deleted file row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredObjectOutcome {
    Removed,
    AlreadyAbsent,
    NotInStorage,
}

#[derive(Debug, Error)]
pub enum FileDeleteError {
    #[error("Storage error: {0}")]
    Storage(#[from] FileStorageError),

    #[error("Database error: {0}")]
    Database(#[source] FilesError),
}

#[derive(Clone)]
pub struct FileDeleteService {
    file_repository: FileRepository,
    storage: Arc<dyn FileStorage>,
}

impl std::fmt::Debug for FileDeleteService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileDeleteService").finish_non_exhaustive()
    }
}

impl FileDeleteService {
    pub const fn new(file_repository: FileRepository, storage: Arc<dyn FileStorage>) -> Self {
        Self {
            file_repository,
            storage,
        }
    }

    pub async fn delete_file(&self, file: &File) -> Result<StoredObjectOutcome, FileDeleteError> {
        let outcome = if Path::new(&file.path).is_absolute() {
            StoredObjectOutcome::NotInStorage
        } else {
            match self.storage.delete(&StoredFileId::new(&file.path)).await {
                Ok(()) => StoredObjectOutcome::Removed,
                Err(FileStorageError::NotFound(_)) => StoredObjectOutcome::AlreadyAbsent,
                Err(e) => return Err(e.into()),
            }
        };

        self.file_repository
            .delete(&file.id)
            .await
            .map_err(FileDeleteError::Database)?;

        Ok(outcome)
    }
}
