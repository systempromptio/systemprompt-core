//! The session identifiers as SQLite values.
//!
//! The identifier types live in `harness-ids` and are re-exported here
//! unchanged. Neither crate can implement `rusqlite`'s traits for the other's
//! types, so ids cross into SQL through `param` / `optional_param` and come
//! back out through `column` / `optional_column`, which parse and reject a
//! malformed value.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

pub use systemprompt_identifiers::engine::{CallId, IdError, MessageId, PartId, ProjectId, SessionId, now_ms};
use rusqlite::Row;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

#[derive(Debug, Clone, Copy)]
pub(crate) struct IdParam<'a>(&'a str);

impl ToSql for IdParam<'_> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(self.0.as_bytes())))
    }
}

#[derive(Debug)]
struct Stored<T>(T);

impl<T: FromStr<Err = IdError>> FromSql for Stored<T> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        T::from_str(text)
            .map(Self)
            .map_err(|error| FromSqlError::Other(Box::new(error)))
    }
}

pub(crate) fn param<I: AsRef<str>>(id: &I) -> IdParam<'_> {
    IdParam(id.as_ref())
}

pub(crate) fn optional_param<I: AsRef<str>>(id: Option<&I>) -> Option<IdParam<'_>> {
    id.map(param)
}

pub(crate) fn column<T: FromStr<Err = IdError>>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    row.get::<_, Stored<T>>(index).map(|stored| stored.0)
}

pub(crate) fn optional_column<T: FromStr<Err = IdError>>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    row.get::<_, Option<Stored<T>>>(index)
        .map(|stored| stored.map(|inner| inner.0))
}
