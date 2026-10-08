//! The canonical `RepositoryError` → [`ApiError`] mapping.
//!
//! A missing entity answers 404, a repository-raised conflict or a unique,
//! exclusion or foreign-key violation 409, an invalid argument or a not-null or
//! check violation 400, and everything else 500. Constraint names, SQL and
//! backend text stay in the logged source, never in the body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::RepositoryError;

use super::ApiError;

const CONFLICT_MESSAGE: &str = "The request conflicts with existing data";
const CONSTRAINT_MESSAGE: &str = "The request violates a data constraint";
const REPOSITORY_FAILURE: &str = "Repository operation failed";

impl From<RepositoryError> for ApiError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound { .. } => Self::not_found(err.to_string()),
            RepositoryError::Conflict { .. } => Self::conflict(err.to_string()),
            RepositoryError::InvalidArgument { .. } => Self::bad_request(err.to_string()),
            other => classify_failure(other),
        }
    }
}

fn classify_failure(err: RepositoryError) -> ApiError {
    let constraint = match &err {
        RepositoryError::Constraint { kind, .. } => Some(*kind),
        _ => None,
    };
    match constraint {
        Some(kind) if kind.is_conflict() => ApiError::conflict(CONFLICT_MESSAGE)
            .with_error_key(kind.code())
            .with_source(err),
        Some(kind) => ApiError::bad_request(CONSTRAINT_MESSAGE)
            .with_error_key(kind.code())
            .with_source(err),
        None => ApiError::internal(REPOSITORY_FAILURE, err),
    }
}
