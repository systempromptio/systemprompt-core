//! User-record persistence, split by concern.
//!
//! The `UserRepository` methods are implemented across the submodules here:
//! lookups (`find`), pagination and search (`list`), mutations
//! (`operations`), bulk mutations over an id set (`bulk`), session tracking
//! (`session`), sharing subject lookups (`directory`), aggregate counts
//! (`stats`), account consolidation
//! (`merge`), archive and restore (`archive`), and the per-table purge a
//! deletion runs (`purge`, planned in `purge_plan`), service-account rows
//! (`service_account`), and the rows the identity config plane manages
//! (`declared`). Re-exports the result and parameter types [`MergeResult`],
//! [`PurgeCount`], [`PurgeOutcome`], [`SkippedPurge`] and [`UpdateUserParams`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod archive;
mod bulk;
mod declared;
mod directory;
mod find;
pub(crate) use find::active_user_row;
mod list;
mod merge;
pub(super) mod operations;
mod purge;
mod purge_plan;
mod service_account;
mod session;
mod stats;
mod updates;

pub use archive::{ArchiveOutcome, ArchiveParams, ArchiveState};
pub use declared::{DeclaredUserRow, DeclaredUserWrite};
pub use merge::{MERGE_EXCLUDED_SECURITY_TABLES, MergeResult};
pub use operations::UpdateUserParams;
pub use purge::{PurgeCount, PurgeOutcome, SkippedPurge};
pub use service_account::{EnsuredServiceAccount, NewServiceAccount};
