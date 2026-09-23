//! Periodic heartbeat on this replica's `services` rows, and the reaping of
//! rows whose replica stopped heartbeating.
//!
//! Every replica heartbeats every [`HEARTBEAT_INTERVAL`]; every
//! [`GC_EVERY_BEATS`]th beat it also deletes rows whose heartbeat is older
//! than [`DEAD_AFTER_SECS`]. The delete is idempotent, so replicas reaping
//! concurrently is harmless, and a deployment with any live replica keeps its
//! registry clean without a scheduled job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_runtime::AppContext;
use tokio::task::JoinHandle;

pub(in crate::services::server) const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

const GC_EVERY_BEATS: u32 = 4;

// Why: six missed heartbeats — long enough that a replica stalled by a slow
// deploy or a GC pause is not evicted while it is still serving.
const DEAD_AFTER_SECS: i64 = 90;

pub(in crate::services::server) fn start_registry_heartbeat(ctx: &AppContext) -> JoinHandle<()> {
    let repository = Arc::clone(ctx.service_repository());
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut beats: u32 = 0;
        loop {
            tick.tick().await;
            if let Err(error) = repository.touch_heartbeat().await {
                tracing::warn!(
                    instance_id = %repository.instance_id(),
                    error = %error,
                    "service registry heartbeat failed"
                );
            }
            beats = beats.wrapping_add(1);
            if !beats.is_multiple_of(GC_EVERY_BEATS) {
                continue;
            }
            match repository.delete_dead_instances(DEAD_AFTER_SECS).await {
                Ok(0) => {},
                Ok(reaped) => tracing::info!(reaped, "service registry reaped dead instances"),
                Err(error) => {
                    tracing::warn!(error = %error, "service registry reap failed");
                },
            }
        }
    })
}
