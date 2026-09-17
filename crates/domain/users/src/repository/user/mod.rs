//! User-record persistence, split by concern.
//!
//! The `UserRepository` methods are implemented across the submodules here:
//! lookups (`find`), pagination and search (`list`), mutations
//! (`operations`), bulk mutations over an id set (`bulk`), session tracking
//! (`session`), aggregate counts (`stats`), account consolidation
//! (`merge`), and the per-table purge a deletion runs (`purge`). Re-exports
//! the result and parameter types [`MergeResult`], [`PurgeCount`] and
//! [`UpdateUserParams`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bulk;
mod find;
mod list;
mod merge;
pub(super) mod operations;
mod purge;
mod session;
mod stats;
mod updates;

pub use merge::{MERGE_EXCLUDED_SECURITY_TABLES, MergeResult};
pub use operations::UpdateUserParams;
pub use purge::PurgeCount;
