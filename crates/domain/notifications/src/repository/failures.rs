//! [`NotificationFailureRepository`]: one row per withheld recipient or failed
//! pipeline step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use systemprompt_identifiers::{EventOutboxId, UserId};
use systemprompt_traits::RepositoryError;

use super::Pools;

/// A failure to record: the outbox fact it came from (if any), the recipient
/// it concerns (if any), the pipeline stage and why.
#[derive(Debug, Clone)]
pub struct NewFailure<'a> {
    pub outbox_id: Option<&'a EventOutboxId>,
    pub recipient: Option<&'a UserId>,
    pub stage: &'a str,
    pub reason: &'a str,
}

#[derive(Debug, Clone)]
pub struct NotificationFailureRepository {
    pools: Pools,
}

impl NotificationFailureRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        executor: impl PgExecutor<'_>,
        failure: NewFailure<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO notification_delivery_failures (outbox_id, recipient_user_id, stage, reason)
             VALUES ($1, $2, $3, $4)",
            failure.outbox_id.map(EventOutboxId::as_str),
            failure.recipient.map(UserId::as_str),
            failure.stage,
            failure.reason,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn record_detached(&self, failure: NewFailure<'_>) -> Result<(), RepositoryError> {
        Self::record(&*self.pools.write, failure).await
    }

    pub async fn record_many(
        executor: impl PgExecutor<'_>,
        outbox_id: Option<&EventOutboxId>,
        failures: &[(Option<UserId>, String, String)],
    ) -> Result<(), RepositoryError> {
        if failures.is_empty() {
            return Ok(());
        }
        let recipients: Vec<Option<String>> = failures
            .iter()
            .map(|(r, _, _)| r.as_ref().map(|u| u.as_str().to_owned()))
            .collect();
        let stages: Vec<String> = failures.iter().map(|(_, s, _)| s.clone()).collect();
        let reasons: Vec<String> = failures.iter().map(|(_, _, r)| r.clone()).collect();
        sqlx::query!(
            "INSERT INTO notification_delivery_failures (outbox_id, recipient_user_id, stage, reason)
             SELECT $1, f.recipient, f.stage, f.reason
             FROM UNNEST($2::text[], $3::text[], $4::text[]) AS f(recipient, stage, reason)",
            outbox_id.map(EventOutboxId::as_str),
            &recipients as _,
            &stages,
            &reasons,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn purge_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM notification_delivery_failures WHERE id IN (
                 SELECT id FROM notification_delivery_failures WHERE at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
