//! [`NotificationFailureRepository`]: one row per withheld recipient or failed
//! pipeline step.
//!
//! The rows double as the retry ledger. A fact's attempts are its `process`
//! rows; a notification's delivery history is its `deliver:<surface>` rows
//! (a channel that failed permanently, never asked again) and its
//! `deliver_retry:<surface>` rows (one per transient failure of a pass).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use systemprompt_identifiers::{EventOutboxId, NotificationId, UserId};
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

/// One channel failure of a delivery pass, linked to its notification.
#[derive(Debug, Clone)]
pub struct DeliveryFailure {
    pub notification: NotificationId,
    pub recipient: UserId,
    pub stage: String,
    pub reason: String,
}

/// What earlier delivery passes left on one notification: the surfaces that
/// failed permanently and the most transient failures any surface has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveryHistory {
    pub permanent: HashSet<String>,
    pub attempts: i64,
}

pub const DELIVER_STAGE: &str = "deliver:";
pub const DELIVER_RETRY_STAGE: &str = "deliver_retry:";

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

    pub async fn record_deliveries(
        executor: impl PgExecutor<'_>,
        failures: &[DeliveryFailure],
    ) -> Result<(), RepositoryError> {
        if failures.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = failures
            .iter()
            .map(|f| f.notification.as_str().to_owned())
            .collect();
        let recipients: Vec<String> = failures
            .iter()
            .map(|f| f.recipient.as_str().to_owned())
            .collect();
        let stages: Vec<String> = failures.iter().map(|f| f.stage.clone()).collect();
        let reasons: Vec<String> = failures.iter().map(|f| f.reason.clone()).collect();
        sqlx::query!(
            "INSERT INTO notification_delivery_failures
                 (notification_id, recipient_user_id, stage, reason)
             SELECT f.id::uuid, f.recipient, f.stage, f.reason
             FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[])
                  AS f(id, recipient, stage, reason)",
            &ids,
            &recipients,
            &stages,
            &reasons,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn delivery_history(
        executor: impl PgExecutor<'_>,
        ids: &[NotificationId],
    ) -> Result<HashMap<NotificationId, DeliveryHistory>, RepositoryError> {
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT notification_id::text AS "id!: NotificationId", stage,
                      COUNT(*) AS "count!"
               FROM notification_delivery_failures
               WHERE notification_id = ANY($1::text[]::uuid[]) AND stage LIKE 'deliver%'
               GROUP BY notification_id, stage"#,
            &ids,
        )
        .fetch_all(executor)
        .await?;
        let mut out: HashMap<NotificationId, DeliveryHistory> = HashMap::new();
        for row in rows {
            let history = out.entry(row.id).or_default();
            if let Some(surface) = row.stage.strip_prefix(DELIVER_STAGE) {
                history.permanent.insert(surface.to_owned());
            } else if row.stage.starts_with(DELIVER_RETRY_STAGE) {
                history.attempts = history.attempts.max(row.count);
            }
        }
        Ok(out)
    }

    pub async fn count_for_outbox(
        executor: impl PgExecutor<'_>,
        outbox_id: &EventOutboxId,
        stage: &str,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM notification_delivery_failures
               WHERE outbox_id = $1 AND stage = $2"#,
            outbox_id.as_str(),
            stage,
        )
        .fetch_one(executor)
        .await?)
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
