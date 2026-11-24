//! [`UsersRetention`], the users member's [`UserRetention`]: the log owners
//! with no `users` row, and the archived-user pass — accounts archived longer
//! ago than `retention.archived_users_days` and not under legal hold are
//! purged, the `users` row and every row keyed on it, through the guarded
//! purge over the admitted purge plan. Without `enforce` the pass only counts
//! them.
//!
//! A purge that left a member's rows behind (a recorded table missing, or a
//! table owner with no recorded declarations) is counted as incomplete.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::AdmittedPurge;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{
    ArchivedUserPurge, JobContext, ProviderError, ProviderResult, UserRetention,
};
use tracing::{info, warn};

use super::UserService;
use crate::UserError;
use crate::repository::UserRepository;

const ARCHIVE_PURGE_BATCH: i64 = 500;

#[derive(Debug, Clone)]
pub(crate) struct UsersRetention {
    users: UserService,
}

impl UsersRetention {
    #[must_use]
    pub(crate) fn new(repository: Arc<UserRepository>) -> Self {
        Self {
            users: UserService::new(repository),
        }
    }
}

fn internal(e: UserError) -> ProviderError {
    ProviderError::Internal(Box::new(e))
}

#[async_trait]
impl UserRetention for UsersRetention {
    async fn missing_user_ids(&self, candidates: &[UserId]) -> ProviderResult<Vec<UserId>> {
        self.users
            .repository
            .missing_ids(candidates)
            .await
            .map_err(internal)
    }

    async fn purge_archived(
        &self,
        ctx: &JobContext,
        window_days: u32,
        enforce: bool,
    ) -> ProviderResult<ArchivedUserPurge> {
        if !enforce {
            let would = self
                .users
                .list_purgeable_archives(window_days, ARCHIVE_PURGE_BATCH)
                .await
                .map_err(internal)?;
            info!(
                would_purge_archived_users = would.len(),
                archived_users_days = window_days,
                "enforce disabled: archived users past the window were not purged"
            );
            return Ok(ArchivedUserPurge::default());
        }
        let plan = ctx.get::<AdmittedPurge>()?;
        let purged = self
            .users
            .purge_expired_archives(window_days, ARCHIVE_PURGE_BATCH, plan)
            .await
            .map_err(internal)?;
        let mut incomplete = 0u64;
        for archive in purged
            .iter()
            .filter(|archive| !archive.outcome.is_complete())
        {
            incomplete += 1;
            warn!(
                user_id = %archive.id,
                skipped = archive.outcome.skipped.len(),
                unrecorded_members = archive.outcome.unrecorded_members.len(),
                "archived user purge left member rows behind"
            );
        }
        Ok(ArchivedUserPurge {
            purged: u64::try_from(purged.len()).unwrap_or(u64::MAX),
            incomplete,
        })
    }
}
