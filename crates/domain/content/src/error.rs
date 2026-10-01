//! Typed error surface for the content crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum ContentError {
        common: [repository, io, json, yaml, validation],

        #[error("database must be PostgreSQL")]
        DatabaseNotPostgres,

        #[error("content not found: {0}")]
        ContentNotFound(String),

        #[error("link not found: {0}")]
        LinkNotFound(String),

        #[error("invalid request: {0}")]
        InvalidRequest(String),

        #[error("parse error: {0}")]
        Parse(String),

        #[error("invalid date '{value}'")]
        InvalidDate {
            value: String,
            #[source]
            source: chrono::ParseError,
        },

        #[error("service error: {0}")]
        Service(String),

        #[error("invalid identifier: {0}")]
        InvalidIdentifier(#[from] systemprompt_identifiers::error::IdValidationError),
    }
}

impl From<sqlx::Error> for ContentError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

pub type ContentResult<T> = Result<T, ContentError>;
