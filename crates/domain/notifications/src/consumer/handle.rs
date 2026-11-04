//! One fact of a drain round, on the round's transaction: decode it, run
//! the pipeline inside its own savepoint, and record what a failure leaves.
//!
//! A permanent failure is recorded and reported done, so the round
//! acknowledges it. A transient failure has already rolled back its
//! savepoint; it is reported for retry unless it is the fact's
//! [`MAX_FACT_ATTEMPTS`]th failure, which is recorded as `fact_abandoned`
//! and reported done.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Acquire, PgConnection};
use systemprompt_events::services::durable::ReportingFact;
use systemprompt_identifiers::{EventOutboxId, NotificationId};
use systemprompt_models::notifications::{NOTIFICATION_EVENT_VERSION, NotificationEvent};

use super::NotificationsConsumer;
use crate::error::{NotificationsError, NotificationsResult};
use crate::pipeline::{BatchContext, metrics};
use crate::repository::{FailureStage, NewFailure, NotificationFailureRepository};

pub const MAX_FACT_ATTEMPTS: i64 = 5;

pub(super) enum Handled {
    Done(Vec<NotificationId>),
    Retry(NotificationsError),
}

// JSON: the fact envelope is decoded first, and its data only after its
// version is checked.
fn decode(raw: &serde_json::Value) -> Result<NotificationEvent, DecodeFailure> {
    let fact: ReportingFact<serde_json::Value> =
        serde_json::from_value(raw.clone()).map_err(DecodeFailure::Decode)?;
    if fact.version != NOTIFICATION_EVENT_VERSION {
        return Err(DecodeFailure::UnsupportedVersion(fact.version));
    }
    serde_json::from_value(fact.data).map_err(DecodeFailure::Decode)
}

#[derive(Debug, thiserror::Error)]
enum DecodeFailure {
    #[error("fact does not decode")]
    Decode(#[source] serde_json::Error),
    #[error("unsupported fact version {0}")]
    UnsupportedVersion(u32),
}

impl DecodeFailure {
    const fn outcome(&self) -> &'static str {
        match self {
            Self::Decode(_) => "decode_failed",
            Self::UnsupportedVersion(_) => "unsupported_version",
        }
    }
}

impl NotificationsConsumer {
    // JSON: the claimed row's fact is undecoded until here.
    pub(super) async fn handle(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        raw: &serde_json::Value,
        ctx: &mut BatchContext,
    ) -> Handled {
        let event = match decode(raw) {
            Ok(event) => event,
            Err(failure) => {
                metrics::fact(failure.outcome());
                fail(conn, id, &FailureStage::Decode, &failure.to_string()).await;
                return Handled::Done(Vec::new());
            },
        };
        match self.process_in_savepoint(conn, id, &event, ctx).await {
            Ok(pending) => {
                metrics::fact("processed");
                Handled::Done(pending)
            },
            Err(error) if error.is_transient() => {
                self.retry_or_abandon(conn, id, &event, error).await
            },
            Err(error) => {
                metrics::fact("failed");
                tracing::warn!(outbox_id = %id, kind = %event.kind, error = %error, "Notification fact failed");
                fail(conn, id, &FailureStage::Process, &error.to_string()).await;
                Handled::Done(Vec::new())
            },
        }
    }

    async fn retry_or_abandon(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        event: &NotificationEvent,
        error: NotificationsError,
    ) -> Handled {
        let earlier = match NotificationFailureRepository::count_for_outbox(
            &mut *conn,
            id,
            &FailureStage::Process,
        )
        .await
        {
            Ok(count) => count,
            Err(count_error) => {
                tracing::warn!(outbox_id = %id, error = %count_error, "Notification fact attempts could not be read");
                return Handled::Retry(error);
            },
        };
        if earlier + 1 < MAX_FACT_ATTEMPTS {
            return Handled::Retry(error);
        }
        metrics::fact("abandoned");
        tracing::error!(outbox_id = %id, kind = %event.kind, attempts = earlier + 1, error = %error, "Notification fact abandoned");
        let reason = format!("fact_abandoned: {error}");
        fail(conn, id, &FailureStage::Process, &reason).await;
        Handled::Done(Vec::new())
    }

    pub(super) async fn record_retry(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        error: &NotificationsError,
    ) {
        metrics::fact("retry");
        tracing::warn!(outbox_id = %id, error = %error, "Notification fact failed; retrying");
        fail(conn, id, &FailureStage::Process, &error.to_string()).await;
    }

    async fn process_in_savepoint(
        &self,
        conn: &mut PgConnection,
        id: &EventOutboxId,
        event: &NotificationEvent,
        ctx: &mut BatchContext,
    ) -> NotificationsResult<Vec<NotificationId>> {
        let mut savepoint = conn.begin().await?;
        match self.pipeline.process(id, event, &mut savepoint, ctx).await {
            Ok(outcome) => {
                savepoint.commit().await?;
                tracing::debug!(
                    outbox_id = %id,
                    persisted = outcome.persisted.len(),
                    held = outcome.held,
                    withheld = outcome.withheld,
                    coalesced = outcome.coalesced,
                    "Notification fact processed"
                );
                Ok(outcome.pending)
            },
            Err(error) => {
                savepoint.rollback().await?;
                Err(error)
            },
        }
    }
}

async fn fail(
    executor: impl sqlx::PgExecutor<'_>,
    id: &EventOutboxId,
    stage: &FailureStage,
    reason: &str,
) {
    let failure = NewFailure {
        outbox_id: Some(id),
        recipient: None,
        stage,
        reason,
    };
    if let Err(error) = NotificationFailureRepository::record(executor, failure).await {
        tracing::error!(outbox_id = %id, %stage, error = %error, "Notification failure could not be recorded");
    }
}
