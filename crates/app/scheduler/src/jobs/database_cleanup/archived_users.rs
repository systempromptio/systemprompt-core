//! The archived-user pass: accounts archived longer ago than
//! `retention.archived_users_days` and not under legal hold are purged —
//! the `users` row and every row keyed on it — through the users crate's
//! guarded purge. Without `enforce` the pass only counts them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_traits::{ProviderError, ProviderResult};
use systemprompt_users::{UserRepository, UserService};
use tracing::info;

const ARCHIVE_PURGE_BATCH: i64 = 500;

pub(super) async fn purge_archived_users(
    db_pool: &DbPool,
    window_days: u32,
    enforce: bool,
) -> ProviderResult<u64> {
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
        return Ok(0);
    }
    let purged = users
        .purge_expired_archives(window_days, ARCHIVE_PURGE_BATCH)
        .await
        .map_err(internal)?;
    Ok(u64::try_from(purged.len()).unwrap_or(u64::MAX))
}
