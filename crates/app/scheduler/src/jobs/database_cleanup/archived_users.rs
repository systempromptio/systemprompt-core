//! The archived-user pass: accounts archived longer ago than
//! `retention.archived_users_days` and not under legal hold are purged —
//! the `users` row and every row keyed on it — through the users crate's
//! guarded purge. Without `enforce` the pass only counts them.
//!
//! A purge that left a member's rows behind (a recorded table missing, or a
//! table owner with no recorded declarations) is counted as incomplete; the
//! job reports the count and does not succeed while it is non-zero.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::AdmittedPurge;
use systemprompt_traits::{ProviderError, ProviderResult};
use systemprompt_users::{UserRepository, UserService};
use tracing::{info, warn};

const ARCHIVE_PURGE_BATCH: i64 = 500;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ArchivePurge {
    pub(super) purged: u64,
    pub(super) incomplete: u64,
}

pub(super) async fn purge_archived_users(
    db_pool: &DbPool,
    plan: &AdmittedPurge,
    window_days: u32,
    enforce: bool,
) -> ProviderResult<ArchivePurge> {
    let users = UserService::new(Arc::new(UserRepository::new(db_pool)));
    let internal = |e: systemprompt_users::UserError| ProviderError::Internal(Box::new(e));
    if !enforce {
        let would = users
            .list_purgeable_archives(window_days, ARCHIVE_PURGE_BATCH)
            .await
            .map_err(internal)?;
        info!(
            would_purge_archived_users = would.len(),
            archived_users_days = window_days,
            "enforce disabled: archived users past the window were not purged"
        );
        return Ok(ArchivePurge::default());
    }
    let purged = users
        .purge_expired_archives(window_days, ARCHIVE_PURGE_BATCH, plan)
        .await
        .map_err(internal)?;
    let mut incomplete = 0_u64;
    for archive in purged.iter().filter(|archive| !archive.outcome.is_complete()) {
        incomplete += 1;
        warn!(
            user_id = %archive.id,
            skipped = archive.outcome.skipped.len(),
            unrecorded_members = archive.outcome.unrecorded_members.len(),
            "archived user purge left member rows behind"
        );
    }
    Ok(ArchivePurge {
        purged: u64::try_from(purged.len()).unwrap_or(u64::MAX),
        incomplete,
    })
}
