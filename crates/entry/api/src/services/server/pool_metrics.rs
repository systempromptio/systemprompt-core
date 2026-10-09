//! Connection-pool gauges for the read and write Postgres pools.
//!
//! A background task samples each `PgPool` every five seconds and publishes
//! `db_pool_size` and `db_pool_connections{state="idle"|"used"}`, labelled by
//! `pool="read"|"write"`. When no read replica is configured both handles
//! point at one pool, which is then reported once as `write`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_runtime::AppContext;

pub(super) const DB_POOL_SIZE: &str = "db_pool_size";
pub(super) const DB_POOL_CONNECTIONS: &str = "db_pool_connections";

const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

pub(super) fn describe() {
    metrics::describe_gauge!(
        DB_POOL_SIZE,
        "Open Postgres connections in the pool (idle + in use)"
    );
    metrics::describe_gauge!(
        DB_POOL_CONNECTIONS,
        "Postgres pool connections by state (idle or used)"
    );
}

pub(super) fn spawn_sampler(ctx: &AppContext) {
    let write = ctx.db_pool().write_pool();
    let read = ctx.db_pool().pool();
    let read = (!Arc::ptr_eq(&read, &write)).then_some(read);
    ctx.background_tasks()
        .spawn_cancellable("db_pool_metrics", |cancel| async move {
            let mut tick = tokio::time::interval(SAMPLE_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {},
                }
                record("write", &write);
                if let Some(read) = read.as_ref() {
                    record("read", read);
                }
            }
        });
}

fn record(label: &'static str, pool: &PgPool) {
    let size = pool.size();
    let idle = u32::try_from(pool.num_idle()).unwrap_or(size);
    let used = size.saturating_sub(idle);
    metrics::gauge!(DB_POOL_SIZE, "pool" => label).set(f64::from(size));
    metrics::gauge!(DB_POOL_CONNECTIONS, "pool" => label, "state" => "idle").set(f64::from(idle));
    metrics::gauge!(DB_POOL_CONNECTIONS, "pool" => label, "state" => "used").set(f64::from(used));
}
