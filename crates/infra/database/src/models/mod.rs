//! Data models exchanged across the database boundary.
//!
//! Typed identifiers, [`DbValue`] and [`JsonRow`] are re-exported from
//! `systemprompt-identifiers` and `systemprompt-traits` so that downstream
//! crates only depend on `systemprompt-database` for the database surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod info;
pub mod query;
pub mod transaction;

pub use info::{ColumnInfo, DatabaseInfo, IndexInfo, TableInfo};
pub use query::{DatabaseQuery, FromDatabaseRow, QueryResult, QueryRow, QuerySelector};
pub use systemprompt_identifiers::{
    ArtifactId, ClientId, ContentId, ContextId, ExecutionStepId, FileId, LogId, SessionId, SkillId,
    TaskId, TokenId, TraceId, UserId,
};
pub use systemprompt_traits::{DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime};
pub use transaction::DatabaseTransaction;
