//! The passes that run before the retention windows: log rows whose owning
//! user is gone, then every registered `RetentionSweep` (an owner's own
//! orphan settlement, such as the AI crate failing requests left `pending`
//! by a lost settlement).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_logging::LoggingRepository;
use systemprompt_traits::{JobContext, ProviderError, ProviderResult, registered_retention_sweeps};
use systemprompt_users::UserRepository;
use tracing::{debug, info};

use crate::services::EnabledExtensions;

pub(super) async fn delete_orphaned_logs(db_pool: &DbPool, enforce: bool) -> ProviderResult<u64> {
    let logs = LoggingRepository::new(db_pool);
    let users = UserRepository::new(db_pool);
    let internal =
        |e: systemprompt_logging::models::LoggingError| ProviderError::Internal(Box::new(e));
    // Why: `logs` and `users` have different owners, so the orphan set is
    // computed by asking each: the log owners seen, minus the users that
    // still exist. System-span rows are owned by a sentinel no user row
    // backs, and are never orphans.
    let seen: Vec<_> = logs
        .distinct_log_user_ids()
        .await
        .map_err(internal)?
        .into_iter()
        .filter(|id| id.as_str() != systemprompt_logging::SYSTEM_LOG_OWNER)
        .collect();
    let orphans = users
        .missing_ids(&seen)
        .await
        .map_err(|e| ProviderError::Internal(Box::new(e)))?;
    if enforce {
        logs.delete_logs_for_users(&orphans).await.map_err(internal)
    } else {
        let would = logs
            .count_logs_for_users(&orphans)
            .await
            .map_err(internal)?;
        info!(
            would_delete_orphaned_logs = would,
            "enforce disabled: orphaned log rows were not deleted"
        );
        Ok(0)
    }
}

pub(super) async fn run_sweeps(
    ctx: &JobContext,
    enabled: &EnabledExtensions,
) -> ProviderResult<()> {
    for registration in registered_retention_sweeps() {
        if !enabled.contains(registration.owner) {
            continue;
        }
        let sweep = registration.sweep;
        debug!(sweep = sweep.name(), "Retention sweep");
        sweep.sweep(ctx, ctx.enforce()).await?;
    }
    Ok(())
}
