//! Typed error surface for the `systemprompt-files` crate.
//!
//! Boilerplate variants (`Repository`, `Io`, `Json`, `Yaml`, `Validation`,
//! `NotFound`, `Config`) are injected by [`systemprompt_models::domain_error`].
//! Database errors funnel through the workspace's single
//! [`systemprompt_traits::RepositoryError`] rather than `sqlx::Error`
//! directly so the layer boundary is preserved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_config::ProfileBootstrapError;
use systemprompt_identifiers::FileId;
use systemprompt_models::domain_error;

domain_error! {
    pub enum FilesError {
        common: [repository, io, json, yaml, validation, not_found, config],

        #[error("profile: {0}")]
        Profile(#[from] ProfileBootstrapError),

        #[error("Failed to read files.yaml ({path:?}): {source}")]
        ConfigRead {
            path: PathBuf,
            #[source]
            source: std::io::Error,
        },

        #[error("Failed to parse files.yaml ({path:?}): {source}")]
        ConfigParse {
            path: PathBuf,
            #[source]
            source: serde_yaml::Error,
        },

        #[error("invalid UUID for file id {id}")]
        InvalidFileId {
            id: FileId,
            #[source]
            source: uuid::Error,
        },
    }
}

impl From<sqlx::Error> for FilesError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

pub type FilesResult<T> = Result<T, FilesError>;

pub(crate) fn parse_file_uuid(id: &FileId) -> FilesResult<uuid::Uuid> {
    uuid::Uuid::parse_str(id.as_str()).map_err(|source| FilesError::InvalidFileId {
        id: id.clone(),
        source,
    })
}
