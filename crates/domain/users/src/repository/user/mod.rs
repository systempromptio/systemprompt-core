//! User-record persistence, split by concern.
//!
//! The `UserRepository` methods are implemented across the submodules here:
//! lookups (`find`), pagination and search (`list`), mutations
//! (`operations`), session tracking (`session`), aggregate counts (`stats`),
//! and account consolidation (`merge`). Re-exports the result and parameter
//! types [`MergeResult`] and [`UpdateUserParams`].

mod find;
mod list;
mod merge;
pub(super) mod operations;
mod session;
mod stats;

pub use merge::MergeResult;
pub use operations::UpdateUserParams;
