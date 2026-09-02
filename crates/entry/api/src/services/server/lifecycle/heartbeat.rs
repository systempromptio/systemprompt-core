//! Periodic heartbeat on this replica's `services` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_runtime::AppContext;
use tokio::task::JoinHandle;

pub(in crate::services::server) const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

pub(in crate::services::server) fn start_registry_heartbeat(ctx: &AppContext) -> JoinHandle<()> {
    let repository = Arc::clone(ctx.service_repository());
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            if let Err(error) = repository.touch_heartbeat().await {
                tracing::warn!(
                    instance_id = %repository.instance_id(),
                    error = %error,
                    "service registry heartbeat failed"
                );
            }
        }
    })
}
