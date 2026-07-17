//! Admin-only database surface.
//!
//! Every module here is part of the documented sqlx allowlist: the SQL is
//! either user-supplied or built dynamically against runtime-supplied table
//! names, so compile-time verification is impossible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin_sql;
mod identifier;
mod introspection;
mod query_executor;

pub use admin_sql::{AdminSql, AdminSqlError, DEFAULT_READONLY_ROW_LIMIT};
pub use identifier::{IdentifierError, SafeIdentifier};
pub use introspection::DatabaseAdminService;
pub use query_executor::{QueryExecutor, QueryExecutorError};
