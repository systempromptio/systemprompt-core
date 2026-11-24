//! The users member's answers to the kernel's `database_cleanup` job: which
//! log owners no longer exist, and the purge of archived accounts past the
//! profile window.
//!
//! [`DynUserRetention`] is provided **sealed** by the users member (it purges
//! users), so only kernel code reads it. With no provider the job skips the
//! orphaned-log pass and the archived-user pass and deletes nothing for
//! either.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_provider_contracts::{JobContext, ProviderResult};

/// The users-owned half of the retention job. `#[async_trait]` keeps the
/// trait `dyn`-compatible.
#[async_trait]
pub trait UserRetention: Send + Sync {
    async fn missing_user_ids(&self, candidates: &[UserId]) -> ProviderResult<Vec<UserId>>;

    async fn purge_archived(
        &self,
        ctx: &JobContext,
        window_days: u32,
        enforce: bool,
    ) -> ProviderResult<ArchivedUserPurge>;
}

pub type DynUserRetention = Arc<dyn UserRetention>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArchivedUserPurge {
    pub purged: u64,
    pub incomplete: u64,
}
