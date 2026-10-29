//! [`NotificationReceiptRepository`]: per-surface delivery, seen, read and
//! escalation receipts. A receipt is unique per notification, surface and
//! state, so every writer is idempotent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use sqlx::PgExecutor;
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::ReceiptState;
use systemprompt_traits::RepositoryError;

use super::Pools;

#[derive(Debug, Clone)]
pub struct NotificationReceiptRepository {
    pools: Pools,
}

impl NotificationReceiptRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        executor: impl PgExecutor<'_>,
        ids: &[NotificationId],
        surface: &str,
        state: ReceiptState,
    ) -> Result<(), RepositoryError> {
        if ids.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        sqlx::query!(
            "INSERT INTO notification_receipts (notification_id, surface, state)
             SELECT t.id::uuid, $2, $3 FROM UNNEST($1::text[]) AS t(id)
             ON CONFLICT (notification_id, surface, state) DO NOTHING",
            &ids,
            surface,
            state.as_str(),
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn record_detached(
        &self,
        ids: &[NotificationId],
        surface: &str,
        state: ReceiptState,
    ) -> Result<(), RepositoryError> {
        Self::record(&*self.pools.write, ids, surface, state).await
    }

    pub async fn record_owned(
        &self,
        owner: &UserId,
        ids: &[NotificationId],
        surface: &str,
        state: ReceiptState,
    ) -> Result<usize, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let result = sqlx::query!(
            "INSERT INTO notification_receipts (notification_id, surface, state)
             SELECT n.id, $3, $4 FROM notifications n
             WHERE n.recipient_user_id = $1 AND n.id = ANY($2::text[]::uuid[])
             ON CONFLICT (notification_id, surface, state) DO NOTHING",
            owner.as_str(),
            &ids,
            surface,
            state.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(usize::try_from(result.rows_affected()).unwrap_or(usize::MAX))
    }

    pub async fn delivered_surfaces(
        &self,
        ids: &[NotificationId],
    ) -> Result<HashMap<NotificationId, HashSet<String>>, RepositoryError> {
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT notification_id::text AS "id!: NotificationId", surface
               FROM notification_receipts
               WHERE notification_id = ANY($1::text[]::uuid[]) AND state = $2"#,
            &ids,
            ReceiptState::Delivered.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?;
        let mut out: HashMap<NotificationId, HashSet<String>> = HashMap::new();
        for row in rows {
            out.entry(row.id).or_default().insert(row.surface);
        }
        Ok(out)
    }
}
