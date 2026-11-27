//! Mapping between table rows and the typed model. Every function takes a plain
//! connection (or a transaction through deref) so store methods can compose
//! them inside one transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod message;
mod part;
mod project;
mod session;

pub(crate) use message::{load_message, query_messages, write_message};
pub(crate) use part::{load_part, query_parts, query_todos, write_part};
pub(crate) use project::{insert_project, load_project, query_projects};
pub(crate) use session::{insert_session, load_session, query_sessions, update_session_row};

pub(crate) fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(crate) fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}
