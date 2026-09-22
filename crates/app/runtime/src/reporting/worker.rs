//! Polling durable analytics delivery with atomic projection acknowledgement.
//! The server's one reporting task: it first makes sure a baseline exists
//! (building it in the background while the server serves), then drains
//! captured facts into the projection.
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
use systemprompt_events::services::durable::OutboxConsumer;
use tokio::task::JoinHandle;

use super::rebuild::RebuildOutcome;
use crate::RuntimeResult;

const BASELINE_RETRY: Duration = Duration::from_secs(30);

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
            match drain(&pool, 256).await {
                Ok(processed) => {
                    if failing {
                        tracing::info!("Analytics projection processing recovered");
                        failing = false;
                    }
                    if processed == 256 {
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

async fn drain(pool: &PgPool, limit: usize) -> Result<usize, AnalyticsError> {
    let outbox = OutboxConsumer::new(pool.clone());
    let mut processed = 0;
    while processed < limit {
        let Some(mut delivery) = outbox.claim(REPORTING_CONSUMER).await? else {
            break;
        };
        let fact = delivery.fact::<ReportingRow>()?;
        if fact.consumer != REPORTING_CONSUMER
            || fact.kind != REPORTING_KIND
            || fact.version != REPORTING_VERSION
        {
            return Err(AnalyticsError::invalid_argument(format!(
                "unsupported reporting contract in outbox event {}",
                delivery.id(),
            )));
        }
        projection::lock_projector(delivery.connection()).await?;
        ReportingProjector::apply_fact(delivery.connection(), &fact.data).await?;
        delivery.acknowledge().await?;
        metrics::counter!("analytics_projection_processed_total").increment(1);
        processed += 1;
    }
    Ok(processed)
}
