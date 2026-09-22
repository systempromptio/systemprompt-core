//! Polling durable analytics delivery with atomic projection acknowledgement.
//! The server's one reporting task: it first makes sure a baseline exists
//! (building it in the background while the server serves), then drains
//! captured facts into the projection a batch at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sqlx::PgPool;
use systemprompt_analytics::AnalyticsError;
use systemprompt_analytics::projection::{
    self, REPORTING_CONSUMER, REPORTING_KIND, REPORTING_VERSION, ReportingProjector, ReportingRow,
};
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::{DeliveryBatch, OutboxConsumer};
use systemprompt_identifiers::EventOutboxId;
use tokio::task::JoinHandle;

use super::rebuild::RebuildOutcome;
use crate::RuntimeResult;

const BASELINE_RETRY: Duration = Duration::from_secs(30);
/// Facts applied and acknowledged per transaction.
const BATCH_SIZE: usize = 1000;
/// Facts one tick drains before yielding to the metrics gauge.
const TICK_LIMIT: usize = 10_000;

pub fn spawn(db: &DbPool) -> RuntimeResult<JoinHandle<()>> {
    let pool = db.write_pool_arc()?;
    let db = db.clone();
    Ok(tokio::spawn(async move {
        ensure_baseline(&db).await;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut failing = false;
        let mut next_metrics = tokio::time::Instant::now();
        loop {
            interval.tick().await;
            match drain(&pool, TICK_LIMIT).await {
                Ok(processed) => {
                    if failing {
                        tracing::info!("Analytics projection processing recovered");
                        failing = false;
                    }
                    if processed == TICK_LIMIT {
                        interval.reset_immediately();
                    }
                },
                Err(error) => {
                    metrics::counter!("analytics_projection_failures_total").increment(1);
                    if !failing {
                        tracing::error!(
                            error = %error,
                            "Analytics projection paused; pending facts retained for retry"
                        );
                        failing = true;
                    }
                },
            }
            if tokio::time::Instant::now() >= next_metrics {
                if let Ok(status) = super::status::from_pool(&pool).await {
                    metrics::gauge!("analytics_projection_pending")
                        .set(status.pending_count as f64);
                    metrics::gauge!("analytics_projection_applied_last_minute")
                        .set(status.applied_last_minute as f64);
                    let age = status.oldest_pending_at.map_or(0, |oldest| {
                        (chrono::Utc::now() - oldest).num_seconds().max(0)
                    });
                    metrics::gauge!("analytics_projection_oldest_pending_seconds").set(age as f64);
                }
                next_metrics = tokio::time::Instant::now() + Duration::from_secs(30);
            }
        }
    }))
}

/// Blocks this task, never the server, until the projection has a baseline:
/// a node that finds another one mid-rebuild waits for it, and a failed
/// attempt is retried rather than taking the process down.
async fn ensure_baseline(db: &DbPool) {
    loop {
        match super::rebuild::initialize(db).await {
            Ok(RebuildOutcome::Rebuilt | RebuildOutcome::AlreadyInitialized) => return,
            Ok(RebuildOutcome::InProgressElsewhere) => {
                tracing::info!("Analytics baseline is being rebuilt by another node; waiting");
            },
            Err(error) => {
                metrics::counter!("analytics_projection_failures_total").increment(1);
                tracing::error!(error = %error, "Analytics baseline rebuild failed; retrying");
            },
        }
        tokio::time::sleep(BASELINE_RETRY).await;
    }
}

pub async fn process_pending(db: &DbPool, limit: usize) -> RuntimeResult<usize> {
    let pool = db.write_pool_arc()?;
    drain(&pool, limit).await.map_err(Into::into)
}

/// Applies up to `limit` pending facts in batches, one transaction each.
/// A batch that fails is re-driven in halves until the failing fact stands
/// alone; that fact stays pending, is skipped for the rest of this drain,
/// and is reported in the returned error after everything else applied.
async fn drain(pool: &PgPool, limit: usize) -> Result<usize, AnalyticsError> {
    let outbox = OutboxConsumer::new(pool.clone());
    let mut processed = 0;
    let mut batch_size = BATCH_SIZE;
    let mut poisoned: Vec<EventOutboxId> = Vec::new();
    while processed < limit {
        let want = batch_size.min(limit - processed);
        let Some(batch) = outbox
            .claim_batch(
                REPORTING_CONSUMER,
                i64::try_from(want).unwrap_or(i64::MAX),
                &poisoned,
            )
            .await?
        else {
            break;
        };
        let claimed = batch.len();
        let head = batch.ids().next().cloned();
        match apply_batch(batch).await {
            Ok(()) => {
                processed += claimed;
                batch_size = BATCH_SIZE;
                metrics::counter!("analytics_projection_processed_total")
                    .increment(u64::try_from(claimed).unwrap_or(u64::MAX));
            },
            Err(error) if claimed > 1 => {
                tracing::warn!(
                    error = %error,
                    claimed,
                    "Reporting batch failed; re-driving in halves to isolate the fact"
                );
                batch_size = claimed / 2;
            },
            Err(error) => {
                let id = head.expect("a failed batch holds at least one row");
                tracing::error!(
                    error = %error,
                    outbox_id = %id,
                    "Reporting fact cannot be applied; left pending and skipped"
                );
                poisoned.push(id);
                batch_size = BATCH_SIZE;
            },
        }
    }
    if let Some(first) = poisoned.first() {
        return Err(AnalyticsError::invalid_argument(format!(
            "{} reporting fact(s) left pending; first outbox id {first} (applied {processed} others)",
            poisoned.len()
        )));
    }
    Ok(processed)
}

async fn apply_batch(mut batch: DeliveryBatch) -> Result<(), AnalyticsError> {
    let facts = batch.facts::<ReportingRow>()?;
    for (id, fact) in &facts {
        if fact.consumer != REPORTING_CONSUMER
            || fact.kind != REPORTING_KIND
            || fact.version != REPORTING_VERSION
        {
            return Err(AnalyticsError::invalid_argument(format!(
                "unsupported reporting contract in outbox event {id}",
            )));
        }
    }
    projection::lock_projector(batch.connection()).await?;
    for (_, fact) in &facts {
        ReportingProjector::apply_fact(batch.connection(), &fact.data).await?;
    }
    batch.acknowledge_all().await?;
    Ok(())
}
