//! One drain round: claim, process and acknowledge facts one transaction at
//! a time, then deliver each committed fact's `pending` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use sqlx::{Acquire, PgConnection};
use systemprompt_events::services::durable::Delivery;
use systemprompt_identifiers::{EventOutboxId, NotificationId};
use systemprompt_models::notifications::{
    NOTIFICATION_EVENT_VERSION, NOTIFICATIONS_CONSUMER, NotificationEvent,
};

use super::NotificationsConsumer;
use crate::error::NotificationsResult;
use crate::pipeline::{BatchContext, metrics};
use crate::repository::{NewFailure, NotificationFailureRepository};

impl NotificationsConsumer {
    pub(super) async fn drain_round(&self) -> NotificationsResult<bool> {
        let started = Instant::now();
        let mut ctx: Option<BatchContext> = None;
        let mut claimed = 0;
        while claimed < self.batch() {
            let Some(mut delivery) = self.outbox.claim(NOTIFICATIONS_CONSUMER).await? else {
                break;
            };
            claimed += 1;
            if ctx.is_none() {
                ctx = Some(BatchContext::load(delivery.connection()).await?);
            }
            let Some(context) = ctx.as_mut() else {
                break;
            };
            let id = delivery.id();
            let pending = self.handle(&mut delivery, &id, context).await;
            if let Err(error) = delivery.acknowledge().await {
                tracing::error!(outbox_id = %id, error = %error, "Notification fact acknowledgement failed");
                metrics::fact("ack_failed");
                break;
            }
            if !pending.is_empty() {
                self.pipeline.deliver(&pending).await;
            }
        }
        if claimed > 0 {
            metrics::batch_seconds(started.elapsed().as_secs_f64());
            self.report_backlog().await;
        }
        Ok(claimed > 0)
    }

    // JSON: the fact envelope is decoded per fact, and its data only after
    // its version is checked, so one bad payload is recorded and skipped.
    async fn handle(
        &self,
        delivery: &mut Delivery,
        id: &EventOutboxId,
        ctx: &mut BatchContext,
    ) -> Vec<NotificationId> {
        let fact = match delivery.fact::<serde_json::Value>() {
            Ok(fact) => fact,
            Err(error) => {
                metrics::fact("decode_failed");
                fail(delivery.connection(), id, "decode", &error.to_string()).await;
                return Vec::new();
            },
        };
        if fact.version != NOTIFICATION_EVENT_VERSION {
            metrics::fact("unsupported_version");
            let reason = format!("unsupported fact version {}", fact.version);
            fail(delivery.connection(), id, "decode", &reason).await;
            return Vec::new();
        }
        let event: NotificationEvent = match serde_json::from_value(fact.data) {
            Ok(event) => event,
            Err(error) => {
                metrics::fact("decode_failed");
                fail(delivery.connection(), id, "decode", &error.to_string()).await;
                return Vec::new();
            },
        };
        match self
            .process_in_savepoint(delivery.connection(), id, &event, ctx)
            .await
        {
            Ok(pending) => {
                metrics::fact("processed");
                pending
            },
            Err(error) => {
                metrics::fact("failed");
                tracing::warn!(outbox_id = %id, kind = %event.kind, error = %error, "Notification fact failed");
                fail(delivery.connection(), id, "process", &error.to_string()).await;
                Vec::new()
            },
        }
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

async fn fail(conn: &mut PgConnection, id: &EventOutboxId, stage: &str, reason: &str) {
    let failure = NewFailure {
        outbox_id: Some(id),
        recipient: None,
        stage,
        reason,
    };
    if let Err(error) = NotificationFailureRepository::record(conn, failure).await {
        tracing::error!(outbox_id = %id, stage, error = %error, "Notification failure could not be recorded");
    }
}
