//! The change context every config plane writer sets before it writes, and
//! the cluster-wide config version a watcher polls.
//!
//! [`with_change_context`] sets `systemprompt.actor`, `systemprompt.reason`
//! and `systemprompt.source` transaction-locally (`set_config(.., true)`), so
//! they vanish at commit or rollback and can never leak to the next user of a
//! pooled connection. The plane tables' `sp_stamp_change()` trigger copies
//! them into `changed_by`, `reason` and `source`; a write made without them is
//! recorded as unattributed. Call it on the transaction, never on a bare pool
//! connection: outside a transaction the settings last one statement.
//!
//! [`PgConfigVersion`] reads the one `config_version` row that
//! `sp_bump_config_version()` bumps on every committed plane write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::{PgConnection, PgPool};
use systemprompt_traits::{ChangeActor, ChangeSource, ConfigVersionSource, RepositoryError};

use crate::DbPool;

pub async fn with_change_context(
    conn: &mut PgConnection,
    actor: &ChangeActor,
    reason: Option<&str>,
    source: &ChangeSource,
) -> Result<(), RepositoryError> {
    let source = source.to_string();
    sqlx::query!(
        "SELECT set_config('systemprompt.actor', $1, true) AS actor, \
         set_config('systemprompt.reason', $2, true) AS reason, \
         set_config('systemprompt.source', $3, true) AS source",
        actor.label(),
        reason.unwrap_or_default(),
        source,
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PgConfigVersion {
    pool: Arc<PgPool>,
}

impl PgConfigVersion {
    #[must_use]
    pub fn new(db: &DbPool) -> Self {
        Self {
            pool: db.write_pool(),
        }
    }
}

#[async_trait]
impl ConfigVersionSource for PgConfigVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        let version = sqlx::query_scalar!("SELECT version FROM config_version WHERE id = 1")
            .fetch_optional(&*self.pool)
            .await?;
        Ok(version.unwrap_or(0))
    }
}
