//! Operational signals for running many replicas: gateway phases and streams,
//! pool acquire wait, scheduler jobs, projection backlog and dropped log rows.
//!
//! sqlx hands the pool straight to every query, so there is no central
//! `acquire` to time. `db_pool_acquire_seconds{pool}` is instead a probe: every
//! five seconds the sampler acquires one connection and records how long the
//! pool made it wait, which is the wait a request arriving at that moment saw.
//!
//! `projection_backlog_depth{projection}` is sampled only on a node that runs
//! the scheduler, since that is where the backlog drains.
//! `log_entries_dropped_total` mirrors the database log sink's in-process drop
//! counter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder};
use sqlx::PgPool;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::job_metrics::{
    JOB_DURATION_BUCKETS, SCHEDULER_JOB_DURATION_SECONDS, SCHEDULER_JOB_RUNS_TOTAL,
};

pub(super) const DB_POOL_ACQUIRE_SECONDS: &str = "db_pool_acquire_seconds";
pub(super) const PROJECTION_BACKLOG_DEPTH: &str = "projection_backlog_depth";
pub(super) const LOG_ENTRIES_DROPPED_TOTAL: &str = "log_entries_dropped_total";

const ACQUIRE_BUCKETS: [f64; 12] = [
    0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0,
];
const ACQUIRE_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
const BACKLOG_INTERVAL: Duration = Duration::from_secs(30);

pub(super) fn with_buckets(builder: PrometheusBuilder) -> Result<PrometheusBuilder, BuildError> {
    builder
        .set_buckets_for_metric(
            Matcher::Full(systemprompt_gateway::GATEWAY_PHASE_SECONDS.to_owned()),
            &systemprompt_gateway::PHASE_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(DB_POOL_ACQUIRE_SECONDS.to_owned()),
            &ACQUIRE_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(SCHEDULER_JOB_DURATION_SECONDS.to_owned()),
            &JOB_DURATION_BUCKETS,
        )
}

pub(super) fn describe() {
    metrics::describe_histogram!(
        systemprompt_gateway::GATEWAY_PHASE_SECONDS,
        metrics::Unit::Seconds,
        "Gateway request time by phase (admission, upstream_first_byte, stream, finalize)"
    );
    metrics::describe_gauge!(
        systemprompt_gateway::GATEWAY_STREAMS_IN_FLIGHT,
        "Streamed gateway response bodies still being written by this process"
    );
    metrics::describe_histogram!(
        DB_POOL_ACQUIRE_SECONDS,
        metrics::Unit::Seconds,
        "Time a probe waited to acquire a Postgres connection from the pool"
    );
    metrics::describe_counter!(
        SCHEDULER_JOB_RUNS_TOTAL,
        "Scheduled job runs by job and status (success, idle, error)"
    );
    metrics::describe_histogram!(
        SCHEDULER_JOB_DURATION_SECONDS,
        metrics::Unit::Seconds,
        "Scheduled job run duration by job"
    );
    metrics::describe_gauge!(
        PROJECTION_BACKLOG_DEPTH,
        "Keys queued for a projection refresh and not yet drained"
    );
    metrics::describe_counter!(
        LOG_ENTRIES_DROPPED_TOTAL,
        "Log entries the database sink dropped because its queue was full"
    );
    metrics::describe_counter!(
        systemprompt_events::CACHE_INVALIDATIONS_RECEIVED_TOTAL,
        "Cross-replica cache invalidations received, by cache domain"
    );
}

pub(super) fn spawn_samplers(ctx: &AppContext, runs_scheduler: bool) {
    let write = ctx.db_pool().write_pool();
    let read = ctx.db_pool().pool();
    let read = (!Arc::ptr_eq(&read, &write)).then_some(read);
    let backlog_pool = runs_scheduler.then(|| Arc::clone(&write));
    ctx.background_tasks()
        .spawn_cancellable("ops_metrics", |cancel| async move {
            let mut tick = tokio::time::interval(SAMPLE_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut backlog_due = Instant::now();
            let mut seen_projections = HashSet::new();
            let mut config_info = None;
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {},
                }
                probe_acquire("write", &write).await;
                if let Some(read) = read.as_ref() {
                    probe_acquire("read", read).await;
                }
                super::config_fingerprint::publish(&mut config_info);
                metrics::counter!(LOG_ENTRIES_DROPPED_TOTAL)
                    .absolute(systemprompt_logging::dropped_log_entries());
                if let Some(pool) = backlog_pool.as_ref()
                    && Instant::now() >= backlog_due
                {
                    backlog_due = Instant::now() + BACKLOG_INTERVAL;
                    sample_backlog(pool, &mut seen_projections).await;
                }
            }
        });
}

async fn probe_acquire(label: &'static str, pool: &PgPool) {
    let started = Instant::now();
    match tokio::time::timeout(ACQUIRE_PROBE_TIMEOUT, pool.acquire()).await {
        Ok(Ok(connection)) => {
            let waited = started.elapsed();
            drop(connection);
            metrics::histogram!(DB_POOL_ACQUIRE_SECONDS, "pool" => label)
                .record(waited.as_secs_f64());
        },
        Ok(Err(error)) => {
            tracing::warn!(pool = label, error = %error, "pool acquire probe failed");
        },
        Err(_) => {
            metrics::histogram!(DB_POOL_ACQUIRE_SECONDS, "pool" => label)
                .record(ACQUIRE_PROBE_TIMEOUT.as_secs_f64());
        },
    }
}

async fn sample_backlog(pool: &PgPool, seen: &mut HashSet<String>) {
    match systemprompt_database::projection::depths(pool).await {
        Ok(depths) => {
            for drained in seen
                .iter()
                .filter(|name| !depths.iter().any(|(p, _)| p == *name))
            {
                metrics::gauge!(PROJECTION_BACKLOG_DEPTH, "projection" => drained.clone()).set(0.0);
            }
            for (projection, depth) in depths {
                seen.insert(projection.clone());
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a backlog depth is far below 2^52"
                )]
                let depth = depth as f64;
                metrics::gauge!(PROJECTION_BACKLOG_DEPTH, "projection" => projection).set(depth);
            }
        },
        Err(error) => {
            tracing::warn!(error = %error, "projection backlog sample failed");
        },
    }
}
