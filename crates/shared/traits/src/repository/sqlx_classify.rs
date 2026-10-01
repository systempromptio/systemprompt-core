//! SQLSTATE classification of `sqlx::Error` into [`RepositoryError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::error::ErrorKind;

use super::{ConstraintKind, RepositoryError};

const UNNAMED_CONSTRAINT: &str = "<unnamed>";

impl From<sqlx::Error> for RepositoryError {
    fn from(err: sqlx::Error) -> Self {
        if matches!(err, sqlx::Error::RowNotFound) {
            return Self::NotFound("row not found".to_owned());
        }
        let classified = err.as_database_error().map(|db_error| {
            let kind = match db_error.kind() {
                ErrorKind::UniqueViolation => Some(ConstraintKind::Unique),
                ErrorKind::ExclusionViolation => Some(ConstraintKind::Exclusion),
                ErrorKind::ForeignKeyViolation => Some(ConstraintKind::ForeignKey),
                ErrorKind::NotNullViolation => Some(ConstraintKind::NotNull),
                ErrorKind::CheckViolation => Some(ConstraintKind::Check),
                _ => None,
            };
            let constraint = db_error
                .constraint()
                .unwrap_or(UNNAMED_CONSTRAINT)
                .to_owned();
            let sqlstate = db_error.code().map(std::borrow::Cow::into_owned);
            (kind, constraint, sqlstate)
        });
        match classified {
            Some((Some(kind), constraint, _)) => Self::Constraint {
                kind,
                constraint,
                source: Box::new(err),
            },
            Some((None, _, sqlstate)) => Self::Database {
                sqlstate,
                source: Box::new(err),
            },
            None => Self::Database {
                sqlstate: None,
                source: Box::new(err),
            },
        }
    }
}
