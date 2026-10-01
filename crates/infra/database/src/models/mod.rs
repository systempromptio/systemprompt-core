//! Data models exchanged across the database boundary.
//!
//! [`DbValue`] and [`JsonRow`] are re-exported from `systemprompt-traits`
//! because they are part of the [`crate::DatabaseProvider`] signatures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod info;
pub mod query;
pub mod transaction;

pub use info::{ColumnInfo, DatabaseInfo, IndexInfo, TableInfo};
pub use query::{DatabaseQuery, QueryResult, QueryRow, QuerySelector};
pub use systemprompt_traits::{DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime};
pub use transaction::DatabaseTransaction;
