//! Typed error surface for the `systemprompt-files` crate.
//!
//! Boilerplate variants (`Repository`, `Io`, `Json`, `Yaml`, `Validation`,
//! `NotFound`, `Config`) are injected by [`systemprompt_models::domain_error`].
//! Database errors funnel through the canonical
//! [`systemprompt_database::RepositoryError`] rather than `sqlx::Error`
//! directly so the layer boundary is preserved.

use systemprompt_models::domain_error;

domain_error! {
    pub enum FilesError {
        common: [repository, io, json, yaml, validation, not_found, config],

        #[error("storage error: {0}")]
        Storage(String),
    }
}

impl From<sqlx::Error> for FilesError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_database::RepositoryError::from(err))
    }
}

pub type FilesResult<T> = Result<T, FilesError>;
