//! Result alias for the database crate's public signatures.
//!
//! The crate returns the workspace's single repository error,
//! [`systemprompt_traits::RepositoryError`], from every database-facing
//! signature, including the dyn-safe `DatabaseProvider` /
//! `DatabaseTransaction` surfaces. `sqlx::Error` converts into it through the
//! SQLSTATE classification enabled by the traits crate's `sqlx` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) use systemprompt_traits::RepositoryError;

pub type DatabaseResult<T> = Result<T, RepositoryError>;

pub(crate) fn is_undefined_table(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "42P01")
}
