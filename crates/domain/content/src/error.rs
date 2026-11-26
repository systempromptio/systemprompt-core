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

        #[error("parse error: {0}")]
        Parse(#[from] ContentParseError),

        #[error("invalid date '{value}'")]
        InvalidDate {
            value: String,
            #[source]
            source: chrono::ParseError,
        },

        #[error("invalid identifier: {0}")]
        InvalidIdentifier(#[from] systemprompt_identifiers::error::IdValidationError),

        #[error("invalid file id '{id}'")]
        InvalidFileId {
            id: systemprompt_identifiers::FileId,
            #[source]
            source: sqlx::types::uuid::Error,
        },

        #[error("the linked files could not be read")]
        FileSource(#[source] systemprompt_extension::content_files::ContentFileLinksError),

        #[error("file {file} is not linked to content {content}")]
        FileNotLinked {
            file: systemprompt_identifiers::FileId,
            content: systemprompt_identifiers::ContentId,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContentParseError {
    #[error("Failed to create datetime")]
    InvalidDatetime,
    #[error("Ambiguous timezone conversion")]
    AmbiguousTimezone,
    #[error("Invalid frontmatter format - missing '---' delimiters")]
    MissingFrontmatterDelimiters,
}

impl From<sqlx::Error> for ContentError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

pub type ContentResult<T> = Result<T, ContentError>;
