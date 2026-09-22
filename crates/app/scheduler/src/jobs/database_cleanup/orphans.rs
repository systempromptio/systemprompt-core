//! The two orphan passes that run before the retention windows: log rows whose
//! owning user is gone, and AI requests left `pending` by a lost settlement.
//!
//! `fail_orphaned_pending` already existed but ran only from
//! `journal::recover()`, which fires at boot, so a long-lived server never
//! closed a `pending` row whose settlement was lost — three such rows had been
//! open for six days on the 2026-09-22 customer instance. This job is its
//! second, regular caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_ai::repository::ai_requests::ORPHAN_AGE;
use systemprompt_database::DbPool;
use systemprompt_logging::LoggingRepository;
use systemprompt_traits::{ProviderError, ProviderResult};
use systemprompt_users::UserRepository;
use tracing::{info, warn};

pub(super) async fn delete_orphaned_logs(db_pool: &DbPool, enforce: bool) -> ProviderResult<u64> {
    let logs =
        LoggingRepository::new(db_pool).map_err(|e| ProviderError::Configuration(e.to_string()))?;
    let users =
        UserRepository::new(db_pool).map_err(|e| ProviderError::Configuration(e.to_string()))?;
    let internal =
        |e: systemprompt_logging::models::LoggingError| ProviderError::Internal(e.to_string());
    // Why: `logs` and `users` have different owners, so the orphan set is
    // computed by asking each: the log owners seen, minus the users that
    // still exist.
    let seen = logs.distinct_log_user_ids().await.map_err(internal)?;
    let orphans = users
        .missing_ids(&seen)
        .await
        .map_err(|e| ProviderError::Internal(e.to_string()))?;
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

pub(super) async fn fail_orphaned_requests(db_pool: &DbPool, enforce: bool) -> ProviderResult<()> {
    if !enforce {
        info!("enforce disabled: orphaned pending AI requests were not failed");
        return Ok(());
    }
    let requests = AiRequestRepository::new(db_pool)
        .map_err(|e| ProviderError::Configuration(e.to_string()))?;
    let orphans = requests
        .fail_orphaned_pending(ORPHAN_AGE)
        .await
        .map_err(|e| ProviderError::Internal(e.to_string()))?;
    for orphan in &orphans {
        warn!(
            ai_request_id = %orphan.id,
            user_id = %orphan.owner,
            "AI request outlived every receipt; failed with unknown usage"
        );
    }
    Ok(())
}
